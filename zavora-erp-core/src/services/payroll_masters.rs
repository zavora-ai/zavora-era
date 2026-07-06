//! Payroll masters & variable-input persistence: earning/deduction types,
//! departments, employee recurring items, per-run inputs, and loans. Includes
//! batch load helpers (grouped by employee / run) used by the payroll engine so
//! a run over thousands of employees issues a constant number of queries.

use std::collections::HashMap;

use chrono::NaiveDate;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::payroll::masters::*;

// ─── Earning types ───────────────────────────────────────────────────────────

pub async fn list_earning_types(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<EarningTypeRow>> {
    Ok(sqlx::query_as::<_, EarningTypeRow>(
        "SELECT * FROM earning_types WHERE entity_id = $1 ORDER BY sequence, name",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?)
}

pub async fn create_earning_type(engine: &ErpEngine, entity_id: Uuid, req: CreateEarningTypeRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO earning_types (id, entity_id, code, name, taxable, pensionable, affects_shif, proratable, gl_account_code, sequence) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (entity_id, code) DO NOTHING",
    )
    .bind(id).bind(entity_id).bind(&req.code).bind(&req.name).bind(req.taxable).bind(req.pensionable)
    .bind(req.affects_shif).bind(req.proratable).bind(&req.gl_account_code).bind(req.sequence)
    .execute(engine.pool()).await?;
    Ok(id)
}

pub async fn set_earning_type_active(engine: &ErpEngine, entity_id: Uuid, id: Uuid, active: bool) -> ErpResult<()> {
    sqlx::query("UPDATE earning_types SET active=$3 WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).bind(active).execute(engine.pool()).await?;
    Ok(())
}

// ─── Deduction types ─────────────────────────────────────────────────────────

pub async fn list_deduction_types(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<DeductionTypeRow>> {
    Ok(sqlx::query_as::<_, DeductionTypeRow>(
        "SELECT * FROM deduction_types WHERE entity_id = $1 ORDER BY sequence, name",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?)
}

pub async fn create_deduction_type(engine: &ErpEngine, entity_id: Uuid, req: CreateDeductionTypeRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO deduction_types (id, entity_id, code, name, category, pre_tax, gl_account_code, sequence) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (entity_id, code) DO NOTHING",
    )
    .bind(id).bind(entity_id).bind(&req.code).bind(&req.name).bind(&req.category).bind(req.pre_tax)
    .bind(&req.gl_account_code).bind(req.sequence)
    .execute(engine.pool()).await?;
    Ok(id)
}

pub async fn set_deduction_type_active(engine: &ErpEngine, entity_id: Uuid, id: Uuid, active: bool) -> ErpResult<()> {
    sqlx::query("UPDATE deduction_types SET active=$3 WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).bind(active).execute(engine.pool()).await?;
    Ok(())
}

/// Seed a sensible default set of earning & deduction types for a tenant, once.
pub async fn seed_default_types(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<()> {
    let ecount: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM earning_types WHERE entity_id=$1")
        .bind(entity_id).fetch_one(engine.pool()).await?;
    if ecount == 0 {
        let defaults = [
            ("HOUSING", "Housing Allowance", true, true, true),
            ("TRANSPORT", "Transport Allowance", true, true, true),
            ("OVERTIME", "Overtime", true, true, true),
            ("BONUS", "Bonus", true, true, true),
            ("COMMISSION", "Commission", true, true, true),
            ("REIMBURSEMENT", "Reimbursement", false, false, false),
        ];
        for (i, (code, name, taxable, pensionable, shif)) in defaults.iter().enumerate() {
            create_earning_type(engine, entity_id, CreateEarningTypeRequest {
                code: (*code).into(), name: (*name).into(), taxable: *taxable,
                pensionable: *pensionable, affects_shif: *shif, proratable: true,
                gl_account_code: None, sequence: (i as i32 + 1) * 10,
            }).await?;
        }
    }
    let dcount: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deduction_types WHERE entity_id=$1")
        .bind(entity_id).fetch_one(engine.pool()).await?;
    if dcount == 0 {
        let defaults = [
            ("PENSION", "Pension Contribution", "voluntary", true),
            ("SACCO", "SACCO Contribution", "welfare", false),
            ("INSURANCE", "Insurance Premium", "voluntary", false),
            ("WELFARE", "Staff Welfare", "welfare", false),
            ("ADVANCE", "Salary Advance", "loan", false),
        ];
        for (i, (code, name, category, pre_tax)) in defaults.iter().enumerate() {
            create_deduction_type(engine, entity_id, CreateDeductionTypeRequest {
                code: (*code).into(), name: (*name).into(), category: (*category).into(),
                pre_tax: *pre_tax, gl_account_code: None, sequence: (i as i32 + 1) * 10,
            }).await?;
        }
    }
    Ok(())
}

// ─── Departments ─────────────────────────────────────────────────────────────

pub async fn list_departments(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<DepartmentRow>> {
    Ok(sqlx::query_as::<_, DepartmentRow>(
        "SELECT * FROM departments WHERE entity_id = $1 ORDER BY name",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?)
}

pub async fn create_department(engine: &ErpEngine, entity_id: Uuid, req: CreateDepartmentRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO departments (id, entity_id, code, name, cost_center, dimension_value_id, parent_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (entity_id, code) DO NOTHING",
    )
    .bind(id).bind(entity_id).bind(&req.code).bind(&req.name).bind(&req.cost_center)
    .bind(req.dimension_value_id).bind(req.parent_id)
    .execute(engine.pool()).await?;
    Ok(id)
}

// ─── Recurring items ─────────────────────────────────────────────────────────

pub async fn list_recurring_items(engine: &ErpEngine, entity_id: Uuid, employee_id: Uuid) -> ErpResult<Vec<RecurringItemRow>> {
    Ok(sqlx::query_as::<_, RecurringItemRow>(
        "SELECT * FROM employee_recurring_items WHERE entity_id=$1 AND employee_id=$2 ORDER BY created_at",
    )
    .bind(entity_id).bind(employee_id).fetch_all(engine.pool()).await?)
}

pub async fn create_recurring_item(engine: &ErpEngine, entity_id: Uuid, req: CreateRecurringItemRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO employee_recurring_items (id, entity_id, employee_id, kind, type_code, name, amount, taxable, start_date, end_date) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,COALESCE($9,CURRENT_DATE),$10)",
    )
    .bind(id).bind(entity_id).bind(req.employee_id).bind(&req.kind).bind(&req.type_code)
    .bind(&req.name).bind(req.amount).bind(req.taxable).bind(req.start_date).bind(req.end_date)
    .execute(engine.pool()).await?;
    Ok(id)
}

pub async fn delete_recurring_item(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<()> {
    sqlx::query("DELETE FROM employee_recurring_items WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).execute(engine.pool()).await?;
    Ok(())
}

// ─── Per-run inputs ──────────────────────────────────────────────────────────

pub async fn list_run_inputs(engine: &ErpEngine, entity_id: Uuid, pay_run_id: Uuid) -> ErpResult<Vec<PayRunInputRow>> {
    Ok(sqlx::query_as::<_, PayRunInputRow>(
        "SELECT * FROM pay_run_inputs WHERE entity_id=$1 AND pay_run_id=$2 ORDER BY created_at",
    )
    .bind(entity_id).bind(pay_run_id).fetch_all(engine.pool()).await?)
}

pub async fn add_run_input(engine: &ErpEngine, entity_id: Uuid, pay_run_id: Uuid, req: CreatePayRunInputRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO pay_run_inputs (id, entity_id, pay_run_id, employee_id, kind, type_code, name, amount, taxable, note) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(id).bind(entity_id).bind(pay_run_id).bind(req.employee_id).bind(&req.kind)
    .bind(&req.type_code).bind(&req.name).bind(req.amount).bind(req.taxable).bind(&req.note)
    .execute(engine.pool()).await?;
    Ok(id)
}

pub async fn delete_run_input(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<()> {
    sqlx::query("DELETE FROM pay_run_inputs WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).execute(engine.pool()).await?;
    Ok(())
}

// ─── Loans ───────────────────────────────────────────────────────────────────

pub async fn list_loans(engine: &ErpEngine, entity_id: Uuid, employee_id: Uuid) -> ErpResult<Vec<EmployeeLoanRow>> {
    Ok(sqlx::query_as::<_, EmployeeLoanRow>(
        "SELECT * FROM employee_loans WHERE entity_id=$1 AND employee_id=$2 ORDER BY created_at DESC",
    )
    .bind(entity_id).bind(employee_id).fetch_all(engine.pool()).await?)
}

pub async fn create_loan(engine: &ErpEngine, entity_id: Uuid, req: CreateLoanRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO employee_loans (id, entity_id, employee_id, name, principal, balance, installment, interest_rate, start_date) \
         VALUES ($1,$2,$3,$4,$5,$5,$6,$7,COALESCE($8,CURRENT_DATE))",
    )
    .bind(id).bind(entity_id).bind(req.employee_id).bind(&req.name).bind(req.principal)
    .bind(req.installment).bind(req.interest_rate).bind(req.start_date)
    .execute(engine.pool()).await?;
    Ok(id)
}

// ─── Batch loaders for the engine (grouped by employee / run) ────────────────

/// Active recurring items for a set of employees, grouped by employee.
pub async fn recurring_items_grouped(
    engine: &ErpEngine,
    entity_id: Uuid,
    as_of: NaiveDate,
) -> ErpResult<HashMap<Uuid, Vec<RecurringItemRow>>> {
    let rows = sqlx::query_as::<_, RecurringItemRow>(
        "SELECT * FROM employee_recurring_items \
         WHERE entity_id=$1 AND active AND start_date <= $2 AND (end_date IS NULL OR end_date >= $2)",
    )
    .bind(entity_id).bind(as_of).fetch_all(engine.pool()).await?;
    let mut map: HashMap<Uuid, Vec<RecurringItemRow>> = HashMap::new();
    for r in rows {
        map.entry(r.employee_id).or_default().push(r);
    }
    Ok(map)
}

/// Active loans for a tenant, grouped by employee.
pub async fn active_loans_grouped(
    engine: &ErpEngine,
    entity_id: Uuid,
) -> ErpResult<HashMap<Uuid, Vec<EmployeeLoanRow>>> {
    let rows = sqlx::query_as::<_, EmployeeLoanRow>(
        "SELECT * FROM employee_loans WHERE entity_id=$1 AND status='active' AND balance > 0",
    )
    .bind(entity_id).fetch_all(engine.pool()).await?;
    let mut map: HashMap<Uuid, Vec<EmployeeLoanRow>> = HashMap::new();
    for r in rows {
        map.entry(r.employee_id).or_default().push(r);
    }
    Ok(map)
}

/// Per-run inputs for a run, grouped by employee.
pub async fn run_inputs_grouped(
    engine: &ErpEngine,
    entity_id: Uuid,
    pay_run_id: Uuid,
) -> ErpResult<HashMap<Uuid, Vec<PayRunInputRow>>> {
    let rows = list_run_inputs(engine, entity_id, pay_run_id).await?;
    let mut map: HashMap<Uuid, Vec<PayRunInputRow>> = HashMap::new();
    for r in rows {
        map.entry(r.employee_id).or_default().push(r);
    }
    Ok(map)
}
