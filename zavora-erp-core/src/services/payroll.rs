use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::parties::EmployeeRow;
use crate::payroll::*;
use crate::types::AgentOrUserId;

/// Run payroll for a period — computes all employee payslips.
pub async fn run_payroll(engine: &ErpEngine, req: RunPayrollRequest) -> ErpResult<PayRun> {
    let id = Uuid::new_v4();

    // Get active employees
    let employees = if let Some(ref ids) = req.employee_ids {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT * FROM employees WHERE entity_id = $1 AND id = ANY($2) AND is_active = true",
        )
        .bind(engine.entity_id())
        .bind(ids)
        .fetch_all(engine.pool())
        .await?
    } else {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT * FROM employees WHERE entity_id = $1 AND is_active = true",
        )
        .bind(engine.entity_id())
        .fetch_all(engine.pool())
        .await?
    };

    if employees.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "No active employees found for payroll".to_string(),
        });
    }

    // Compute payslips
    let mut payslips = Vec::new();
    for emp in &employees {
        let allowances: Vec<crate::parties::Allowance> =
            serde_json::from_value(emp.allowances.clone()).unwrap_or_default();
        let allowances_total: Decimal = allowances.iter().map(|a| a.amount).sum();
        let helb = emp.helb_deduction.unwrap_or(Decimal::ZERO);
        let relief = emp.tax_relief;

        let deductions = compute_payslip_deductions(
            emp.basic_salary,
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
        entity_id: engine.entity_id(),
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
    pay_run.recalculate();

    // Persist
    sqlx::query(
        r#"INSERT INTO pay_runs 
           (id, entity_id, period_id, pay_date, total_gross, total_paye, total_nssf, total_sha, total_housing_levy, total_helb, total_net, status, created_by, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"#,
    )
    .bind(id)
    .bind(engine.entity_id())
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

/// Approve a pay run.
pub async fn approve_pay_run(engine: &ErpEngine, req: ApprovePayRunRequest) -> ErpResult<()> {
    sqlx::query(
        "UPDATE pay_runs SET status = 'approved', approved_by = $1, approved_at = $2 WHERE id = $3 AND entity_id = $4",
    )
    .bind(serde_json::to_value(&req.approved_by).unwrap_or_default())
    .bind(Utc::now())
    .bind(req.pay_run_id)
    .bind(engine.entity_id())
    .execute(engine.pool())
    .await?;
    Ok(())
}

/// Post a pay run — creates GL journal entry.
pub async fn post_pay_run(
    engine: &ErpEngine,
    pay_run_id: Uuid,
    posted_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let pay_run = sqlx::query_as::<_, PayRunRow>(
        "SELECT * FROM pay_runs WHERE id = $1 AND entity_id = $2",
    )
    .bind(pay_run_id)
    .bind(engine.entity_id())
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "PayRun".to_string(),
        id: pay_run_id,
    })?;

    if pay_run.status != "approved" {
        return Err(ErpError::ValidationFailed {
            message: "Pay run must be approved before posting".to_string(),
        });
    }

    // Build journal entry:
    // DR Salaries & Wages (7010) — total gross
    // DR Employer NSSF (7020) — employer NSSF
    // DR Employer Housing Levy (7030) — employer housing levy
    // CR Net Salary Payable (3400) — total net
    // CR PAYE Payable (3310) — total PAYE
    // CR NSSF Payable (3320) — employee + employer NSSF
    // CR SHA Payable (3330) — total SHA
    // CR HELB Payable (3340) — total HELB
    // CR Housing Levy Payable (3350) — employee + employer housing levy

    let base_ccy = engine.config().base_currency.clone();
    let employer_nssf = pay_run.total_nssf / Decimal::new(2, 0); // half is employer
    let employer_hl = pay_run.total_housing_levy / Decimal::new(2, 0);

    let lines = vec![
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: "7010".to_string(),
            debit: Some(pay_run.total_gross),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Gross salaries".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: "7020".to_string(),
            debit: Some(employer_nssf),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Employer NSSF contribution".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: "7030".to_string(),
            debit: Some(employer_hl),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Employer housing levy".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: "3400".to_string(),
            debit: None,
            credit: Some(pay_run.total_net),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Net salary payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: "3310".to_string(),
            debit: None,
            credit: Some(pay_run.total_paye),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("PAYE payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: "3320".to_string(),
            debit: None,
            credit: Some(pay_run.total_nssf),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("NSSF payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: "3330".to_string(),
            debit: None,
            credit: Some(pay_run.total_sha),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("SHA payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: "3340".to_string(),
            debit: None,
            credit: Some(pay_run.total_helb),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("HELB payable".to_string()),
            dimensions: None,
        },
        crate::ledger::journal::CreateJournalLineRequest {
            account_code: "3350".to_string(),
            debit: None,
            credit: Some(pay_run.total_housing_levy),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Housing levy payable".to_string()),
            dimensions: None,
        },
    ];

    let entry_req = crate::ledger::journal::CreateJournalEntryRequest {
        date: pay_run.pay_date,
        source: crate::ledger::journal::JournalSource::Payroll,
        reference: format!("PAYROLL-{}", pay_run.pay_date),
        description: format!("Payroll for {}", pay_run.pay_date),
        lines,
        post_immediately: true,
    };

    let period = crate::services::periods::period_for_date(engine, pay_run.pay_date).await?;
    let entry = crate::services::journal::create_and_post(engine, entry_req, period.id, posted_by.clone()).await?;

    sqlx::query("UPDATE pay_runs SET status = 'posted', journal_entry_id = $1 WHERE id = $2")
        .bind(entry.id)
        .bind(pay_run_id)
        .execute(engine.pool())
        .await?;

    Ok(entry.id)
}
