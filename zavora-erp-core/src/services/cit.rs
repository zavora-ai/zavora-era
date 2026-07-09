//! Corporation tax (Kenya): the installment-tax calendar and a CIT estimate.
//!
//! Computes an **estimate** of the year's corporation tax from the ledger —
//! accounting profit (P&L) + book-depreciation add-back − capital allowances
//! from the fixed-asset register — and lays out the statutory installment
//! schedule (20th of the 4th, 6th, 9th and 12th months of the accounting
//! period; balance of tax by the end of the 4th month after year end).
//! Installment payments are recorded through the existing tax-filings module
//! (`tax_type` starting with `CIT`), which this estimate reads back to show
//! what's paid vs due.
//!
//! This is decision-support, not a tax computation of record: capital
//! allowances use configurable Second-Schedule default rates (straight line)
//! and the caller can pass a manual `adjustments` figure for non-deductibles /
//! other allowances the ledger can't see. iTax remains the filing of record.

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::reporting::{ReportContent, ReportParameters, ReportRequest, ReportType};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use serde::Serialize;
use uuid::Uuid;

/// Resident corporate income tax rate (Kenya).
const CIT_RATE_PERCENT: u32 = 30;

/// Second-Schedule (post-2021) wear-and-tear defaults, straight line, percent
/// per annum by asset category. Deliberately conservative; the accountant
/// adjusts via `adjustments` where a class qualifies for more (e.g. 50%
/// first-year investment deduction on manufacturing machinery).
fn allowance_rate_percent(category: &str) -> u32 {
    match category {
        "LandAndBuildings" => 10,
        "MotorVehicles" => 25,
        "PlantAndMachinery" => 25,
        "FurnitureAndFittings" => 10,
        "ComputerEquipment" => 25,
        "Software" => 25,
        _ => 10,
    }
}

#[derive(Debug, Serialize)]
pub struct CitInstallment {
    pub label: String,
    pub due_date: NaiveDate,
    /// Cumulative share of the year's estimate due by this date (25/50/75/100).
    pub cumulative_percent: u32,
    pub amount: Decimal,
    pub status: String, // "paid" | "due" | "upcoming"
}

#[derive(Debug, Serialize)]
pub struct CitEstimate {
    pub fiscal_year_start: NaiveDate,
    pub fiscal_year_end: NaiveDate,
    pub accounting_profit: Decimal,
    /// Book depreciation added back (postings to the depreciation-expense account).
    pub depreciation_add_back: Decimal,
    /// Estimated capital allowances from the fixed-asset register (defaults above).
    pub capital_allowances: Decimal,
    /// Caller-supplied manual adjustment to taxable profit (±).
    pub adjustments: Decimal,
    pub taxable_profit_estimate: Decimal,
    pub cit_rate_percent: u32,
    pub estimated_tax: Decimal,
    /// CIT payments recorded in tax filings for this fiscal year.
    pub paid_to_date: Decimal,
    pub installments: Vec<CitInstallment>,
    /// Balance of tax deadline: end of the 4th month after year end.
    pub balance_due_date: NaiveDate,
    pub notes: Vec<String>,
}

/// The fiscal year (start, end) ending in calendar year `ending_year`, given
/// the entity's fiscal-year-end month/day.
pub fn fiscal_window(fy_end_month: u32, fy_end_day: u32, ending_year: i32) -> (NaiveDate, NaiveDate) {
    let end = NaiveDate::from_ymd_opt(ending_year, fy_end_month, fy_end_day)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(ending_year, 12, 31).unwrap());
    // Start = the day after the same month/day one year earlier
    // (Feb-29 year-ends fall back to Feb-28 in non-leap years).
    let prior_end = NaiveDate::from_ymd_opt(ending_year - 1, fy_end_month, fy_end_day)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(ending_year - 1, fy_end_month, 28).unwrap());
    (prior_end.succ_opt().unwrap(), end)
}

/// Installment due dates: the 20th of the 4th, 6th, 9th and 12th months of
/// the accounting period (Income Tax Act, s.12).
pub fn installment_dates(fy_start: NaiveDate) -> [(String, NaiveDate, u32); 4] {
    let nth = |months: u32| {
        let total = fy_start.month0() + months - 1;
        let year = fy_start.year() + (total / 12) as i32;
        let month = total % 12 + 1;
        NaiveDate::from_ymd_opt(year, month, 20).unwrap()
    };
    [
        ("1st installment (25%)".into(), nth(4), 25),
        ("2nd installment (50%)".into(), nth(6), 50),
        ("3rd installment (75%)".into(), nth(9), 75),
        ("4th installment (100%)".into(), nth(12), 100),
    ]
}

/// End of the 4th month after the fiscal year end (balance-of-tax deadline).
pub fn balance_due(fy_end: NaiveDate) -> NaiveDate {
    let total = fy_end.month0() + 5; // first day of the 5th month after…
    let year = fy_end.year() + (total / 12) as i32;
    let month = total % 12 + 1;
    NaiveDate::from_ymd_opt(year, month, 1).unwrap().pred_opt().unwrap() // …minus one day
}

/// Straight-line annual allowance for one asset within the fiscal year,
/// stopping once the cost is fully written down.
fn asset_allowance(cost: Decimal, rate_percent: u32, acquired: NaiveDate, fy_start: NaiveDate, fy_end: NaiveDate) -> Decimal {
    if acquired > fy_end || cost <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let rate = Decimal::from(rate_percent) / Decimal::from(100);
    let annual = cost * rate;
    // Whole fiscal years already claimed before this one (acquisition year
    // counts as year 1 — Kenya W&T is not prorated for part years).
    let years_before = (fy_start.year() - acquired.year()).max(0) as i64;
    let claimed = annual * Decimal::from(years_before);
    if claimed >= cost {
        return Decimal::ZERO;
    }
    annual.min(cost - claimed)
}

pub async fn estimate(
    engine: &ErpEngine,
    entity_id: Uuid,
    ending_year: Option<i32>,
    adjustments: Decimal,
) -> ErpResult<CitEstimate> {
    let config = super::settings::get_settings(engine, entity_id).await?;
    let today = chrono::Utc::now().date_naive();
    let fy_end_md = &config.fiscal_year_end;

    // Default: the fiscal year containing today.
    let ending_year = ending_year.unwrap_or_else(|| {
        let this_years_end =
            NaiveDate::from_ymd_opt(today.year(), fy_end_md.month, fy_end_md.day).unwrap_or(today);
        if today <= this_years_end { today.year() } else { today.year() + 1 }
    });
    let (fy_start, fy_end) = fiscal_window(fy_end_md.month, fy_end_md.day, ending_year);

    // Accounting profit for the window (ledger-true).
    let report = super::reporting::generate_report(
        engine,
        ReportRequest {
            entity_id,
            report_type: ReportType::ProfitAndLoss,
            parameters: ReportParameters {
                period_from: Some(fy_start),
                period_to: Some(fy_end.min(today)),
                ..Default::default()
            },
        },
    )
    .await?;
    let accounting_profit = match report.content {
        ReportContent::ProfitAndLoss(p) => p.net_profit,
        _ => Decimal::ZERO,
    };

    // Book depreciation posted in the window (added back; replaced by W&T).
    let depreciation_add_back: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(COALESCE(functional_debit,0) - COALESCE(functional_credit,0)), 0)
           FROM journal_lines
           WHERE entity_id = $1 AND account_code = $2 AND entry_date BETWEEN $3 AND $4"#,
    )
    .bind(entity_id)
    .bind(&config.posting.depreciation_expense)
    .bind(fy_start)
    .bind(fy_end)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    // Capital allowances from the asset register at default class rates.
    let assets: Vec<(String, Decimal, NaiveDate)> = sqlx::query_as(
        r#"SELECT category, cost, acquisition_date FROM fixed_assets
           WHERE entity_id = $1 AND acquisition_date <= $2"#,
    )
    .bind(entity_id)
    .bind(fy_end)
    .fetch_all(engine.pool())
    .await
    .unwrap_or_default();
    let capital_allowances: Decimal = assets
        .iter()
        .map(|(cat, cost, acq)| asset_allowance(*cost, allowance_rate_percent(cat), *acq, fy_start, fy_end))
        .sum();

    let taxable = (accounting_profit + depreciation_add_back - capital_allowances + adjustments)
        .max(Decimal::ZERO);
    let rate = Decimal::from(CIT_RATE_PERCENT) / Decimal::from(100);
    let estimated_tax = (taxable * rate).round_dp(2);

    // CIT payments already recorded for this fiscal year.
    let paid_to_date: Decimal = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(amount), 0) FROM tax_filings
           WHERE entity_id = $1 AND UPPER(tax_type) LIKE 'CIT%'
             AND period_from >= $2 AND period_to <= $3 AND status = 'remitted'"#,
    )
    .bind(entity_id)
    .bind(fy_start)
    .bind(fy_end)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    let quarter = Decimal::from_f64(0.25).unwrap();
    let installments = installment_dates(fy_start)
        .into_iter()
        .map(|(label, due, cum)| {
            let cumulative = (estimated_tax * Decimal::from(cum) / Decimal::from(100)).round_dp(2);
            let status = if paid_to_date >= cumulative && cumulative > Decimal::ZERO {
                "paid"
            } else if due <= today {
                "due"
            } else {
                "upcoming"
            };
            CitInstallment {
                label,
                due_date: due,
                cumulative_percent: cum,
                amount: (estimated_tax * quarter).round_dp(2),
                status: status.into(),
            }
        })
        .collect();

    Ok(CitEstimate {
        fiscal_year_start: fy_start,
        fiscal_year_end: fy_end,
        accounting_profit,
        depreciation_add_back,
        capital_allowances: capital_allowances.round_dp(2),
        adjustments,
        taxable_profit_estimate: taxable.round_dp(2),
        cit_rate_percent: CIT_RATE_PERCENT,
        estimated_tax,
        paid_to_date,
        installments,
        balance_due_date: balance_due(fy_end),
        notes: vec![
            "Estimate for decision support — iTax is the filing of record.".into(),
            "Capital allowances use Second-Schedule default straight-line rates per asset category; pass `adjustments` for investment deductions, disallowables, or loss carry-forwards.".into(),
            "Installment basis: current-year estimate. If using the 110%-of-prior-year basis, compare with last year's assessed tax.".into(),
            "Record installment payments as tax filings with tax_type 'CIT-installment' so this schedule tracks them.".into(),
        ],
    })
}

/// Result of posting a CIT provision.
#[derive(Debug, serde::Serialize)]
pub struct CitProvisionResult {
    pub journal_entry_id: uuid::Uuid,
    pub journal_number: String,
    pub fiscal_year_end: NaiveDate,
    /// The year's estimated tax at posting time.
    pub estimated_tax: rust_decimal::Decimal,
    /// Provision already on the books for this year before this posting.
    pub previously_provided: rust_decimal::Decimal,
    /// The incremental amount this posting booked.
    pub provided_now: rust_decimal::Decimal,
}

/// Book the corporation-tax provision for a fiscal year:
/// `DR 8500 Corporate Income Tax / CR 3510 Corporation Tax Payable`.
///
/// Incremental true-up: the amount defaults to the current estimate less what
/// prior `CIT-PROV-…` entries already provided for the year, so re-running
/// after profits move books only the difference. `amount_override` books an
/// exact figure instead (e.g. the tax agent's computation). Remitting the tax
/// stays in the tax-filings flow — this books the expense/liability only.
pub async fn post_provision(
    engine: &ErpEngine,
    entity_id: uuid::Uuid,
    ending_year: Option<i32>,
    adjustments: rust_decimal::Decimal,
    amount_override: Option<rust_decimal::Decimal>,
    posted_by: &crate::types::AgentOrUserId,
) -> ErpResult<CitProvisionResult> {
    use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
    use rust_decimal::Decimal;

    let est = estimate(engine, entity_id, ending_year, adjustments).await?;
    let fy_label = est.fiscal_year_end.year();

    // What earlier provision entries already booked for this year.
    let previously_provided: Decimal = sqlx::query_scalar::<_, Option<Decimal>>(
        r#"SELECT SUM(COALESCE(jl.functional_credit,0) - COALESCE(jl.functional_debit,0))
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
             AND je.reference LIKE $2 AND jl.account_code = '3510'"#,
    )
    .bind(entity_id)
    .bind(format!("CIT-PROV-{fy_label}%"))
    .fetch_one(engine.pool())
    .await?
    .unwrap_or(Decimal::ZERO);

    let amount = match amount_override {
        Some(a) => a,
        None => (est.estimated_tax - previously_provided).round_dp(2),
    };
    if amount <= Decimal::ZERO {
        return Err(crate::error::ErpError::ValidationFailed {
            message: format!(
                "Nothing to provide: estimate {} vs {} already provided for FY{fy_label}. Pass an explicit amount to adjust.",
                est.estimated_tax, previously_provided
            ),
        });
    }

    // Post within the fiscal year (true-ups after year end land on the last
    // day of the year — reopen or allow documents if that period is locked).
    let today = chrono::Utc::now().date_naive();
    let date = today.clamp(est.fiscal_year_start, est.fiscal_year_end);
    let base_ccy = engine.config_for(entity_id).await?.base_currency.clone();
    let reference = format!("CIT-PROV-{fy_label}-{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let entry_req = CreateJournalEntryRequest {
        date,
        source: JournalSource::Manual,
        source_id: None,
        reference: reference.clone(),
        description: format!("Corporation tax provision FY{fy_label} ({} of est. {})", amount, est.estimated_tax),
        lines: vec![
            CreateJournalLineRequest {
                account_code: "8500".to_string(),
                debit: Some(amount),
                credit: None,
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("CIT provision FY{fy_label}")),
                dimensions: None,
            },
            CreateJournalLineRequest {
                account_code: "3510".to_string(),
                debit: None,
                credit: Some(amount),
                currency: base_ccy,
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("Corporation tax payable FY{fy_label}")),
                dimensions: None,
            },
        ],
        post_immediately: true,
    };
    let period = crate::services::periods::period_for_date(engine, entity_id, date).await?;
    let entry = crate::services::journal::create_and_post(engine, entity_id, entry_req, period.id, posted_by.clone()).await?;

    Ok(CitProvisionResult {
        journal_entry_id: entry.id,
        journal_number: entry.number,
        fiscal_year_end: est.fiscal_year_end,
        estimated_tax: est.estimated_tax,
        previously_provided,
        provided_now: amount,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fiscal_window_calendar_year() {
        let (s, e) = fiscal_window(12, 31, 2026);
        assert_eq!(s, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(e, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
    }

    #[test]
    fn fiscal_window_june_year_end() {
        let (s, e) = fiscal_window(6, 30, 2026);
        assert_eq!(s, NaiveDate::from_ymd_opt(2025, 7, 1).unwrap());
        assert_eq!(e, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap());
    }

    #[test]
    fn installment_calendar_matches_ita_s12() {
        // Calendar-year FY: 20 Apr / 20 Jun / 20 Sep / 20 Dec.
        let d = installment_dates(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(d[0].1, NaiveDate::from_ymd_opt(2026, 4, 20).unwrap());
        assert_eq!(d[1].1, NaiveDate::from_ymd_opt(2026, 6, 20).unwrap());
        assert_eq!(d[2].1, NaiveDate::from_ymd_opt(2026, 9, 20).unwrap());
        assert_eq!(d[3].1, NaiveDate::from_ymd_opt(2026, 12, 20).unwrap());
        // July–June FY: 20 Oct / 20 Dec / 20 Mar / 20 Jun.
        let d = installment_dates(NaiveDate::from_ymd_opt(2025, 7, 1).unwrap());
        assert_eq!(d[0].1, NaiveDate::from_ymd_opt(2025, 10, 20).unwrap());
        assert_eq!(d[3].1, NaiveDate::from_ymd_opt(2026, 6, 20).unwrap());
    }

    #[test]
    fn balance_of_tax_end_of_fourth_month() {
        assert_eq!(
            balance_due(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
            NaiveDate::from_ymd_opt(2027, 4, 30).unwrap()
        );
        assert_eq!(
            balance_due(NaiveDate::from_ymd_opt(2026, 6, 30).unwrap()),
            NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()
        );
    }

    #[test]
    fn allowance_straight_line_stops_at_cost() {
        let cost = Decimal::from(100_000);
        let acq = NaiveDate::from_ymd_opt(2023, 3, 1).unwrap();
        let fy = |y| fiscal_window(12, 31, y);
        // 25%/yr: 2023–2026 full 25k, 2027 zero (fully written down).
        for y in 2023..=2026 {
            let (s, e) = fy(y);
            assert_eq!(asset_allowance(cost, 25, acq, s, e), Decimal::from(25_000), "year {y}");
        }
        let (s, e) = fy(2027);
        assert_eq!(asset_allowance(cost, 25, acq, s, e), Decimal::ZERO);
    }
}
