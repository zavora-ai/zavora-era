//! Domain models for the enterprise payroll masters and variable inputs:
//! earning/deduction types, departments, employee recurring items, per-run
//! inputs, and loans. Persistence/CRUD lives in `services::payroll_masters`; the
//! payroll engine consumes the load helpers there.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ── Earning types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct EarningTypeRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub code: String,
    pub name: String,
    pub taxable: bool,
    pub pensionable: bool,
    pub affects_shif: bool,
    pub proratable: bool,
    pub gl_account_code: Option<String>,
    pub sequence: i32,
    pub active: bool,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEarningTypeRequest {
    pub code: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub taxable: bool,
    #[serde(default = "default_true")]
    pub pensionable: bool,
    #[serde(default = "default_true")]
    pub affects_shif: bool,
    #[serde(default = "default_true")]
    pub proratable: bool,
    #[serde(default)]
    pub gl_account_code: Option<String>,
    #[serde(default = "default_seq")]
    pub sequence: i32,
}

// ── Deduction types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct DeductionTypeRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub code: String,
    pub name: String,
    pub category: String,
    pub pre_tax: bool,
    pub gl_account_code: Option<String>,
    pub sequence: i32,
    pub active: bool,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDeductionTypeRequest {
    pub code: String,
    pub name: String,
    #[serde(default = "default_voluntary")]
    pub category: String,
    #[serde(default)]
    pub pre_tax: bool,
    #[serde(default)]
    pub gl_account_code: Option<String>,
    #[serde(default = "default_seq")]
    pub sequence: i32,
}

// ── Departments ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct DepartmentRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub code: String,
    pub name: String,
    pub cost_center: Option<String>,
    pub dimension_value_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDepartmentRequest {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub cost_center: Option<String>,
    #[serde(default)]
    pub dimension_value_id: Option<Uuid>,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
}

// ── Recurring items ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RecurringItemRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub employee_id: Uuid,
    pub kind: String, // earning|deduction
    pub type_code: Option<String>,
    pub name: String,
    pub amount: Decimal,
    pub taxable: Option<bool>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRecurringItemRequest {
    pub employee_id: Uuid,
    pub kind: String,
    #[serde(default)]
    pub type_code: Option<String>,
    pub name: String,
    pub amount: Decimal,
    #[serde(default)]
    pub taxable: Option<bool>,
    #[serde(default)]
    pub start_date: Option<NaiveDate>,
    #[serde(default)]
    pub end_date: Option<NaiveDate>,
}

// ── Per-run inputs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PayRunInputRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub pay_run_id: Uuid,
    pub employee_id: Uuid,
    pub kind: String,
    pub type_code: Option<String>,
    pub name: String,
    pub amount: Decimal,
    pub taxable: bool,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePayRunInputRequest {
    pub employee_id: Uuid,
    pub kind: String,
    #[serde(default)]
    pub type_code: Option<String>,
    pub name: String,
    pub amount: Decimal,
    #[serde(default = "default_true")]
    pub taxable: bool,
    #[serde(default)]
    pub note: Option<String>,
}

// ── Loans ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct EmployeeLoanRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub employee_id: Uuid,
    pub name: String,
    pub principal: Decimal,
    pub balance: Decimal,
    pub installment: Decimal,
    pub interest_rate: Decimal,
    pub start_date: NaiveDate,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLoanRequest {
    pub employee_id: Uuid,
    pub name: String,
    pub principal: Decimal,
    pub installment: Decimal,
    #[serde(default)]
    pub interest_rate: Decimal,
    #[serde(default)]
    pub start_date: Option<NaiveDate>,
}

fn default_true() -> bool {
    true
}
fn default_seq() -> i32 {
    100
}
fn default_voluntary() -> String {
    "voluntary".to_string()
}
