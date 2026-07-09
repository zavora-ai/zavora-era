use std::collections::HashMap;

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::parties::EmployeeRow;
use crate::period::PeriodStatus;
use crate::payroll::compute::{compute_payslip, DeductionLine, EarningLine, PayrollInputs};
use crate::payroll::*;
use crate::types::AgentOrUserId;

/// Aggregate totals for a computed run.
#[derive(Default)]
struct RunTotals {
    gross: Decimal,
    paye: Decimal,
    nssf: Decimal,
    sha: Decimal,
    housing: Decimal,
    helb: Decimal,
    net: Decimal,
    employer_cost: Decimal,
    count: i32,
}

/// Resolve a pay run's period window; falls back to the pay_date's calendar month.
async fn period_window(
    engine: &ErpEngine,
    entity_id: Uuid,
    period_id: Uuid,
    pay_date: NaiveDate,
) -> (NaiveDate, NaiveDate) {
    let row: Option<(NaiveDate, NaiveDate)> = sqlx::query_as(
        "SELECT start_date, end_date FROM fiscal_periods WHERE id = $1 AND entity_id = $2",
    )
    .bind(period_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await
    .ok()
    .flatten();
    row.unwrap_or_else(|| {
        let from = NaiveDate::from_ymd_opt(pay_date.year(), pay_date.month(), 1).unwrap();
        let to = if pay_date.month() == 12 {
            NaiveDate::from_ymd_opt(pay_date.year(), 12, 31).unwrap()
        } else {
            NaiveDate::from_ymd_opt(pay_date.year(), pay_date.month() + 1, 1).unwrap().pred_opt().unwrap()
        };
        (from, to)
    })
}

/// Approved unpaid-leave working days per employee within [from,to], one query.
async fn unpaid_leave_grouped(
    engine: &ErpEngine,
    entity_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> HashMap<Uuid, Decimal> {
    let rows: Vec<(Uuid, Decimal)> = sqlx::query_as(
        r#"SELECT lr.employee_id, COALESCE(SUM(lr.working_days),0)
           FROM leave_requests lr JOIN leave_types lt ON lt.id = lr.leave_type_id
           WHERE lr.entity_id = $1 AND lr.status = 'Approved' AND lt.paid = FALSE
             AND lr.start_date <= $3 AND lr.end_date >= $2
           GROUP BY lr.employee_id"#,
    )
    .bind(entity_id).bind(from).bind(to)
    .fetch_all(engine.pool()).await.unwrap_or_default();
    rows.into_iter().collect()
}

/// Year-to-date accumulators (before this run) per employee, one query.
async fn ytd_priors(
    engine: &ErpEngine,
    entity_id: Uuid,
    run_id: Uuid,
    pay_date: NaiveDate,
) -> HashMap<Uuid, (Decimal, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal)> {
    let year_start = NaiveDate::from_ymd_opt(pay_date.year(), 1, 1).unwrap();
    let rows: Vec<(Uuid, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal)> = sqlx::query_as(
        r#"SELECT ps.employee_id,
                  COALESCE(SUM(ps.gross),0), COALESCE(SUM(ps.paye),0),
                  COALESCE(SUM(ps.nssf_employee),0), COALESCE(SUM(ps.sha),0),
                  COALESCE(SUM(ps.housing_employee),0), COALESCE(SUM(ps.helb),0),
                  COALESCE(SUM(ps.net),0)
           FROM payslips ps JOIN pay_runs pr ON pr.id = ps.pay_run_id
           WHERE pr.entity_id = $1 AND pr.status <> 'draft'
             AND pr.pay_date >= $2 AND pr.pay_date < $3 AND ps.pay_run_id <> $4
           GROUP BY ps.employee_id"#,
    )
    .bind(entity_id).bind(year_start).bind(pay_date).bind(run_id)
    .fetch_all(engine.pool()).await.unwrap_or_default();
    rows.into_iter()
        .map(|(e, g, p, n, s, h, hl, net)| (e, (g, p, n, s, h, hl, net)))
        .collect()
}

/// Compute and persist all payslips for a draft run (fresh each call — used by
/// both the initial run and recompute). Set-based bulk loads keep the query
/// count constant regardless of headcount. Returns the run totals.
async fn compute_into_run(
    engine: &ErpEngine,
    entity_id: Uuid,
    run_id: Uuid,
    period_id: Uuid,
    pay_date: NaiveDate,
    employees: &[EmployeeRow],
) -> ErpResult<RunTotals> {
    let cfg = crate::services::payroll_config::resolve(engine, entity_id, pay_date).await?;
    let (period_from, period_to) = period_window(engine, entity_id, period_id, pay_date).await;
    let holidays = crate::services::leave::holiday_dates_pub(engine, entity_id, period_from, period_to)
        .await
        .unwrap_or_default();
    let period_working = crate::hr::working_days(period_from, period_to, false, false, &holidays);

    // Bulk inputs (constant query count).
    let unpaid = unpaid_leave_grouped(engine, entity_id, period_from, period_to).await;
    let recurring = crate::services::payroll_masters::recurring_items_grouped(engine, entity_id, period_to)
        .await
        .unwrap_or_default();
    let inputs = crate::services::payroll_masters::run_inputs_grouped(engine, entity_id, run_id)
        .await
        .unwrap_or_default();
    let loans = crate::services::payroll_masters::active_loans_grouped(engine, entity_id)
        .await
        .unwrap_or_default();
    // Deduction-type attributes (pre_tax, category) by code.
    let dtypes = crate::services::payroll_masters::list_deduction_types(engine, entity_id)
        .await
        .unwrap_or_default();
    let dtype_map: HashMap<String, (bool, String)> = dtypes
        .into_iter()
        .map(|t| (t.code, (t.pre_tax, t.category)))
        .collect();
    let priors = ytd_priors(engine, entity_id, run_id, pay_date).await;

    let mut totals = RunTotals::default();
    let mut tx = engine.pool().begin().await?;

    // Fresh recompute: clear any prior payslips (and loan repayments) for this run.
    sqlx::query("DELETE FROM payslips WHERE pay_run_id = $1").bind(run_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM loan_repayments WHERE pay_run_id = $1").bind(run_id).execute(&mut *tx).await?;

    for emp in employees {
        // Earnings: base allowances (honouring taxable) + recurring + per-run inputs.
        let mut earnings: Vec<EarningLine> = Vec::new();
        let allowances: Vec<crate::parties::Allowance> =
            serde_json::from_value(emp.allowances.clone()).unwrap_or_default();
        for a in &allowances {
            earnings.push(EarningLine {
                code: None,
                name: a.name.clone(),
                amount: a.amount,
                taxable: a.taxable,
                pensionable: a.taxable,
                affects_shif: a.taxable,
            });
        }
        if let Some(items) = recurring.get(&emp.id) {
            for it in items.iter().filter(|i| i.kind == "earning") {
                let t = it.taxable.unwrap_or(true);
                earnings.push(EarningLine { code: it.type_code.clone(), name: it.name.clone(), amount: it.amount, taxable: t, pensionable: t, affects_shif: t });
            }
        }
        if let Some(items) = inputs.get(&emp.id) {
            for it in items.iter().filter(|i| i.kind == "earning") {
                earnings.push(EarningLine { code: it.type_code.clone(), name: it.name.clone(), amount: it.amount, taxable: it.taxable, pensionable: it.taxable, affects_shif: it.taxable });
            }
        }

        // Proration: joiner/leaver window intersected with the period, less unpaid leave.
        let eff_from = emp.start_date.max(period_from);
        let eff_to = emp.end_date.map(|e| e.min(period_to)).unwrap_or(period_to);
        let worked_working = if eff_to >= eff_from {
            crate::hr::working_days(eff_from, eff_to, false, false, &holidays)
        } else {
            Decimal::ZERO
        };
        let unpaid_days = unpaid.get(&emp.id).copied().unwrap_or(Decimal::ZERO);
        let worked = (worked_working - unpaid_days).max(Decimal::ZERO);
        let basic = if period_working > Decimal::ZERO && worked < period_working {
            (emp.basic_salary * worked / period_working).round_dp(2)
        } else {
            emp.basic_salary
        };

        // Deductions: recurring + per-run voluntary/welfare + active-loan installments.
        let mut deductions: Vec<DeductionLine> = Vec::new();
        let resolve_ded = |type_code: &Option<String>| -> (bool, String) {
            type_code
                .as_ref()
                .and_then(|c| dtype_map.get(c))
                .map(|(p, cat)| (*p, cat.clone()))
                .unwrap_or((false, "voluntary".to_string()))
        };
        if let Some(items) = recurring.get(&emp.id) {
            for it in items.iter().filter(|i| i.kind == "deduction") {
                let (pre_tax, category) = resolve_ded(&it.type_code);
                deductions.push(DeductionLine { code: it.type_code.clone(), name: it.name.clone(), amount: it.amount, pre_tax, category });
            }
        }
        if let Some(items) = inputs.get(&emp.id) {
            for it in items.iter().filter(|i| i.kind == "deduction") {
                let (pre_tax, category) = resolve_ded(&it.type_code);
                deductions.push(DeductionLine { code: it.type_code.clone(), name: it.name.clone(), amount: it.amount, pre_tax, category });
            }
        }
        if let Some(emp_loans) = loans.get(&emp.id) {
            for ln in emp_loans {
                let inst = ln.installment.min(ln.balance);
                if inst > Decimal::ZERO {
                    // code carries the loan id so posting can amortize the right loan.
                    deductions.push(DeductionLine { code: Some(ln.id.to_string()), name: ln.name.clone(), amount: inst, pre_tax: false, category: "loan".to_string() });
                }
            }
        }

        // Insurance relief (ITA s.31): 15% of life/health/education-policy
        // premiums, capped at KES 5,000/month (the compute engine enforces the
        // cap). Premiums are the deduction lines whose type category is
        // "insurance" — set the category on the deduction type master.
        let insurance_premiums: Decimal = deductions
            .iter()
            .filter(|d| d.category.eq_ignore_ascii_case("insurance"))
            .map(|d| d.amount)
            .sum();
        let insurance_relief = (insurance_premiums * Decimal::new(15, 2)).round_dp(2);

        let inp = PayrollInputs {
            basic_salary: basic,
            earnings,
            deductions,
            helb: emp.helb_deduction.unwrap_or(Decimal::ZERO),
            personal_relief: emp.tax_relief,
            insurance_relief,
            disability_exemption: emp.disability_exemption,
        };
        let c = compute_payslip(&cfg, &inp);

        // YTD = priors + this run.
        let (pg, pp, pn, ps_, ph, phl, pnet) = priors.get(&emp.id).copied().unwrap_or_default();
        let ytd = serde_json::json!({
            "gross": pg + c.gross,
            "paye": pp + c.net_paye,
            "nssf": pn + c.nssf_employee,
            "sha": ps_ + c.sha,
            "housing": ph + c.housing_levy_employee,
            "helb": phl + c.helb,
            "net": pnet + c.net_salary,
        });

        // Back-compat deductions blob (payslip PDF + ESS read this).
        let deductions = PayslipDeductions {
            gross_salary: c.gross,
            taxable_income: c.taxable_income,
            paye: c.paye,
            personal_relief: c.personal_relief,
            insurance_relief: c.insurance_relief,
            net_paye: c.net_paye,
            nssf_employee: c.nssf_employee,
            nssf_employer: c.nssf_employer,
            sha: c.sha,
            housing_levy_employee: c.housing_levy_employee,
            housing_levy_employer: c.housing_levy_employer,
            helb: c.helb,
            total_deductions: c.total_deductions,
            net_salary: c.net_salary,
        };

        sqlx::query(
            r#"INSERT INTO payslips
               (id, pay_run_id, employee_id, deductions, custom_deductions, custom_earnings,
                employee_name, staff_number, kra_pin, department_id,
                gross, taxable, paye, nssf_employee, nssf_employer, sha,
                housing_employee, housing_employer, helb, total_deductions, net,
                earnings, deductions_detail, ytd)
               VALUES ($1,$2,$3,$4,'[]'::jsonb,'[]'::jsonb,
                       $5,$6,$7,$8,
                       $9,$10,$11,$12,$13,$14,
                       $15,$16,$17,$18,$19,
                       $20,$21,$22)"#,
        )
        .bind(Uuid::new_v4()).bind(run_id).bind(emp.id)
        .bind(serde_json::to_value(&deductions).unwrap_or_default())
        .bind(&emp.full_name).bind(&emp.staff_number).bind(&emp.kra_pin).bind(emp.department_id)
        .bind(c.gross).bind(c.taxable_income).bind(c.net_paye).bind(c.nssf_employee).bind(c.nssf_employer).bind(c.sha)
        .bind(c.housing_levy_employee).bind(c.housing_levy_employer).bind(c.helb).bind(c.total_deductions).bind(c.net_salary)
        .bind(serde_json::to_value(&c.earnings).unwrap_or_else(|_| serde_json::json!([])))
        .bind(serde_json::to_value(&c.deductions).unwrap_or_else(|_| serde_json::json!([])))
        .bind(ytd)
        .execute(&mut *tx)
        .await?;

        totals.gross += c.gross;
        totals.paye += c.net_paye;
        totals.nssf += c.nssf_employee + c.nssf_employer;
        totals.sha += c.sha;
        totals.housing += c.housing_levy_employee + c.housing_levy_employer;
        totals.helb += c.helb;
        totals.net += c.net_salary;
        totals.employer_cost += c.employer_cost;
        totals.count += 1;
    }

    tx.commit().await?;
    Ok(totals)
}

/// Persist the run totals computed by [`compute_into_run`].
async fn store_totals(engine: &ErpEngine, run_id: Uuid, t: &RunTotals) -> ErpResult<()> {
    sqlx::query(
        "UPDATE pay_runs SET total_gross=$2, total_paye=$3, total_nssf=$4, total_sha=$5, \
         total_housing_levy=$6, total_helb=$7, total_net=$8, employee_count=$9, total_employer_cost=$10 \
         WHERE id=$1",
    )
    .bind(run_id).bind(t.gross).bind(t.paye).bind(t.nssf).bind(t.sha).bind(t.housing)
    .bind(t.helb).bind(t.net).bind(t.count).bind(t.employer_cost)
    .execute(engine.pool())
    .await?;
    Ok(())
}

/// Load a pay run (header + payslips) as a `PayRun` for the API.
pub async fn load_pay_run(engine: &ErpEngine, entity_id: Uuid, run_id: Uuid) -> ErpResult<PayRun> {
    let r = sqlx::query_as::<_, PayRunRow>("SELECT * FROM pay_runs WHERE id=$1 AND entity_id=$2")
        .bind(run_id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "PayRun".into(), id: run_id })?;

    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT id, employee_id, employee_name, deductions FROM payslips WHERE pay_run_id=$1 ORDER BY employee_name",
    )
    .bind(run_id).fetch_all(engine.pool()).await?;
    let payslips = rows.iter().map(|row| {
        let deductions: PayslipDeductions = serde_json::from_value(row.get::<serde_json::Value, _>("deductions")).unwrap_or(PayslipDeductions {
            gross_salary: Decimal::ZERO, taxable_income: Decimal::ZERO, paye: Decimal::ZERO,
            personal_relief: Decimal::ZERO, insurance_relief: Decimal::ZERO, net_paye: Decimal::ZERO,
            nssf_employee: Decimal::ZERO, nssf_employer: Decimal::ZERO, sha: Decimal::ZERO,
            housing_levy_employee: Decimal::ZERO, housing_levy_employer: Decimal::ZERO, helb: Decimal::ZERO,
            total_deductions: Decimal::ZERO, net_salary: Decimal::ZERO,
        });
        Payslip {
            id: row.get::<Uuid, _>("id"),
            pay_run_id: run_id,
            employee_id: row.get::<Uuid, _>("employee_id"),
            employee_name: row.get::<Option<String>, _>("employee_name").unwrap_or_default(),
            deductions,
            custom_deductions: Vec::new(),
            custom_earnings: Vec::new(),
        }
    }).collect();

    let status = match r.status.as_str() {
        "approved" => PayRunStatus::Approved,
        "posted" => PayRunStatus::Posted,
        "paid" => PayRunStatus::Paid,
        _ => PayRunStatus::Draft,
    };
    let created_by: AgentOrUserId = serde_json::from_value(r.created_by.clone())
        .unwrap_or(AgentOrUserId::Agent("system".into()));
    let approved_by: Option<AgentOrUserId> = r.approved_by.clone().and_then(|v| serde_json::from_value(v).ok());

    Ok(PayRun {
        id: r.id,
        entity_id: r.entity_id,
        period_id: r.period_id,
        pay_date: r.pay_date,
        payslips,
        total_gross: r.total_gross,
        total_paye: r.total_paye,
        total_nssf: r.total_nssf,
        total_sha: r.total_sha,
        total_housing_levy: r.total_housing_levy,
        total_helb: r.total_helb,
        total_net: r.total_net,
        status,
        journal_entry_id: r.journal_entry_id,
        created_by,
        created_at: r.created_at,
        approved_by,
        approved_at: r.approved_at,
    })
}

/// Run payroll for a period — creates a **draft** run and computes every active
/// employee's payslip (config-driven, effective-dated, itemized). Variable
/// per-run inputs can then be added and the draft recomputed before approval.
pub async fn run_payroll(engine: &ErpEngine, entity_id: Uuid, req: RunPayrollRequest) -> ErpResult<PayRun> {
    // One open draft per period — edit/recompute or delete it instead of stacking.
    let existing_draft: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM pay_runs WHERE entity_id=$1 AND period_id=$2 AND status='draft' LIMIT 1",
    )
    .bind(entity_id).bind(req.period_id).fetch_optional(engine.pool()).await?;
    if existing_draft.is_some() {
        return Err(ErpError::ValidationFailed {
            message: "A draft pay run already exists for this period. Edit or delete it first.".into(),
        });
    }

    let employees = if let Some(ref ids) = req.employee_ids {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT * FROM employees WHERE entity_id = $1 AND id = ANY($2) AND is_active = true",
        )
        .bind(entity_id).bind(ids).fetch_all(engine.pool()).await?
    } else {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT * FROM employees WHERE entity_id = $1 AND is_active = true",
        )
        .bind(entity_id).fetch_all(engine.pool()).await?
    };
    if employees.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "No active employees found for payroll".to_string(),
        });
    }

    // Lazy-seed config + master types so admins can edit them.
    let _ = crate::services::payroll_config::ensure_seeded(engine, entity_id).await;
    let _ = crate::services::payroll_masters::seed_default_types(engine, entity_id).await;

    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO pay_runs (id, entity_id, period_id, pay_date, status, created_by, created_at)
           VALUES ($1,$2,$3,$4,'draft',$5,$6)"#,
    )
    .bind(id).bind(entity_id).bind(req.period_id).bind(req.pay_date)
    .bind(serde_json::to_value(&req.run_by).unwrap_or_default()).bind(Utc::now())
    .execute(engine.pool()).await?;

    let totals = compute_into_run(engine, entity_id, id, req.period_id, req.pay_date, &employees).await?;
    store_totals(engine, id, &totals).await?;

    load_pay_run(engine, entity_id, id).await
}

/// Recompute a **draft** run's payslips (picks up newly-added per-run inputs and
/// master/config changes). Rejected once the run leaves draft.
pub async fn recompute_pay_run(engine: &ErpEngine, entity_id: Uuid, run_id: Uuid) -> ErpResult<PayRun> {
    let r = sqlx::query_as::<_, PayRunRow>("SELECT * FROM pay_runs WHERE id=$1 AND entity_id=$2")
        .bind(run_id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "PayRun".into(), id: run_id })?;
    if r.status != "draft" {
        return Err(ErpError::ValidationFailed { message: "Only draft pay runs can be recomputed".into() });
    }
    // Recompute the same employee set that the draft covers.
    let emp_ids: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT employee_id FROM payslips WHERE pay_run_id=$1")
        .bind(run_id).fetch_all(engine.pool()).await?;
    let employees = sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE entity_id=$1 AND id = ANY($2)",
    )
    .bind(entity_id).bind(&emp_ids).fetch_all(engine.pool()).await?;

    let totals = compute_into_run(engine, entity_id, run_id, r.period_id, r.pay_date, &employees).await?;
    store_totals(engine, run_id, &totals).await?;
    load_pay_run(engine, entity_id, run_id).await
}

/// A pay run header row for the history list.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct PayRunListRow {
    pub id: Uuid,
    pub period_id: Uuid,
    pub pay_date: NaiveDate,
    pub status: String,
    pub employee_count: i32,
    pub total_gross: Decimal,
    pub total_net: Decimal,
    pub total_employer_cost: Decimal,
    pub created_at: chrono::DateTime<Utc>,
}

/// List pay runs (history), newest first.
pub async fn list_pay_runs(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<PayRunListRow>> {
    Ok(sqlx::query_as::<_, PayRunListRow>(
        "SELECT id, period_id, pay_date, status, employee_count, total_gross, total_net, \
         total_employer_cost, created_at FROM pay_runs WHERE entity_id=$1 ORDER BY pay_date DESC, created_at DESC",
    )
    .bind(entity_id).fetch_all(engine.pool()).await?)
}

/// Delete a draft pay run (and its payslips/inputs via FK cascade). Draft only.
pub async fn delete_draft_pay_run(engine: &ErpEngine, entity_id: Uuid, run_id: Uuid) -> ErpResult<()> {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM pay_runs WHERE id=$1 AND entity_id=$2")
        .bind(run_id).bind(entity_id).fetch_optional(engine.pool()).await?;
    match status.as_deref() {
        Some("draft") => {}
        Some(_) => return Err(ErpError::ValidationFailed { message: "Only draft pay runs can be deleted".into() }),
        None => return Err(ErpError::NotFound { entity_type: "PayRun".into(), id: run_id }),
    }
    let mut tx = engine.pool().begin().await?;
    sqlx::query("DELETE FROM payslips WHERE pay_run_id=$1").bind(run_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM loan_repayments WHERE pay_run_id=$1").bind(run_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM pay_run_inputs WHERE pay_run_id=$1").bind(run_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM pay_runs WHERE id=$1 AND entity_id=$2").bind(run_id).bind(entity_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Approve a pay run (R13.1).
///
/// State transition: Draft → Approved.
/// Rejects if pay run is not in Draft status.
pub async fn approve_pay_run(engine: &ErpEngine, entity_id: Uuid, req: ApprovePayRunRequest) -> ErpResult<()> {
    let pay_run = sqlx::query_as::<_, PayRunRow>(
        "SELECT * FROM pay_runs WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.pay_run_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "PayRun".to_string(),
        id: req.pay_run_id,
    })?;

    // Validate state transition: only Draft → Approved
    if pay_run.status != "draft" {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Pay run must be in Draft status to approve, current status: {}",
                pay_run.status
            ),
        });
    }

    sqlx::query(
        "UPDATE pay_runs SET status = 'approved', approved_by = $1, approved_at = $2 WHERE id = $3 AND entity_id = $4",
    )
    .bind(serde_json::to_value(&req.approved_by).unwrap_or_default())
    .bind(Utc::now())
    .bind(req.pay_run_id)
    .bind(entity_id)
    .execute(engine.pool())
    .await?;
    Ok(())
}

/// Post a pay run — creates GL journal entry (R13.2, R13.3, R13.5).
///
/// State transition: Approved → Posted.
/// Validates:
/// - Pay run is in Approved status
/// - Fiscal period for pay_date is Open (R13.5)
///
/// Creates a consolidated journal entry per the design:
/// DR 7010 Salaries (total_gross)
/// DR 7020 Employer NSSF (employer_nssf)
/// DR 7030 Employer Housing Levy (employer_hl)
/// CR 3310 PAYE Payable (total_paye)
/// CR 3320 NSSF Payable (total_nssf = employee + employer)
/// CR 3330 SHA Payable (total_sha)
/// CR 3340 HELB Payable (total_helb)
/// CR 3350 Housing Levy Payable (total_housing_levy = employee + employer)
/// CR 3400 Net Pay Payable (total_net)
pub async fn post_pay_run(
    engine: &ErpEngine,
    entity_id: Uuid,
    pay_run_id: Uuid,
    posted_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let pay_run = sqlx::query_as::<_, PayRunRow>(
        "SELECT * FROM pay_runs WHERE id = $1 AND entity_id = $2",
    )
    .bind(pay_run_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "PayRun".to_string(),
        id: pay_run_id,
    })?;

    // Validate state transition: only Approved → Posted
    if pay_run.status != "approved" {
        return Err(ErpError::ValidationFailed {
            message: "Pay run must be approved before posting".to_string(),
        });
    }

    // Validate fiscal period for pay_date is Open before posting (R13.5)
    let period = crate::services::periods::period_for_date(engine, entity_id, pay_run.pay_date).await?;
    if period.parsed_status() != PeriodStatus::Open {
        return Err(ErpError::PeriodClosedDetailed {
            period_name: period.name.clone(),
            status: format!("{:?}", period.parsed_status()),
            period_id: period.id,
        });
    }

    // Build consolidated journal entry (R13.2):
    // DR Salaries & Wages (7010) — total gross
    // DR Employer NSSF (7020) — employer NSSF
    // DR Employer Housing Levy (7030) — employer housing levy
    // CR PAYE Payable (3310) — total PAYE
    // CR NSSF Payable (3320) — employee + employer NSSF
    // CR SHA Payable (3330) — total SHA
    // CR HELB Payable (3340) — total HELB
    // CR Housing Levy Payable (3350) — employee + employer housing levy
    // CR Net Salary Payable (3400) — total net

    let base_ccy = engine.config_for(entity_id).await?.base_currency.clone();
    let posting = engine.posting_for(entity_id).await?;
    // NSSF total includes both employee + employer; employer portion is half
    let employer_nssf = pay_run.total_nssf / Decimal::new(2, 0);
    let employer_hl = pay_run.total_housing_levy / Decimal::new(2, 0);

    // Salaries expense split by department and dimension-tagged, so labour cost
    // can be analysed per cost centre. The split sums to total_gross → balanced.
    let dept_gross: Vec<(Option<String>, Decimal)> = sqlx::query_as(
        "SELECT d.code, SUM(ps.gross) FROM payslips ps \
         LEFT JOIN departments d ON d.id = ps.department_id \
         WHERE ps.pay_run_id = $1 GROUP BY d.code",
    )
    .bind(pay_run_id)
    .fetch_all(engine.pool())
    .await
    .unwrap_or_default();

    let mut lines: Vec<crate::ledger::journal::CreateJournalLineRequest> = Vec::new();
    for (dept, amt) in &dept_gross {
        if *amt == Decimal::ZERO {
            continue;
        }
        lines.push(crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.salaries_expense.clone(),
            debit: Some(*amt),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(match dept {
                Some(c) => format!("Gross salaries — {c}"),
                None => "Gross salaries".to_string(),
            }),
            dimensions: dept.clone().map(|c| HashMap::from([("Department".to_string(), c)])),
        });
    }
    if lines.is_empty() {
        lines.push(crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.salaries_expense.clone(),
            debit: Some(pay_run.total_gross),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Gross salaries".to_string()),
            dimensions: None,
        });
    }
    lines.extend(vec![
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.nssf_employer_expense.clone(),
            debit: Some(employer_nssf),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Employer NSSF contribution".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.housing_levy_employer_expense.clone(),
            debit: Some(employer_hl),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Employer housing levy".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.paye_payable.clone(),
            debit: None,
            credit: Some(pay_run.total_paye),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("PAYE payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.nssf_payable.clone(),
            debit: None,
            credit: Some(pay_run.total_nssf),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("NSSF payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.sha_payable.clone(),
            debit: None,
            credit: Some(pay_run.total_sha),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("SHA payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.helb_payable.clone(),
            debit: None,
            credit: Some(pay_run.total_helb),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("HELB payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.housing_levy_payable.clone(),
            debit: None,
            credit: Some(pay_run.total_housing_levy),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Housing levy payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.net_pay_payable.clone(),
            debit: None,
            credit: Some(pay_run.total_net),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Net salary payable".to_string()),
            dimensions: None,
        },
    ]);

    // Voluntary/loan deductions withheld from net pay: credit each to its mapped
    // liability account (deduction_types.gl_account_code), falling back to net
    // pay payable. This keeps the entry balanced now that net excludes them.
    let ded_rows: Vec<(Option<String>, Option<String>, Decimal)> = sqlx::query_as(
        "SELECT d->>'code', d->>'category', SUM((d->>'amount')::numeric) \
         FROM payslips ps, LATERAL jsonb_array_elements(ps.deductions_detail) d \
         WHERE ps.pay_run_id = $1 GROUP BY d->>'code', d->>'category'",
    )
    .bind(pay_run_id)
    .fetch_all(engine.pool())
    .await
    .unwrap_or_default();
    if !ded_rows.is_empty() {
        let dtypes = crate::services::payroll_masters::list_deduction_types(engine, entity_id)
            .await
            .unwrap_or_default();
        let acct_map: std::collections::HashMap<String, Option<String>> =
            dtypes.into_iter().map(|t| (t.code, t.gl_account_code)).collect();
        let mut by_account: std::collections::HashMap<String, Decimal> = std::collections::HashMap::new();
        for (code, _cat, amt) in &ded_rows {
            if *amt <= Decimal::ZERO {
                continue;
            }
            let account = code
                .as_ref()
                .and_then(|c| acct_map.get(c).cloned().flatten())
                .unwrap_or_else(|| posting.net_pay_payable.clone());
            *by_account.entry(account).or_insert(Decimal::ZERO) += *amt;
        }
        for (account, amt) in by_account {
            lines.push(crate::ledger::journal::CreateJournalLineRequest {
                account_code: account,
                debit: None,
                credit: Some(amt),
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some("Payroll deductions withheld".to_string()),
                dimensions: None,
            });
        }
    }

    let entry_req = crate::ledger::journal::CreateJournalEntryRequest {
        date: pay_run.pay_date,
        source: crate::ledger::journal::JournalSource::Payroll,
        source_id: Some(pay_run.id),
        reference: format!("PAYROLL-{}", pay_run.pay_date),
        description: format!("Payroll for {}", pay_run.pay_date),
        lines,
        post_immediately: true,
    };

    // create_and_post also enforces period status internally (defence-in-depth)
    let entry = crate::services::journal::create_and_post(engine, entity_id, entry_req, period.id, posted_by.clone()).await?;

    // Transition: Approved → Posted (R13.3)
    sqlx::query("UPDATE pay_runs SET status = 'posted', journal_entry_id = $1 WHERE id = $2")
        .bind(entry.id)
        .bind(pay_run_id)
        .execute(engine.pool())
        .await?;

    // Loan amortization: record each loan repayment and decrement the balance
    // (idempotent via UNIQUE(loan_id, pay_run_id)).
    let loan_rows: Vec<(String, Decimal)> = sqlx::query_as(
        "SELECT d->>'code', SUM((d->>'amount')::numeric) \
         FROM payslips ps, LATERAL jsonb_array_elements(ps.deductions_detail) d \
         WHERE ps.pay_run_id = $1 AND d->>'category' = 'loan' GROUP BY d->>'code'",
    )
    .bind(pay_run_id)
    .fetch_all(engine.pool())
    .await
    .unwrap_or_default();
    for (code, amt) in loan_rows {
        let Ok(loan_id) = Uuid::parse_str(&code) else { continue };
        let bal: Option<Decimal> = sqlx::query_scalar(
            "SELECT balance FROM employee_loans WHERE id = $1 AND entity_id = $2",
        )
        .bind(loan_id).bind(entity_id).fetch_optional(engine.pool()).await.ok().flatten();
        if let Some(b) = bal {
            let new_bal = (b - amt).max(Decimal::ZERO);
            let _ = sqlx::query(
                "UPDATE employee_loans SET balance = $2, \
                 status = CASE WHEN $2 <= 0 THEN 'settled' ELSE status END WHERE id = $1",
            )
            .bind(loan_id).bind(new_bal).execute(engine.pool()).await;
            let _ = sqlx::query(
                "INSERT INTO loan_repayments (id, entity_id, loan_id, pay_run_id, amount, balance_after) \
                 VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (loan_id, pay_run_id) DO NOTHING",
            )
            .bind(Uuid::new_v4()).bind(entity_id).bind(loan_id).bind(pay_run_id).bind(amt).bind(new_bal)
            .execute(engine.pool()).await;
        }
    }

    // Best-effort: email each employee their payslip PDF.
    email_payslips(engine, entity_id, pay_run_id).await;

    Ok(entry.id)
}

/// Email every employee on a pay run their payslip PDF (best-effort; skips
/// employees without an email; never fails the post).
async fn email_payslips(engine: &ErpEngine, entity_id: Uuid, pay_run_id: Uuid) {
    use base64::Engine as _;
    let rows = sqlx::query_scalar::<_, Uuid>(
        "SELECT employee_id FROM payslips WHERE pay_run_id = $1",
    )
    .bind(pay_run_id)
    .fetch_all(engine.pool())
    .await
    .unwrap_or_default();

    for emp_id in rows {
        // Recipient: ESS login email, else personal email.
        let email: Option<String> = sqlx::query_scalar(
            "SELECT COALESCE((SELECT email FROM employee_users WHERE employee_id = e.id AND entity_id = e.entity_id LIMIT 1), e.personal_email) \
             FROM employees e WHERE e.id = $1 AND e.entity_id = $2",
        )
        .bind(emp_id).bind(entity_id).fetch_optional(engine.pool()).await.ok().flatten().flatten();
        let Some(email) = email else { continue };

        let pdf = match payslip_pdf(engine, entity_id, pay_run_id, emp_id).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        let req = crate::notifications::SendNotificationRequest {
            event_type: crate::notifications::NotificationEventType::PayRunApprovalNeeded,
            channels: vec![crate::types::Channel::Email],
            recipients: vec![email],
            subject: Some("Your payslip".into()),
            body: "Please find your payslip attached.".into(),
            related_type: Some("pay_run".into()),
            related_id: Some(pay_run_id),
            schedule_at: None,
            attachments: vec![crate::notifications::NotificationAttachment {
                filename: "payslip.pdf".into(),
                mime_type: "application/pdf".into(),
                content_base64: base64::engine::general_purpose::STANDARD.encode(&pdf),
            }],
        };
        let _ = crate::services::notifications::send_notification(engine, entity_id, req).await;
    }
}
///
/// State transition: Posted → Paid.
/// Rejects if pay run is not in Posted status.
pub async fn mark_pay_run_paid(
    engine: &ErpEngine,
    entity_id: Uuid,
    pay_run_id: Uuid,
    paid_by: &AgentOrUserId,
) -> ErpResult<()> {
    let pay_run = sqlx::query_as::<_, PayRunRow>(
        "SELECT * FROM pay_runs WHERE id = $1 AND entity_id = $2",
    )
    .bind(pay_run_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "PayRun".to_string(),
        id: pay_run_id,
    })?;

    // Validate state transition: only Posted → Paid
    if pay_run.status != "posted" {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Pay run must be in Posted status to mark as paid, current status: {}",
                pay_run.status
            ),
        });
    }

    sqlx::query("UPDATE pay_runs SET status = 'paid' WHERE id = $1 AND entity_id = $2")
        .bind(pay_run_id)
        .bind(entity_id)
        .execute(engine.pool())
        .await?;

    // Record audit event
    let audit_event = serde_json::json!({
        "event_type": "PayRunPaid",
        "object_type": "pay_run",
        "object_id": pay_run_id,
        "actor": paid_by,
        "timestamp": Utc::now(),
    });
    let stream_key = format!("erp:audit:{}", entity_id);
    let mut redis_conn = engine.redis_conn().await;
    let _: Result<(), _> = redis::cmd("XADD")
        .arg(&stream_key)
        .arg("*")
        .arg("data")
        .arg(audit_event.to_string())
        .query_async(&mut redis_conn)
        .await;

    Ok(())
}

/// Build a payslip PDF for a specific (pay_run, employee). Reads the persisted
/// payslip deductions + employee + company name. Returns PDF bytes.
pub async fn payslip_pdf(
    engine: &ErpEngine,
    entity_id: Uuid,
    pay_run_id: Uuid,
    employee_id: Uuid,
) -> ErpResult<Vec<u8>> {
    use sqlx::Row;
    let row = sqlx::query(
        r#"SELECT ps.deductions, ps.earnings, ps.deductions_detail, ps.ytd,
                  pr.pay_date, e.full_name, e.staff_number, e.kra_pin
           FROM payslips ps
           JOIN pay_runs pr ON pr.id = ps.pay_run_id
           JOIN employees e ON e.id = ps.employee_id
           WHERE ps.pay_run_id = $1 AND ps.employee_id = $2 AND pr.entity_id = $3"#,
    )
    .bind(pay_run_id)
    .bind(employee_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Payslip".into(), id: pay_run_id })?;

    let deductions: crate::payroll::statutory::PayslipDeductions =
        serde_json::from_value(row.get::<serde_json::Value, _>("deductions"))
            .map_err(|e| ErpError::Internal(format!("payslip decode: {e}")))?;
    let pay_date: chrono::NaiveDate = row.get("pay_date");
    let company_name = engine
        .config_for(entity_id)
        .await
        .map(|c| c.branding.company_name.clone())
        .unwrap_or_else(|_| "Company".to_string());

    // Itemized earnings & other deductions (empty for pre-denormalized runs).
    let earnings: Vec<crate::payroll::compute::EarningLine> =
        serde_json::from_value(row.get::<serde_json::Value, _>("earnings")).unwrap_or_default();
    let other: Vec<crate::payroll::compute::DeductionLine> =
        serde_json::from_value(row.get::<serde_json::Value, _>("deductions_detail")).unwrap_or_default();
    let ytd = row.get::<serde_json::Value, _>("ytd");
    let ytd_dec = |k: &str| -> Decimal {
        ytd.get(k).and_then(|v| serde_json::from_value::<Decimal>(v.clone()).ok()).unwrap_or(Decimal::ZERO)
    };

    let d = crate::payroll::payslip_pdf::PayslipPdfData {
        company_name,
        employee_name: row.get::<String, _>("full_name"),
        staff_number: row.get::<String, _>("staff_number"),
        kra_pin: row.get::<String, _>("kra_pin"),
        pay_date: pay_date.to_string(),
        period_label: pay_date.format("%B %Y").to_string(),
        earnings: earnings.into_iter().map(|e| (e.name, e.amount)).collect(),
        gross_salary: deductions.gross_salary,
        taxable_income: deductions.taxable_income,
        paye: deductions.paye,
        personal_relief: deductions.personal_relief,
        net_paye: deductions.net_paye,
        nssf_employee: deductions.nssf_employee,
        nssf_employer: deductions.nssf_employer,
        sha: deductions.sha,
        housing_levy_employee: deductions.housing_levy_employee,
        housing_levy_employer: deductions.housing_levy_employer,
        helb: deductions.helb,
        other_deductions: other.into_iter().map(|x| (x.name, x.amount)).collect(),
        total_deductions: deductions.total_deductions,
        net_salary: deductions.net_salary,
        ytd_gross: ytd_dec("gross"),
        ytd_paye: ytd_dec("paye"),
        ytd_net: ytd_dec("net"),
    };
    Ok(crate::payroll::payslip_pdf::render_payslip_pdf(&d))
}

/// KRA iTax P10 employee-details CSV (`B_Employees_Dtls`) for one pay run —
/// importable into the iTax PAYE return workbook. Columns follow the official
/// template order; fields the system doesn't track (car/housing benefits,
/// mortgage interest…) are emitted as 0/blank, and residency/employment type
/// default to Resident / Primary Employee — edit in the workbook for the
/// exceptions before upload.
pub async fn itax_p10_csv(engine: &ErpEngine, entity_id: Uuid, run_id: Uuid) -> ErpResult<Vec<u8>> {
    // Tenant-scope the run before exporting anything.
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pay_runs WHERE id=$1 AND entity_id=$2)",
    )
    .bind(run_id)
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?;
    if !exists {
        return Err(ErpError::NotFound { entity_type: "PayRun".into(), id: run_id });
    }

    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT employee_name, kra_pin, gross, taxable, nssf_employee, deductions, earnings
         FROM payslips WHERE pay_run_id=$1 ORDER BY employee_name",
    )
    .bind(run_id)
    .fetch_all(engine.pool())
    .await?;

    let esc = |s: &str| {
        if s.contains(',') || s.contains('"') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };

    let mut out = Vec::new();
    out.extend_from_slice(
        b"PIN of Employee,Name of Employee,Residential Status,Type of Employee,\
Basic Salary,Housing Allowance,Transport Allowance,Leave Pay,Overtime Allowance,\
Directors Fee,Lump Sum Payment if any,Other Allowances,Total Cash Pay,\
Value of Car Benefit,Other Non Cash Benefits,Total Non Cash Pay,Global Income,\
Type of Housing,Rent of House,Computed Rent of House,Rent Recovered from Employee,\
Net Value of Housing,Total Gross Pay,30% of Cash Pay,Actual Contribution,\
Permissible Limit,Mortgage Interest,Deposit on Home Ownership Plan,\
Amount of Benefit,Taxable Pay,Tax Payable,Monthly Personal Relief,\
Amount of Insurance Relief,PAYE Tax,Self Assessed PAYE\n",
    );

    for row in &rows {
        let name: String = row.get::<Option<String>, _>("employee_name").unwrap_or_default();
        let pin: String = row.get::<Option<String>, _>("kra_pin").unwrap_or_default();
        let gross: Decimal = row.get("gross");
        let taxable: Decimal = row.get("taxable");
        let nssf: Decimal = row.get("nssf_employee");
        let d: PayslipDeductions = serde_json::from_value(row.get::<serde_json::Value, _>("deductions"))
            .unwrap_or_else(|_| serde_json::from_value(serde_json::json!({})).unwrap_or(PayslipDeductions {
                gross_salary: Decimal::ZERO, taxable_income: Decimal::ZERO, paye: Decimal::ZERO,
                personal_relief: Decimal::ZERO, insurance_relief: Decimal::ZERO, net_paye: Decimal::ZERO,
                nssf_employee: Decimal::ZERO, nssf_employer: Decimal::ZERO, sha: Decimal::ZERO,
                housing_levy_employee: Decimal::ZERO, housing_levy_employer: Decimal::ZERO,
                helb: Decimal::ZERO, total_deductions: Decimal::ZERO, net_salary: Decimal::ZERO,
            }));
        // Allowance lines are the earnings JSONB; basic = gross - allowances.
        let earnings: Vec<serde_json::Value> =
            serde_json::from_value(row.get::<serde_json::Value, _>("earnings")).unwrap_or_default();
        let allowances: Decimal = earnings
            .iter()
            .filter_map(|e| e.get("amount").and_then(|a| a.as_str()).and_then(|a| a.parse::<Decimal>().ok()))
            .sum();
        let basic = (gross - allowances).max(Decimal::ZERO);
        let thirty_pct = (gross * Decimal::new(30, 2)).round_dp(2);
        // Pension relief permissible limit (ITA): KES 20,000/month.
        let permissible = Decimal::new(20_000, 0).min(nssf.max(thirty_pct.min(Decimal::new(20_000, 0))));

        let line = format!(
            "{pin},{name},Resident,Primary Employee,{basic},0,0,0,0,0,0,{allowances},{gross},0,0,0,0,Benefit not given,0,0,0,0,{gross},{thirty},{nssf},{permissible},0,0,0,{taxable},{tax},{prelief},{irelief},{paye},0\n",
            pin = esc(&pin),
            name = esc(&name),
            basic = basic,
            allowances = allowances,
            gross = gross,
            thirty = thirty_pct,
            nssf = nssf,
            permissible = permissible,
            taxable = taxable,
            tax = d.paye,
            prelief = d.personal_relief,
            irelief = d.insurance_relief,
            paye = d.net_paye,
        );
        out.extend_from_slice(line.as_bytes());
    }

    Ok(out)
}
