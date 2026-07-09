//! Amortisation service: create prepayment/deferred-revenue schedules and post
//! their monthly installments to the ledger.
//!
//! Each schedule spreads a total over `periods` months from `start_date`. The
//! run is idempotent and catches up: it books every installment whose month has
//! arrived but isn't yet posted (tracked by `amortized_periods`), stopping at
//! the first month with no open fiscal period — mirroring the depreciation run.
//! Equal installments with the rounding remainder absorbed by the final period,
//! so the schedule fully clears the holding account.

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::amortization::*;
use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::types::AgentOrUserId;

/// First day of the month `n` months after `start`.
fn month_start_plus(start: NaiveDate, n: u32) -> NaiveDate {
    let m0 = start.month0() + n;
    let year = start.year() + (m0 / 12) as i32;
    let month = m0 % 12 + 1;
    NaiveDate::from_ymd_opt(year, month, 1).unwrap()
}

/// Create an amortisation schedule. Validates the accounts exist and are active.
pub async fn create_schedule(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateScheduleRequest,
    created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    if req.periods == 0 {
        return Err(ErpError::ValidationFailed { message: "Periods must be at least 1.".to_string() });
    }
    if req.total_amount <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed { message: "Total amount must be positive.".to_string() });
    }
    if req.balance_account == req.pnl_account {
        return Err(ErpError::ValidationFailed {
            message: "Balance-sheet and P&L accounts must differ.".to_string(),
        });
    }
    for code in [&req.balance_account, &req.pnl_account] {
        let ok: Option<bool> = sqlx::query_scalar(
            "SELECT is_active FROM accounts WHERE entity_id = $1 AND code = $2",
        )
        .bind(entity_id)
        .bind(code)
        .fetch_optional(engine.pool())
        .await?;
        match ok {
            Some(true) => {}
            Some(false) => return Err(ErpError::ValidationFailed { message: format!("Account {code} is inactive.") }),
            None => return Err(ErpError::ValidationFailed { message: format!("Account {code} not found.") }),
        }
    }

    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO amortization_schedules
           (id, entity_id, kind, description, balance_account, pnl_account, total_amount, periods, start_date, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(req.kind.as_str())
    .bind(&req.description)
    .bind(&req.balance_account)
    .bind(&req.pnl_account)
    .bind(req.total_amount)
    .bind(req.periods as i32)
    .bind(req.start_date)
    .bind(serde_json::to_value(created_by).ok())
    .execute(engine.pool())
    .await?;
    Ok(id)
}

/// Run amortisation for all active schedules up to (and including) the month of
/// `as_of`, catching up any installments not yet posted. Returns the schedule
/// IDs that had at least one installment posted.
pub async fn run_amortization(
    engine: &ErpEngine,
    entity_id: Uuid,
    as_of: NaiveDate,
    triggered_by: &AgentOrUserId,
) -> ErpResult<Vec<Uuid>> {
    let base_ccy = engine.config_for(entity_id).await?.base_currency.clone();
    let schedules = sqlx::query_as::<_, ScheduleRow>(
        "SELECT * FROM amortization_schedules WHERE entity_id = $1 AND status = 'active'",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    let mut touched = Vec::new();
    for sched in schedules {
        let periods = sched.periods.max(0) as u32;
        let per_installment = (sched.total_amount / Decimal::from(periods.max(1))).round_dp(2);
        let mut posted_any = false;

        let mut idx = sched.amortized_periods.max(0) as u32;
        while idx < periods {
            let period_month = month_start_plus(sched.start_date, idx);
            // Only post installments whose month has arrived.
            if period_month > as_of {
                break;
            }
            // Last installment absorbs the rounding remainder so the schedule
            // clears the holding account exactly.
            let amount = if idx + 1 == periods {
                sched.total_amount - per_installment * Decimal::from(periods - 1)
            } else {
                per_installment
            };
            if amount <= Decimal::ZERO {
                idx += 1;
                continue;
            }

            let kind = AmortizationKind::from_str(&sched.kind).unwrap_or(AmortizationKind::PrepaidExpense);
            // Prepaid: DR expense (P&L) / CR prepaid (BS).
            // Deferred rev: DR deferred-rev (BS) / CR revenue (P&L).
            let (dr, cr) = match kind {
                AmortizationKind::PrepaidExpense => (sched.pnl_account.clone(), sched.balance_account.clone()),
                AmortizationKind::DeferredRevenue => (sched.balance_account.clone(), sched.pnl_account.clone()),
            };
            let lines = vec![
                CreateJournalLineRequest {
                    account_code: dr,
                    debit: Some(amount),
                    credit: None,
                    currency: base_ccy.clone(),
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!("{} — period {}/{}", sched.description, idx + 1, periods)),
                    dimensions: None,
                },
                CreateJournalLineRequest {
                    account_code: cr,
                    debit: None,
                    credit: Some(amount),
                    currency: base_ccy.clone(),
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!("{} — period {}/{}", sched.description, idx + 1, periods)),
                    dimensions: None,
                },
            ];
            let entry_req = CreateJournalEntryRequest {
                date: period_month,
                source: JournalSource::Manual,
                source_id: Some(sched.id),
                reference: format!("AMORT-{}-{}", &sched.id.to_string()[..8], idx + 1),
                description: format!("Amortisation: {}", sched.description),
                lines,
                post_immediately: true,
            };

            // Stop catch-up at the first month with no open period (don't error
            // the whole run — later schedules/months just wait).
            let period = match crate::services::periods::period_for_date(engine, entity_id, period_month).await {
                Ok(p) => p,
                Err(_) => break,
            };
            match crate::services::journal::create_and_post(engine, entity_id, entry_req, period.id, triggered_by.clone()).await {
                Ok(_) => {}
                Err(_) => break,
            }

            idx += 1;
            posted_any = true;
        }

        if posted_any || idx as i32 != sched.amortized_periods {
            let status = if idx >= periods { "completed" } else { "active" };
            sqlx::query("UPDATE amortization_schedules SET amortized_periods = $1, status = $2 WHERE id = $3")
                .bind(idx as i32)
                .bind(status)
                .bind(sched.id)
                .execute(engine.pool())
                .await?;
            if posted_any {
                touched.push(sched.id);
            }
        }
    }
    Ok(touched)
}

/// Run amortisation for every tenant (scheduler entry point). Returns the total
/// number of schedules that posted an installment.
pub async fn run_all(engine: &ErpEngine) -> ErpResult<u32> {
    let entities: Vec<Uuid> =
        sqlx::query_scalar("SELECT DISTINCT entity_id FROM amortization_schedules WHERE status = 'active'")
            .fetch_all(engine.pool())
            .await?;
    let today = Utc::now().date_naive();
    let mut total = 0u32;
    for entity_id in entities {
        let actor = AgentOrUserId::Agent("scheduler".to_string());
        if let Ok(touched) = run_amortization(engine, entity_id, today, &actor).await {
            total += touched.len() as u32;
        }
    }
    Ok(total)
}

pub async fn list_schedules(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<ScheduleRow>> {
    Ok(sqlx::query_as::<_, ScheduleRow>(
        "SELECT * FROM amortization_schedules WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?)
}

/// Cancel a schedule (stops future installments; posted ones stand).
pub async fn cancel_schedule(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<()> {
    let n = sqlx::query("UPDATE amortization_schedules SET status = 'cancelled' WHERE id = $1 AND entity_id = $2 AND status = 'active'")
        .bind(id)
        .bind(entity_id)
        .execute(engine.pool())
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ErpError::NotFound { entity_type: "AmortizationSchedule".to_string(), id });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_stepping_wraps_year() {
        let start = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert_eq!(month_start_plus(start, 0), start);
        assert_eq!(month_start_plus(start, 2), NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
        assert_eq!(month_start_plus(start, 13), NaiveDate::from_ymd_opt(2027, 12, 1).unwrap());
    }
}
