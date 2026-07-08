//! Cash-flow forecast: a deterministic 13-week rolling view built from the
//! documents the ledger already holds — open AR/AP by due date, unremitted
//! statutory filings, and the payroll cycle. This is the non-AI counterpart
//! of Amos's cash-forecast skill: same inputs, fixed rules, no judgement.
//!
//! Rules (stated in `assumptions` so the reader knows what the model did):
//! - Overdue receivables ≤ 90 days slot into week 1; > 90 days are EXCLUDED
//!   from inflows and reported separately (hope is not a forecast).
//! - Overdue payables slot into week 1 (creditors don't wait politely).
//! - Filed-but-unremitted tax lands in week 1 (KRA first).
//! - Payroll recurs monthly on the last run's pay-day at the last run's total
//!   cost (gross + employer statutory).

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ForecastWeek {
    pub week_start: NaiveDate,
    pub week_end: NaiveDate,
    pub inflows: Decimal,
    pub outflows: Decimal,
    pub net: Decimal,
    pub closing: Decimal,
}

#[derive(Debug, Serialize)]
pub struct CashForecast {
    pub as_of: NaiveDate,
    pub opening_cash: Decimal,
    pub weeks: Vec<ForecastWeek>,
    /// Receivables overdue > 90 days — excluded from the forecast.
    pub excluded_overdue_ar: Decimal,
    /// The first week whose closing balance is negative, if any.
    pub first_negative_week: Option<NaiveDate>,
    pub assumptions: Vec<String>,
}

/// Week index (0-based) for a cash event dated `d`: overdue/near items land in
/// week 0; anything beyond the horizon is ignored by the caller.
fn week_index(d: NaiveDate, start: NaiveDate) -> i64 {
    ((d - start).num_days().max(0)) / 7
}

pub async fn cash_forecast(engine: &ErpEngine, entity_id: Uuid, weeks: usize) -> ErpResult<CashForecast> {
    let weeks = weeks.clamp(4, 26);
    let today = chrono::Utc::now().date_naive();
    // Monday of the current week anchors the buckets.
    let start = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let horizon = start + Duration::days((weeks * 7) as i64);

    let opening_cash: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0)
           FROM journal_lines jl
           WHERE jl.entity_id = $1
             AND jl.account_code IN (
                 SELECT gl_account FROM bank_accounts WHERE entity_id = $1 AND is_active = true)"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    let mut inflow = vec![Decimal::ZERO; weeks];
    let mut outflow = vec![Decimal::ZERO; weeks];
    let mut excluded_overdue_ar = Decimal::ZERO;

    // AR: open invoice balances by due date.
    let invoices: Vec<(Option<NaiveDate>, Decimal)> = sqlx::query_as(
        r#"SELECT due_date, balance_due FROM invoices
           WHERE entity_id = $1 AND balance_due > 0
             AND status NOT IN ('draft', 'cancelled', 'voided', 'written_off')"#,
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await
    .unwrap_or_default();
    for (due, amount) in invoices {
        let due = due.unwrap_or(today);
        if due < today - Duration::days(90) {
            excluded_overdue_ar += amount;
            continue;
        }
        if due < horizon {
            inflow[week_index(due, start) as usize % weeks.max(1)] += amount;
        }
    }

    // AP: open bill balances by due date (overdue → week 0).
    let bills: Vec<(Option<NaiveDate>, Decimal)> = sqlx::query_as(
        r#"SELECT due_date, balance_due FROM bills
           WHERE entity_id = $1 AND balance_due > 0
             AND status NOT IN ('draft', 'cancelled', 'voided')"#,
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await
    .unwrap_or_default();
    for (due, amount) in bills {
        let due = due.unwrap_or(today);
        if due < horizon {
            outflow[week_index(due, start) as usize % weeks.max(1)] += amount;
        }
    }

    // Statutory: filed-but-unremitted tax — week 0 (KRA first).
    let unremitted: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(amount), 0) FROM tax_filings
           WHERE entity_id = $1 AND status = 'filed'"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);
    outflow[0] += unremitted;

    // Payroll: the last run's total cost, monthly on its pay-day.
    let last_run: Option<(NaiveDate, Decimal)> = sqlx::query_as(
        r#"SELECT pay_date,
                  total_gross + total_nssf + total_sha + total_housing_levy AS total_cost
           FROM pay_runs
           WHERE entity_id = $1 AND status IN ('approved', 'posted', 'paid')
           ORDER BY pay_date DESC LIMIT 1"#,
    )
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await
    .unwrap_or(None);
    if let Some((pay_date, cost)) = last_run {
        if cost > Decimal::ZERO {
            let pay_day = pay_date.day().min(28);
            let mut d = NaiveDate::from_ymd_opt(today.year(), today.month(), pay_day).unwrap();
            for _ in 0..=(weeks / 4 + 1) {
                if d >= today && d < horizon {
                    outflow[week_index(d, start) as usize % weeks.max(1)] += cost;
                }
                let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) };
                d = NaiveDate::from_ymd_opt(y, m, pay_day).unwrap();
            }
        }
    }

    let mut closing = opening_cash;
    let mut first_negative_week = None;
    let weeks_out: Vec<ForecastWeek> = (0..weeks)
        .map(|i| {
            let ws = start + Duration::days((i * 7) as i64);
            let net = inflow[i] - outflow[i];
            closing += net;
            if closing < Decimal::ZERO && first_negative_week.is_none() {
                first_negative_week = Some(ws);
            }
            ForecastWeek {
                week_start: ws,
                week_end: ws + Duration::days(6),
                inflows: inflow[i],
                outflows: outflow[i],
                net,
                closing,
            }
        })
        .collect();

    Ok(CashForecast {
        as_of: today,
        opening_cash,
        weeks: weeks_out,
        excluded_overdue_ar,
        first_negative_week,
        assumptions: vec![
            "Overdue receivables within 90 days are assumed to arrive this week; older ones are excluded (shown separately).".into(),
            "Overdue payables and filed-but-unremitted tax are due immediately (week 1).".into(),
            "Payroll recurs monthly at the last run's total cost on its usual pay-day.".into(),
            "Only documented amounts are forecast — undocumented recurring costs (rent paid without bills, new sales) are not included.".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn week_bucketing() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 6).unwrap(); // a Monday
        assert_eq!(week_index(start, start), 0);
        assert_eq!(week_index(start + Duration::days(6), start), 0);
        assert_eq!(week_index(start + Duration::days(7), start), 1);
        // Overdue clamps to week 0.
        assert_eq!(week_index(start - Duration::days(30), start), 0);
        assert_eq!(week_index(start + Duration::days(90), start), 12);
    }
}
