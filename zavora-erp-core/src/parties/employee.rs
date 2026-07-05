use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::BankDetails;

/// Employment type classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EmploymentType {
    Permanent,
    Contract,
    Casual,
}

/// An allowance component of employee compensation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allowance {
    pub name: String,
    pub amount: Decimal,
    pub taxable: bool,
}

/// An employee record for payroll processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Employee {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub staff_number: String,
    pub full_name: String,
    pub kra_pin: String,
    pub nssf_number: Option<String>,
    pub nhif_number: Option<String>,
    pub helb_deduction: Option<Decimal>,
    pub employment_type: EmploymentType,
    pub basic_salary: Decimal,
    pub allowances: Vec<Allowance>,
    pub bank_account: BankDetails,
    pub tax_relief: Decimal,
    pub disability_exemption: bool,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Database row for employee.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct EmployeeRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub staff_number: String,
    pub full_name: String,
    pub kra_pin: String,
    pub nssf_number: Option<String>,
    pub nhif_number: Option<String>,
    pub helb_deduction: Option<Decimal>,
    pub employment_type: String,
    pub basic_salary: Decimal,
    pub allowances: serde_json::Value,
    pub bank_account: serde_json::Value,
    pub tax_relief: Decimal,
    pub disability_exemption: bool,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    // HR Phase 1 additions (nullable; default None for legacy rows).
    #[serde(default)]
    pub manager_id: Option<Uuid>,
    #[serde(default)]
    pub department: Option<String>,
    #[serde(default)]
    pub job_title: Option<String>,
    #[serde(default)]
    pub personal_email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
}

/// Request to create an employee.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEmployeeRequest {
    pub staff_number: String,
    pub full_name: String,
    pub kra_pin: String,
    pub nssf_number: Option<String>,
    pub nhif_number: Option<String>,
    pub helb_deduction: Option<Decimal>,
    pub employment_type: EmploymentType,
    pub basic_salary: Decimal,
    pub allowances: Vec<Allowance>,
    pub bank_account: BankDetails,
    pub tax_relief: Option<Decimal>,
    pub disability_exemption: Option<bool>,
    pub start_date: NaiveDate,
    #[serde(default)]
    pub department: Option<String>,
    #[serde(default)]
    pub job_title: Option<String>,
    #[serde(default)]
    pub manager_id: Option<Uuid>,
    #[serde(default)]
    pub personal_email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
}

/// Request to update an employee.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateEmployeeRequest {
    pub full_name: Option<String>,
    pub kra_pin: Option<String>,
    pub nssf_number: Option<Option<String>>,
    pub nhif_number: Option<Option<String>>,
    pub helb_deduction: Option<Option<Decimal>>,
    pub employment_type: Option<EmploymentType>,
    pub basic_salary: Option<Decimal>,
    pub allowances: Option<Vec<Allowance>>,
    pub bank_account: Option<BankDetails>,
    pub tax_relief: Option<Decimal>,
    pub disability_exemption: Option<bool>,
    pub end_date: Option<Option<NaiveDate>>,
    pub is_active: Option<bool>,
}
