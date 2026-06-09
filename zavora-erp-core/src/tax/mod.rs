use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// VAT return data prepared for iTax filing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VatReturnData {
    pub entity_id: Uuid,
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub vat_number: String,
    pub standard_rated_sales: Decimal,
    pub zero_rated_sales: Decimal,
    pub exempt_sales: Decimal,
    pub total_output_vat: Decimal,
    pub standard_rated_purchases: Decimal,
    pub zero_rated_purchases: Decimal,
    pub exempt_purchases: Decimal,
    pub total_input_vat: Decimal,
    pub net_vat_payable: Decimal,
    pub filing_due_date: NaiveDate,
}

/// WHT certificate data (P10A report).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhtCertificate {
    pub entity_id: Uuid,
    pub vendor_id: Uuid,
    pub vendor_name: String,
    pub vendor_pin: String,
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub wht_category: String,
    pub gross_amount: Decimal,
    pub wht_rate: Decimal,
    pub wht_amount: Decimal,
    pub certificate_number: String,
    pub date_issued: NaiveDate,
}

/// PAYE P10 schedule data for KRA filing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayeP10Data {
    pub entity_id: Uuid,
    pub entity_pin: String,
    pub period: NaiveDate, // first day of month
    pub employees: Vec<P10EmployeeRecord>,
    pub total_gross: Decimal,
    pub total_paye: Decimal,
    pub filing_due_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P10EmployeeRecord {
    pub employee_name: String,
    pub kra_pin: String,
    pub gross_salary: Decimal,
    pub benefits: Decimal,
    pub pension_contribution: Decimal,
    pub taxable_pay: Decimal,
    pub tax_charged: Decimal,
    pub personal_relief: Decimal,
    pub insurance_relief: Decimal,
    pub paye_due: Decimal,
}

/// Sales tax summary report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesTaxSummary {
    pub entity_id: Uuid,
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub output_vat_standard: Decimal,
    pub output_vat_zero: Decimal,
    pub output_vat_exempt: Decimal,
    pub input_vat_standard: Decimal,
    pub input_vat_zero: Decimal,
    pub input_vat_exempt: Decimal,
    pub net_position: Decimal,
    pub lines: Vec<SalesTaxLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesTaxLine {
    pub date: NaiveDate,
    pub document_type: String,
    pub document_number: String,
    pub party_name: String,
    pub party_pin: Option<String>,
    pub taxable_amount: Decimal,
    pub vat_amount: Decimal,
    pub vat_rate: Decimal,
}
