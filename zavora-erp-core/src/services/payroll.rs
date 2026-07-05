use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::parties::EmployeeRow;
use crate::period::PeriodStatus;
use crate::payroll::*;
use crate::types::AgentOrUserId;

/// Run payroll for a period — computes all employee payslips.
///
/// Validates that active employees exist for the period before computation.
/// Rejects the run if no active employees are found (R12.7).
pub async fn run_payroll(engine: &ErpEngine, entity_id: Uuid, req: RunPayrollRequest) -> ErpResult<PayRun> {
    let id = Uuid::new_v4();

    // Get active employees
    let employees = if let Some(ref ids) = req.employee_ids {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT * FROM employees WHERE entity_id = $1 AND id = ANY($2) AND is_active = true",
        )
        .bind(entity_id)
        .bind(ids)
        .fetch_all(engine.pool())
        .await?
    } else {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT * FROM employees WHERE entity_id = $1 AND is_active = true",
        )
        .bind(entity_id)
        .fetch_all(engine.pool())
        .await?
    };

    // Reject if no active employees exist for the period (R12.7)
    if employees.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "No active employees found for payroll".to_string(),
        });
    }

    // Period date range (for unpaid-leave proration).
    let period_dates: Option<(chrono::NaiveDate, chrono::NaiveDate)> = sqlx::query_as(
        "SELECT start_date, end_date FROM fiscal_periods WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.period_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?;

    // Compute payslips (R12.1 — computes gross, PAYE, NSSF, SHA, Housing Levy, HELB, net)
    let mut payslips = Vec::new();
    for emp in &employees {
        let allowances: Vec<crate::parties::Allowance> =
            serde_json::from_value(emp.allowances.clone()).unwrap_or_default();
        let allowances_total: Decimal = allowances.iter().map(|a| a.amount).sum();
        let helb = emp.helb_deduction.unwrap_or(Decimal::ZERO);
        let relief = emp.tax_relief;

        // Unpaid-leave proration: reduce basic pay by the fraction of the period's
        // working days spent on approved unpaid leave. (Configurable policy later;
        // default prorates on the period's actual working days.)
        let mut basic = emp.basic_salary;
        if let Some((from, to)) = period_dates {
            let unpaid = crate::services::leave::unpaid_leave_days(engine, entity_id, emp.id, from, to)
                .await
                .unwrap_or(Decimal::ZERO);
            if unpaid > Decimal::ZERO {
                let holidays = crate::services::leave::holiday_dates_pub(engine, entity_id, from, to)
                    .await
                    .unwrap_or_default();
                let period_working =
                    crate::hr::working_days(from, to, false, false, &holidays);
                if period_working > Decimal::ZERO {
                    let worked = (period_working - unpaid).max(Decimal::ZERO);
                    basic = (emp.basic_salary * worked / period_working).round_dp(2);
                }
            }
        }

        let deductions = compute_payslip_deductions(
            basic,
            allowances_total,
            helb,
            relief,
            emp.disability_exemption,
        );

        payslips.push(Payslip {
            id: Uuid::new_v4(),
            pay_run_id: id,
            employee_id: emp.id,
            employee_name: emp.full_name.clone(),
            deductions,
            custom_deductions: Vec::new(),
            custom_earnings: Vec::new(),
        });
    }

    let mut pay_run = PayRun {
        id,
        entity_id,
        period_id: req.period_id,
        pay_date: req.pay_date,
        payslips,
        total_gross: Decimal::ZERO,
        total_paye: Decimal::ZERO,
        total_nssf: Decimal::ZERO,
        total_sha: Decimal::ZERO,
        total_housing_levy: Decimal::ZERO,
        total_helb: Decimal::ZERO,
        total_net: Decimal::ZERO,
        status: PayRunStatus::Draft,
        journal_entry_id: None,
        created_by: req.run_by.clone(),
        created_at: Utc::now(),
        approved_by: None,
        approved_at: None,
    };
    // Recalculate totals (R12.6)
    pay_run.recalculate();

    // Persist
    sqlx::query(
        r#"INSERT INTO pay_runs 
           (id, entity_id, period_id, pay_date, total_gross, total_paye, total_nssf, total_sha, total_housing_levy, total_helb, total_net, status, created_by, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(req.period_id)
    .bind(req.pay_date)
    .bind(pay_run.total_gross)
    .bind(pay_run.total_paye)
    .bind(pay_run.total_nssf)
    .bind(pay_run.total_sha)
    .bind(pay_run.total_housing_levy)
    .bind(pay_run.total_helb)
    .bind(pay_run.total_net)
    .bind("draft")
    .bind(serde_json::to_value(&req.run_by).unwrap_or_default())
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(pay_run)
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

    let lines = vec![
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: posting.salaries_expense.clone(),
            debit: Some(pay_run.total_gross),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Gross salaries".to_string()),
            dimensions: None,
        },
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
    ];

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

    Ok(entry.id)
}

/// Mark a posted pay run as paid after salary disbursement (R14.3).
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
