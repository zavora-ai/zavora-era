use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::fx::*;
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::types::AgentOrUserId;

/// Upsert an exchange rate.
pub async fn upsert_rate(engine: &ErpEngine, req: UpsertRateRequest) -> ErpResult<ExchangeRate> {
    let id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO exchange_rates (id, entity_id, from_ccy, to_ccy, rate_date, rate_type, rate, source)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           ON CONFLICT (entity_id, from_ccy, to_ccy, rate_date, rate_type) 
           DO UPDATE SET rate = $7, source = $8"#,
    )
    .bind(id)
    .bind(engine.entity_id())
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
        entity_id: engine.entity_id(),
        from_ccy: req.from_ccy,
        to_ccy: req.to_ccy,
        rate_date: req.rate_date,
        rate_type: req.rate_type,
        rate: req.rate,
        source: req.source,
    })
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
    period_id: Uuid,
    rate_date: NaiveDate,
    triggered_by: AgentOrUserId,
) -> ErpResult<Uuid> {
    let base_ccy = engine.config().base_currency.clone();

    // Get the period to determine date range
    let period = crate::services::periods::get_period(engine, period_id).await?;

    // Find all accounts with foreign currency balances as of the rate_date.
    // We look at journal lines where currency != base currency and sum their transaction amounts.
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
           GROUP BY jl.account_code, a.name, jl.currency
           HAVING COALESCE(SUM(COALESCE(jl.debit, 0) - COALESCE(jl.credit, 0)), 0) != 0"#,
    )
    .bind(engine.entity_id())
    .bind(rate_date)
    .bind(&base_ccy)
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
        let new_rate = get_rate(engine, &bal.currency, &base_ccy, rate_date).await?;

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
                account_code: engine.posting().unrealised_fx_gain.clone(),
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
                account_code: engine.posting().unrealised_fx_loss.clone(),
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
        reference: format!("FXREVAL-{}", rate_date),
        description: format!("FX revaluation as at {}", rate_date),
        lines: journal_lines.clone(),
        post_immediately: true,
    };

    let entry = crate::services::journal::create_and_post(
        engine,
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
        reference: format!("FXREVAL-REV-{}", rate_date),
        description: format!("Reversal of FX revaluation as at {}", rate_date),
        lines: reversal_lines,
        post_immediately: true,
    };

    // Get or create the next period for the reversal
    let next_period = crate::services::periods::period_for_date(engine, reversal_date).await?;

    let _reversal_entry = crate::services::journal::create_and_post(
        engine,
        reversal_req,
        next_period.id,
        triggered_by,
    )
    .await?;

    Ok(entry.id)
}

/// Get the exchange rate for a currency pair on a given date.
/// Falls back to the most recent rate before that date.
async fn get_rate(
    engine: &ErpEngine,
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
    .bind(engine.entity_id())
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
