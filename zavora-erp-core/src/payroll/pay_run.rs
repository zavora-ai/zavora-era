use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::AgentOrUserId;

use super::statutory::PayslipDeductions;

/// Status of a pay run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PayRunStatus {
    Draft,
    Approved,
    Posted,
    Paid,
}

/// An individual payslip within a pay run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payslip {
    pub id: Uuid,
    pub pay_run_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub deductions: PayslipDeductions,
    pub custom_deductions: Vec<CustomDeduction>,
    pub custom_earnings: Vec<CustomEarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomDeduction {
    pub name: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomEarning {
    pub name: String,
    pub amount: Decimal,
    pub taxable: bool,
}

/// A pay run — processing payroll for a period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayRun {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub period_id: Uuid,
    pub pay_date: NaiveDate,
    pub payslips: Vec<Payslip>,
    pub total_gross: Decimal,
    pub total_paye: Decimal,
    pub total_nssf: Decimal,
    pub total_sha: Decimal,
    pub total_housing_levy: Decimal,
    pub total_helb: Decimal,
    pub total_net: Decimal,
    pub status: PayRunStatus,
    pub journal_entry_id: Option<Uuid>,
    pub created_by: AgentOrUserId,
    pub created_at: DateTime<Utc>,
    pub approved_by: Option<AgentOrUserId>,
    pub approved_at: Option<DateTime<Utc>>,
}

impl PayRun {
    /// Recalculate totals from payslips.
    pub fn recalculate(&mut self) {
        self.total_gross = self.payslips.iter().map(|p| p.deductions.gross_salary).sum();
        self.total_paye = self.payslips.iter().map(|p| p.deductions.net_paye).sum();
        self.total_nssf = self
            .payslips
            .iter()
            .map(|p| p.deductions.nssf_employee + p.deductions.nssf_employer)
            .sum();
        self.total_sha = self.payslips.iter().map(|p| p.deductions.sha).sum();
        self.total_housing_levy = self
            .payslips
            .iter()
            .map(|p| p.deductions.housing_levy_employee + p.deductions.housing_levy_employer)
            .sum();
        self.total_helb = self.payslips.iter().map(|p| p.deductions.helb).sum();
        self.total_net = self.payslips.iter().map(|p| p.deductions.net_salary).sum();
    }
}

/// Database row for pay run.
#[derive(Debug, Clone, FromRow)]
pub struct PayRunRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub period_id: Uuid,
    pub pay_date: NaiveDate,
    pub total_gross: Decimal,
    pub total_paye: Decimal,
    pub total_nssf: Decimal,
    pub total_sha: Decimal,
    pub total_housing_levy: Decimal,
    pub total_helb: Decimal,
    pub total_net: Decimal,
    pub status: String,
    pub journal_entry_id: Option<Uuid>,
    pub created_by: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub approved_by: Option<serde_json::Value>,
    pub approved_at: Option<DateTime<Utc>>,
}

/// Request to run payroll for a period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPayrollRequest {
    pub period_id: Uuid,
    pub pay_date: NaiveDate,
    pub employee_ids: Option<Vec<Uuid>>, // None = all active employees
    pub run_by: AgentOrUserId,
}

/// Request to approve a pay run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovePayRunRequest {
    pub pay_run_id: Uuid,
    pub approved_by: AgentOrUserId,
}
