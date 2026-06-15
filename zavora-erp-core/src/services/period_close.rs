use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::period::{FiscalPeriod, PeriodStatus};
use crate::types::AgentOrUserId;

/// Request to execute a year-end close for a fiscal year.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct YearEndCloseRequest {
    pub fiscal_year: i32,
    pub executed_by: AgentOrUserId,
}

/// Result of a successful year-end close.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct YearEndCloseResult {
    pub fiscal_year: i32,
    pub closing_entry_id: Uuid,
    pub opening_entry_id: Uuid,
    pub net_income: Decimal,
}

/// Account balance row returned from the aggregate query.
#[derive(Debug, Clone, sqlx::FromRow)]
struct AccountBalance {
    pub account_code: String,
    pub account_type: String,
    pub balance: Decimal,
}

/// Execute the year-end closing procedure for a fiscal year.
///
/// Steps:
/// 1. Verify all 12 periods of the fiscal year are HardClosed
/// 2. Compute total Revenue and Expense balances across all periods
/// 3. Create closing JE: DR Revenue accounts / CR Expense accounts / net to Retained Earnings (4600)
/// 4. Create opening balance JE in period 1 of next fiscal year carrying forward all BS account balances
pub async fn execute_year_end_close(
    engine: &ErpEngine,
    req: YearEndCloseRequest,
) -> ErpResult<YearEndCloseResult> {
    // Step 1: Verify all 12 periods are HardClosed
    let periods = get_fiscal_year_periods(engine, req.fiscal_year).await?;
    validate_all_periods_hard_closed(&periods)?;

    // Step 2: Compute P&L account balances (Revenue and Expense) for the fiscal year
    let pnl_balances = compute_pnl_balances(engine, &periods).await?;

    // Step 3: Generate closing Journal Entry
    let last_period = periods
        .last()
        .ok_or_else(|| ErpError::ValidationFailed {
            message: "No periods found for fiscal year".to_string(),
        })?;

    let closing_entry = build_closing_entry(engine, &pnl_balances, last_period, &req).await?;
    let closing_je = crate::services::journal::create_and_post(
        engine,
        closing_entry,
        last_period.id,
        req.executed_by.clone(),
    )
    .await?;

    // Step 4: Generate opening balance JE in period 1 of next fiscal year
    let next_year_periods = get_fiscal_year_periods(engine, req.fiscal_year + 1).await?;
    let first_period_next_year = next_year_periods
        .first()
        .ok_or_else(|| ErpError::ValidationFailed {
            message: format!(
                "No fiscal periods found for next year {}. Please generate periods first.",
                req.fiscal_year + 1
            ),
        })?;

    let bs_balances = compute_balance_sheet_balances(engine, &periods).await?;
    let opening_entry =
        build_opening_entry(engine, &bs_balances, first_period_next_year, &req).await?;
    let opening_je = crate::services::journal::create_and_post(
        engine,
        opening_entry,
        first_period_next_year.id,
        req.executed_by.clone(),
    )
    .await?;

    // Compute net income (Revenue - Expense) for reporting
    let net_income = compute_net_income(&pnl_balances);

    // Emit audit event
    let audit_event = serde_json::json!({
        "event_type": "year_end_close",
        "object_type": "fiscal_year",
        "fiscal_year": req.fiscal_year,
        "closing_entry_id": closing_je.id,
        "opening_entry_id": opening_je.id,
        "net_income": net_income.to_string(),
        "actor": req.executed_by,
        "timestamp": Utc::now(),
    });

    let stream_key = format!("erp:audit:{}", engine.entity_id());
    let mut redis_conn = engine.redis_conn().await;
    let _: Result<(), _> = redis::cmd("XADD")
        .arg(&stream_key)
        .arg("*")
        .arg("data")
        .arg(audit_event.to_string())
        .query_async(&mut redis_conn)
        .await;

    Ok(YearEndCloseResult {
        fiscal_year: req.fiscal_year,
        closing_entry_id: closing_je.id,
        opening_entry_id: opening_je.id,
        net_income,
    })
}

/// Fetch all fiscal periods for a given year, ordered by period_number.
async fn get_fiscal_year_periods(
    engine: &ErpEngine,
    fiscal_year: i32,
) -> ErpResult<Vec<FiscalPeriod>> {
    let periods = sqlx::query_as::<_, FiscalPeriod>(
        r#"SELECT * FROM fiscal_periods
           WHERE entity_id = $1 AND fiscal_year = $2
           ORDER BY period_number ASC"#,
    )
    .bind(engine.entity_id())
    .bind(fiscal_year)
    .fetch_all(engine.pool())
    .await?;

    if periods.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: format!("No fiscal periods found for year {}", fiscal_year),
        });
    }

    Ok(periods)
}

/// Validate that all 12 periods are HardClosed. Return an error listing any non-HardClosed periods.
fn validate_all_periods_hard_closed(periods: &[FiscalPeriod]) -> ErpResult<()> {
    if periods.len() != 12 {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Expected 12 periods for the fiscal year, found {}",
                periods.len()
            ),
        });
    }

    let non_closed: Vec<&FiscalPeriod> = periods
        .iter()
        .filter(|p| p.parsed_status() != PeriodStatus::HardClosed)
        .collect();

    if !non_closed.is_empty() {
        let names: Vec<String> = non_closed
            .iter()
            .map(|p| format!("{} (status: {})", p.name, p.status))
            .collect();
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Year-end close requires all periods to be hard-closed. The following periods are not: {}",
                names.join(", ")
            ),
        });
    }

    Ok(())
}

/// Compute the net balance for all Revenue and Expense accounts across the fiscal year periods.
/// Returns a list of (account_code, account_type, net_balance) where:
/// - Revenue accounts: balance = sum(credits) - sum(debits) (credit-normal)
/// - Expense accounts: balance = sum(debits) - sum(credits) (debit-normal)
async fn compute_pnl_balances(
    engine: &ErpEngine,
    periods: &[FiscalPeriod],
) -> ErpResult<Vec<AccountBalance>> {
    let period_ids: Vec<Uuid> = periods.iter().map(|p| p.id).collect();

    let balances = sqlx::query_as::<_, AccountBalance>(
        r#"SELECT
               jl.account_code,
               a.account_type,
               CASE
                   WHEN a.account_type = 'revenue' THEN
                       COALESCE(SUM(jl.functional_credit), 0) - COALESCE(SUM(jl.functional_debit), 0)
                   WHEN a.account_type = 'expense' THEN
                       COALESCE(SUM(jl.functional_debit), 0) - COALESCE(SUM(jl.functional_credit), 0)
                   ELSE 0
               END AS balance
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           JOIN accounts a ON a.code = jl.account_code AND a.entity_id = je.entity_id
           WHERE je.entity_id = $1
             AND je.period_id = ANY($2)
             AND je.status = 'posted'
             AND a.account_type IN ('revenue', 'expense')
           GROUP BY jl.account_code, a.account_type
           HAVING CASE
               WHEN a.account_type = 'revenue' THEN
                   COALESCE(SUM(jl.functional_credit), 0) - COALESCE(SUM(jl.functional_debit), 0)
               WHEN a.account_type = 'expense' THEN
                   COALESCE(SUM(jl.functional_debit), 0) - COALESCE(SUM(jl.functional_credit), 0)
               ELSE 0
           END <> 0"#,
    )
    .bind(engine.entity_id())
    .bind(&period_ids)
    .fetch_all(engine.pool())
    .await?;

    Ok(balances)
}

/// Compute Balance Sheet account balances (Asset, Liability, Equity) across all periods of the year.
/// These carry forward to the next year as opening balances.
async fn compute_balance_sheet_balances(
    engine: &ErpEngine,
    periods: &[FiscalPeriod],
) -> ErpResult<Vec<AccountBalance>> {
    // For BS accounts, we need cumulative balances up to and including the fiscal year.
    // Query all posted journal lines for BS accounts across all time up to end of this year.
    let last_period_end = periods
        .last()
        .map(|p| p.end_date)
        .ok_or_else(|| ErpError::ValidationFailed {
            message: "No periods available".to_string(),
        })?;

    let balances = sqlx::query_as::<_, AccountBalance>(
        r#"SELECT
               jl.account_code,
               a.account_type,
               CASE
                   WHEN a.account_type IN ('asset', 'contra_liability', 'contra_revenue') THEN
                       COALESCE(SUM(jl.functional_debit), 0) - COALESCE(SUM(jl.functional_credit), 0)
                   ELSE
                       COALESCE(SUM(jl.functional_credit), 0) - COALESCE(SUM(jl.functional_debit), 0)
               END AS balance
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           JOIN accounts a ON a.code = jl.account_code AND a.entity_id = je.entity_id
           WHERE je.entity_id = $1
             AND je.date <= $2
             AND je.status = 'posted'
             AND a.account_type IN ('asset', 'liability', 'equity', 'contra_asset', 'contra_liability')
           GROUP BY jl.account_code, a.account_type
           HAVING CASE
               WHEN a.account_type IN ('asset', 'contra_liability', 'contra_revenue') THEN
                   COALESCE(SUM(jl.functional_debit), 0) - COALESCE(SUM(jl.functional_credit), 0)
               ELSE
                   COALESCE(SUM(jl.functional_credit), 0) - COALESCE(SUM(jl.functional_debit), 0)
               END <> 0"#,
    )
    .bind(engine.entity_id())
    .bind(last_period_end)
    .fetch_all(engine.pool())
    .await?;

    Ok(balances)
}

/// Build the closing journal entry that zeros out all P&L accounts into Retained Earnings.
///
/// - DR all Revenue accounts (to zero them out, since they have credit-normal balances)
/// - CR all Expense accounts (to zero them out, since they have debit-normal balances)
/// - Net difference goes to Retained Earnings (4600):
///   - If net income (Revenue > Expense): CR Retained Earnings
///   - If net loss (Expense > Revenue): DR Retained Earnings
async fn build_closing_entry(
    engine: &ErpEngine,
    pnl_balances: &[AccountBalance],
    last_period: &FiscalPeriod,
    req: &YearEndCloseRequest,
) -> ErpResult<CreateJournalEntryRequest> {
    let base_ccy = engine.config().base_currency.clone();
    let retained_earnings = engine.config().posting.retained_earnings.clone();
    let mut lines: Vec<CreateJournalLineRequest> = Vec::new();

    for acct in pnl_balances {
        if acct.balance == Decimal::ZERO {
            continue;
        }

        match acct.account_type.as_str() {
            "revenue" => {
                // Revenue has credit-normal balance; to close, we DR it
                lines.push(CreateJournalLineRequest {
                    account_code: acct.account_code.clone(),
                    debit: Some(acct.balance),
                    credit: None,
                    currency: base_ccy.clone(),
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!(
                        "Year-end close FY{}: close revenue account",
                        req.fiscal_year
                    )),
                    dimensions: None,
                });
            }
            "expense" => {
                // Expense has debit-normal balance; to close, we CR it
                lines.push(CreateJournalLineRequest {
                    account_code: acct.account_code.clone(),
                    debit: None,
                    credit: Some(acct.balance),
                    currency: base_ccy.clone(),
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!(
                        "Year-end close FY{}: close expense account",
                        req.fiscal_year
                    )),
                    dimensions: None,
                });
            }
            _ => {}
        }
    }

    // Compute net income: total revenue balances - total expense balances
    let net_income = compute_net_income(pnl_balances);

    // Post the net to Retained Earnings (4600)
    if net_income > Decimal::ZERO {
        // Net income: CR Retained Earnings
        lines.push(CreateJournalLineRequest {
            account_code: retained_earnings.clone(),
            debit: None,
            credit: Some(net_income),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!(
                "Year-end close FY{}: net income to retained earnings",
                req.fiscal_year
            )),
            dimensions: None,
        });
    } else if net_income < Decimal::ZERO {
        // Net loss: DR Retained Earnings
        lines.push(CreateJournalLineRequest {
            account_code: retained_earnings.clone(),
            debit: Some(net_income.abs()),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!(
                "Year-end close FY{}: net loss to retained earnings",
                req.fiscal_year
            )),
            dimensions: None,
        });
    }

    // If there are no P&L balances, we still succeed but with an empty closing entry
    if lines.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "No revenue or expense balances to close for this fiscal year".to_string(),
        });
    }

    Ok(CreateJournalEntryRequest {
        date: last_period.end_date,
        source: JournalSource::YearEndClose,
        reference: format!("YEC-{}", req.fiscal_year),
        description: format!("Year-end closing entry for fiscal year {}", req.fiscal_year),
        lines,
        post_immediately: true,
    })
}

/// Build the opening balance journal entry for the next fiscal year.
/// Carries forward all Balance Sheet account balances plus the retained earnings adjustment.
async fn build_opening_entry(
    engine: &ErpEngine,
    bs_balances: &[AccountBalance],
    first_period: &FiscalPeriod,
    req: &YearEndCloseRequest,
) -> ErpResult<CreateJournalEntryRequest> {
    let base_ccy = engine.config().base_currency.clone();
    let mut lines: Vec<CreateJournalLineRequest> = Vec::new();

    for acct in bs_balances {
        if acct.balance == Decimal::ZERO {
            continue;
        }

        // Debit-normal accounts (assets, contra_liability): positive balance = debit
        // Credit-normal accounts (liability, equity, contra_asset): positive balance = credit
        let is_debit_normal = matches!(
            acct.account_type.as_str(),
            "asset" | "contra_liability" | "contra_revenue"
        );

        if is_debit_normal {
            if acct.balance > Decimal::ZERO {
                lines.push(CreateJournalLineRequest {
                    account_code: acct.account_code.clone(),
                    debit: Some(acct.balance),
                    credit: None,
                    currency: base_ccy.clone(),
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!(
                        "Opening balance FY{}: carry forward",
                        req.fiscal_year + 1
                    )),
                    dimensions: None,
                });
            } else {
                // Negative balance on debit-normal account means credit
                lines.push(CreateJournalLineRequest {
                    account_code: acct.account_code.clone(),
                    debit: None,
                    credit: Some(acct.balance.abs()),
                    currency: base_ccy.clone(),
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!(
                        "Opening balance FY{}: carry forward",
                        req.fiscal_year + 1
                    )),
                    dimensions: None,
                });
            }
        } else {
            // Credit-normal (liability, equity, contra_asset)
            if acct.balance > Decimal::ZERO {
                lines.push(CreateJournalLineRequest {
                    account_code: acct.account_code.clone(),
                    debit: None,
                    credit: Some(acct.balance),
                    currency: base_ccy.clone(),
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!(
                        "Opening balance FY{}: carry forward",
                        req.fiscal_year + 1
                    )),
                    dimensions: None,
                });
            } else {
                // Negative balance on credit-normal account means debit
                lines.push(CreateJournalLineRequest {
                    account_code: acct.account_code.clone(),
                    debit: Some(acct.balance.abs()),
                    credit: None,
                    currency: base_ccy.clone(),
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!(
                        "Opening balance FY{}: carry forward",
                        req.fiscal_year + 1
                    )),
                    dimensions: None,
                });
            }
        }
    }

    if lines.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "No balance sheet balances to carry forward".to_string(),
        });
    }

    Ok(CreateJournalEntryRequest {
        date: first_period.start_date,
        source: JournalSource::OpeningBalance,
        reference: format!("OB-{}", req.fiscal_year + 1),
        description: format!(
            "Opening balances for fiscal year {} (carried forward from FY{})",
            req.fiscal_year + 1,
            req.fiscal_year
        ),
        lines,
        post_immediately: true,
    })
}

/// Compute net income from P&L balances: total revenue - total expenses.
fn compute_net_income(pnl_balances: &[AccountBalance]) -> Decimal {
    let total_revenue: Decimal = pnl_balances
        .iter()
        .filter(|a| a.account_type == "revenue")
        .map(|a| a.balance)
        .sum();

    let total_expense: Decimal = pnl_balances
        .iter()
        .filter(|a| a.account_type == "expense")
        .map(|a| a.balance)
        .sum();

    total_revenue - total_expense
}
