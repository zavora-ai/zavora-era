use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::fx::*;
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::types::AgentOrUserId;

/// Upsert an exchange rate.
pub async fn upsert_rate(engine: &ErpEngine, entity_id: Uuid, req: UpsertRateRequest) -> ErpResult<ExchangeRate> {
    let id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO exchange_rates (id, entity_id, from_ccy, to_ccy, rate_date, rate_type, rate, source)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           ON CONFLICT (entity_id, from_ccy, to_ccy, rate_date, rate_type) 
           DO UPDATE SET rate = $7, source = $8"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&req.from_ccy)
    .bind(&req.to_ccy)
    .bind(req.rate_date)
    .bind(serde_json::to_string(&req.rate_type).unwrap_or_default())
    .bind(req.rate)
    .bind(&req.source)
    .execute(engine.pool())
    .await?;

    Ok(ExchangeRate {
        id,
        entity_id,
        from_ccy: req.from_ccy,
        to_ccy: req.to_ccy,
        rate_date: req.rate_date,
        rate_type: req.rate_type,
        rate: req.rate,
        source: req.source,
    })
}

// ── CBK (Central Bank of Kenya) daily rate auto-load ──────────────────────────
//
// The CBK does not expose a first-class REST API, so we consume its official
// daily indicative rates via the open-source Frankfurter service, which
// republishes the CBK feed (base https://api.frankfurter.dev, provider=CBK).
// The feed is EUR-pivoted; we derive each foreign→base cross-rate and upsert
// with source="CBK". Configurable via FX_PROVIDER_URL (e.g. a self-hosted
// Frankfurter) for air-gapped deploys.

/// One row of the Frankfurter `/v2/rates` payload (base=EUR, quote=X).
#[derive(Debug, Clone, serde::Deserialize)]
struct FrankfurterRate {
    date: String,
    #[allow(dead_code)]
    base: String,
    quote: String,
    rate: f64,
}

/// Summary of a CBK rate sync.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CbkSyncSummary {
    pub date: NaiveDate,
    pub base: String,
    pub updated: usize,
    pub currencies: Vec<String>,
}

/// Derive foreign→`base` cross-rates from the EUR-pivoted CBK feed.
/// The feed gives `rate` = units of `quote` per 1 EUR, so:
///   X→base = (EUR→base) / (EUR→X)  (i.e. how many `base` units 1 X buys).
/// Pure + total (no I/O) so it is unit-testable. Returns the feed date and the
/// list of (currency, rate) pairs, base excluded.
fn cross_rates_for_base(rows: &[FrankfurterRate], base: &str) -> Option<(NaiveDate, Vec<(String, Decimal)>)> {
    let eur_to_base = rows.iter().find(|r| r.quote.eq_ignore_ascii_case(base))?.rate;
    if eur_to_base <= 0.0 {
        return None;
    }
    let date = rows.first().and_then(|r| NaiveDate::parse_from_str(&r.date, "%Y-%m-%d").ok())?;
    let mut out = Vec::new();
    for r in rows {
        if r.quote.eq_ignore_ascii_case(base) || r.rate <= 0.0 {
            continue;
        }
        let x_to_base = eur_to_base / r.rate;
        if let Some(d) = Decimal::from_f64_retain(x_to_base) {
            out.push((r.quote.to_uppercase(), d.round_dp(6)));
        }
    }
    Some((date, out))
}

/// Fetch the CBK rate table (via Frankfurter).
async fn fetch_cbk_rows() -> ErpResult<Vec<FrankfurterRate>> {
    let base_url = std::env::var("FX_PROVIDER_URL")
        .unwrap_or_else(|_| "https://api.frankfurter.dev".to_string());
    let url = format!("{}/v2/rates?providers=CBK", base_url.trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| ErpError::ValidationFailed { message: format!("CBK rate feed unreachable: {e}") })?;
    if !resp.status().is_success() {
        return Err(ErpError::ValidationFailed {
            message: format!("CBK rate feed returned HTTP {}", resp.status()),
        });
    }
    resp.json::<Vec<FrankfurterRate>>()
        .await
        .map_err(|e| ErpError::ValidationFailed { message: format!("CBK rate feed response invalid: {e}") })
}

/// Auto-load the latest CBK indicative rates for an entity: fetch the feed,
/// derive foreign→base cross-rates, and upsert them (source="CBK", Spot).
pub async fn sync_cbk_rates(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<CbkSyncSummary> {
    let base = engine.config_for(entity_id).await?.base_currency.clone();
    let rows = fetch_cbk_rows().await?;
    let (date, crosses) = cross_rates_for_base(&rows, &base).ok_or_else(|| ErpError::ValidationFailed {
        message: format!("CBK feed carries no rate for base currency {base}"),
    })?;

    let mut currencies = Vec::with_capacity(crosses.len());
    for (ccy, rate) in crosses {
        upsert_rate(
            engine,
            entity_id,
            UpsertRateRequest {
                from_ccy: ccy.clone(),
                to_ccy: base.clone(),
                rate_date: date,
                rate_type: RateType::Spot,
                rate,
                source: "CBK".to_string(),
            },
        )
        .await?;
        currencies.push(ccy);
    }
    Ok(CbkSyncSummary { date, base, updated: currencies.len(), currencies })
}

/// Run FX revaluation for a period.
///
/// This function:
/// 1. Finds all accounts with non-base-currency balances
/// 2. Computes unrealised gain/loss using the new rate vs the last known rate
/// 3. Creates a journal entry: DR/CR Unrealised FX Gain/Loss
/// 4. Creates a reversal entry dated first day of next period
///
/// Returns the ID of the main journal entry.
pub async fn run_fx_revaluation(
    engine: &ErpEngine,
    entity_id: Uuid,
    period_id: Uuid,
    rate_date: NaiveDate,
    triggered_by: AgentOrUserId,
) -> ErpResult<Uuid> {
    let base_ccy = engine.config_for(entity_id).await?.base_currency.clone();

    // Get the period to determine date range
    let period = crate::services::periods::get_period(engine, entity_id, period_id).await?;

    // Find MONETARY accounts with foreign currency balances as of the rate_date.
    // IAS 21 retranslates monetary items only (cash, receivables, payables):
    // restricting to balance-sheet account types keeps P&L lines out (an FCY
    // expense is settled history, not an open exposure), and the explicit
    // exclusions keep the non-monetary balance-sheet accounts (stock, fixed
    // assets, accumulated depreciation) at their historical rates.
    let posting = engine.posting_for(entity_id).await?;
    let non_monetary = vec![
        posting.inventory_asset.clone(),
        posting.fixed_asset.clone(),
        posting.accumulated_depreciation.clone(),
    ];
    let fcy_balances = sqlx::query_as::<_, FcyBalanceRow>(
        r#"SELECT
               jl.account_code,
               a.name as account_name,
               jl.currency,
               COALESCE(SUM(COALESCE(jl.debit, 0) - COALESCE(jl.credit, 0)), 0) as balance_fcy,
               COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0) as balance_lcy
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           JOIN accounts a ON a.code = jl.account_code AND a.entity_id = je.entity_id
           WHERE je.entity_id = $1
             AND je.status = 'posted'
             AND je.date <= $2
             AND jl.currency != $3
             AND a.account_type IN ('Asset', 'Liability', 'ContraAsset', 'ContraLiability')
             AND NOT (jl.account_code = ANY($4))
           GROUP BY jl.account_code, a.name, jl.currency
           HAVING COALESCE(SUM(COALESCE(jl.debit, 0) - COALESCE(jl.credit, 0)), 0) != 0"#,
    )
    .bind(entity_id)
    .bind(rate_date)
    .bind(&base_ccy)
    .bind(&non_monetary)
    .fetch_all(engine.pool())
    .await?;

    if fcy_balances.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "No foreign currency balances found for revaluation".to_string(),
        });
    }

    // For each account/currency, get the new rate and compute gain/loss
    let mut reval_lines = Vec::new();
    let mut journal_lines = Vec::new();
    let mut total_gain = Decimal::ZERO;
    let mut total_loss = Decimal::ZERO;

    for bal in &fcy_balances {
        // Get the new rate for this currency pair on rate_date
        let new_rate = get_rate(engine, entity_id, &bal.currency, &base_ccy, rate_date).await?;

        // New value in local currency
        let new_value_lcy = bal.balance_fcy * new_rate;
        // Old value is the current functional balance
        let old_value_lcy = bal.balance_lcy;
        // Old rate implied
        let old_rate = if bal.balance_fcy != Decimal::ZERO {
            old_value_lcy / bal.balance_fcy
        } else {
            Decimal::ONE
        };

        let gain_loss = new_value_lcy - old_value_lcy;

        if gain_loss == Decimal::ZERO {
            continue;
        }

        reval_lines.push(FxRevaluationLine {
            account_code: bal.account_code.clone(),
            account_name: bal.account_name.clone(),
            currency: bal.currency.clone(),
            balance_fcy: bal.balance_fcy,
            old_rate,
            new_rate,
            old_value_lcy,
            new_value_lcy,
            gain_loss,
        });

        if gain_loss > Decimal::ZERO {
            total_gain += gain_loss;
        } else {
            total_loss += gain_loss.abs();
        }

        // Create journal lines:
        // If gain (new > old): DR Account / CR Unrealised FX Gain (8100)
        // If loss (new < old): DR Unrealised FX Loss (8110) / CR Account
        if gain_loss > Decimal::ZERO {
            // DR the asset/liability account for the revaluation increase
            journal_lines.push(CreateJournalLineRequest {
                account_code: bal.account_code.clone(),
                debit: Some(gain_loss),
                credit: None,
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("FX reval {} {}", bal.currency, bal.account_code)),
                dimensions: None,
            });
            // CR Unrealised FX Gain
            journal_lines.push(CreateJournalLineRequest {
                account_code: engine.posting_for(entity_id).await?.unrealised_fx_gain.clone(),
                debit: None,
                credit: Some(gain_loss),
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("FX gain on {} {}", bal.currency, bal.account_code)),
                dimensions: None,
            });
        } else {
            let loss_amount = gain_loss.abs();
            // DR Unrealised FX Loss
            journal_lines.push(CreateJournalLineRequest {
                account_code: engine.posting_for(entity_id).await?.unrealised_fx_loss.clone(),
                debit: Some(loss_amount),
                credit: None,
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("FX loss on {} {}", bal.currency, bal.account_code)),
                dimensions: None,
            });
            // CR the asset/liability account
            journal_lines.push(CreateJournalLineRequest {
                account_code: bal.account_code.clone(),
                debit: None,
                credit: Some(loss_amount),
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("FX reval {} {}", bal.currency, bal.account_code)),
                dimensions: None,
            });
        }
    }

    if journal_lines.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "No revaluation adjustments needed — all balances are at current rates".to_string(),
        });
    }

    // Create main revaluation journal entry
    let entry_req = CreateJournalEntryRequest {
        date: rate_date,
        source: JournalSource::FxRevaluation,
        source_id: None,
        reference: format!("FXREVAL-{}", rate_date),
        description: format!("FX revaluation as at {}", rate_date),
        lines: journal_lines.clone(),
        post_immediately: true,
    };

    let entry = crate::services::journal::create_and_post(
        engine,
        entity_id,
        entry_req,
        period_id,
        triggered_by.clone(),
    )
    .await?;

    // Create reversal entry dated first day of next period
    let reversal_date = next_period_start(period.end_date);
    let reversal_lines: Vec<CreateJournalLineRequest> = journal_lines
        .iter()
        .map(|l| CreateJournalLineRequest {
            account_code: l.account_code.clone(),
            debit: l.credit, // swap debit/credit for reversal
            credit: l.debit,
            currency: l.currency.clone(),
            fx_rate: l.fx_rate,
            description: l.description.as_ref().map(|d| format!("Reversal: {}", d)),
            dimensions: None,
        })
        .collect();

    let reversal_req = CreateJournalEntryRequest {
        date: reversal_date,
        source: JournalSource::FxRevaluation,
        source_id: None,
        reference: format!("FXREVAL-REV-{}", rate_date),
        description: format!("Reversal of FX revaluation as at {}", rate_date),
        lines: reversal_lines,
        post_immediately: true,
    };

    // Get or create the next period for the reversal
    let next_period = crate::services::periods::period_for_date(engine, entity_id, reversal_date).await?;

    let _reversal_entry = crate::services::journal::create_and_post(
        engine,
        entity_id,
        reversal_req,
        next_period.id,
        triggered_by,
    )
    .await?;

    Ok(entry.id)
}

/// Get the exchange rate for a currency pair on a given date.
/// Falls back to the most recent rate before that date.
pub(crate) async fn get_rate(
    engine: &ErpEngine,
    entity_id: Uuid,
    from_ccy: &str,
    to_ccy: &str,
    date: NaiveDate,
) -> ErpResult<Decimal> {
    // Try exact date first, then most recent prior date
    let rate = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT rate FROM exchange_rates 
           WHERE entity_id = $1 AND from_ccy = $2 AND to_ccy = $3 AND rate_date <= $4
           ORDER BY rate_date DESC
           LIMIT 1"#,
    )
    .bind(entity_id)
    .bind(from_ccy)
    .bind(to_ccy)
    .bind(date)
    .fetch_optional(engine.pool())
    .await?;

    rate.ok_or_else(|| ErpError::FxRateNotFound {
        from_ccy: from_ccy.to_string(),
        to_ccy: to_ccy.to_string(),
        date,
    })
}

/// Get the **month rate** for a currency pair for the month containing `date`.
///
/// Per IAS 21, a periodic (e.g. monthly) rate is permitted when rates don't
/// fluctuate significantly. We use the rate dated on/closest to the month-end of
/// `date`'s month: prefer a rate within that month (latest one, i.e. month-end),
/// else fall back to the most recent rate on/before the month-end. This keeps a
/// whole month's foreign receipts (e.g. Amazon KDP royalties paid monthly) on a
/// single consistent rate.
pub(crate) async fn get_month_rate(
    engine: &ErpEngine,
    entity_id: Uuid,
    from_ccy: &str,
    to_ccy: &str,
    date: NaiveDate,
) -> ErpResult<Decimal> {
    use chrono::Datelike;
    // Last day of `date`'s month.
    let (y, m) = (date.year(), date.month());
    let month_end = if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1)
    }
    .and_then(|d| d.pred_opt())
    .unwrap_or(date);
    let month_start = NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(date);

    // Prefer the latest rate within the month (the month-end rate).
    let in_month = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT rate FROM exchange_rates
           WHERE entity_id = $1 AND from_ccy = $2 AND to_ccy = $3
             AND rate_date >= $4 AND rate_date <= $5
           ORDER BY rate_date DESC
           LIMIT 1"#,
    )
    .bind(entity_id)
    .bind(from_ccy)
    .bind(to_ccy)
    .bind(month_start)
    .bind(month_end)
    .fetch_optional(engine.pool())
    .await?;
    if let Some(r) = in_month {
        return Ok(r);
    }
    // Else fall back to the most recent rate on/before the month-end.
    get_rate(engine, entity_id, from_ccy, to_ccy, month_end).await
}

/// Compute the first day of the next month after a given date.
fn next_period_start(period_end: NaiveDate) -> NaiveDate {
    period_end + chrono::Duration::days(1)
}

#[derive(Debug, sqlx::FromRow)]
struct FcyBalanceRow {
    account_code: String,
    account_name: String,
    currency: String,
    balance_fcy: Decimal,
    balance_lcy: Decimal,
}

#[cfg(test)]
mod cbk_tests {
    use super::{cross_rates_for_base, FrankfurterRate};
    use rust_decimal::Decimal;

    fn row(quote: &str, rate: f64) -> FrankfurterRate {
        FrankfurterRate { date: "2026-07-10".into(), base: "EUR".into(), quote: quote.into(), rate }
    }

    #[test]
    fn derives_foreign_to_kes_cross_rates() {
        // EUR-pivoted feed: 1 EUR = 147.76 KES = 1.1437 USD = 0.85199 GBP.
        let rows = vec![row("KES", 147.76), row("USD", 1.1437), row("GBP", 0.85199), row("EUR", 1.0)];
        let (date, crosses) = cross_rates_for_base(&rows, "KES").expect("KES present");
        assert_eq!(date.to_string(), "2026-07-10");
        // Base itself is excluded.
        assert!(!crosses.iter().any(|(c, _)| c == "KES"), "base excluded");
        // USD→KES ≈ 147.76 / 1.1437 ≈ 129.19.
        let usd = crosses.iter().find(|(c, _)| c == "USD").expect("USD").1;
        assert!((usd - Decimal::new(12919, 2)).abs() < Decimal::new(2, 2), "USD/KES ~129.19, got {usd}");
        // GBP→KES ≈ 147.76 / 0.85199 ≈ 173.43.
        let gbp = crosses.iter().find(|(c, _)| c == "GBP").expect("GBP").1;
        assert!((gbp - Decimal::new(17343, 2)).abs() < Decimal::new(5, 2), "GBP/KES ~173.43, got {gbp}");
    }

    #[test]
    fn returns_none_when_base_absent() {
        let rows = vec![row("USD", 1.1437), row("GBP", 0.85199)];
        assert!(cross_rates_for_base(&rows, "KES").is_none());
    }

    #[test]
    fn skips_nonpositive_rates() {
        let rows = vec![row("KES", 147.76), row("USD", 0.0), row("GBP", 0.85199)];
        let (_d, crosses) = cross_rates_for_base(&rows, "KES").unwrap();
        assert!(!crosses.iter().any(|(c, _)| c == "USD"), "zero-rate currency skipped");
    }
}
