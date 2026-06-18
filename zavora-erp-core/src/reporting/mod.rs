use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{ExportFormat, MonthlyAmount};

/// Report request — unified entry point for all report types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRequest {
    pub entity_id: Uuid,
    pub report_type: ReportType,
    pub parameters: ReportParameters,
}

/// Available report types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportType {
    TrialBalance,
    BalanceSheet,
    ProfitAndLoss,
    CashFlow,
    ArAgeing,
    ApAgeing,
    VatReturn,
    GlDetail,
    CustomerStatement,
    VendorStatement,
    CustomerPaymentHistory,
    BankReconSummary,
    PayrollSummary,
    PayeP10,
    WhtCertificate,
    SalesTaxSummary,
}

/// Parameters for report generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportParameters {
    pub as_at: Option<NaiveDate>,
    pub period_from: Option<NaiveDate>,
    pub period_to: Option<NaiveDate>,
    pub compare_to: Option<NaiveDate>,
    pub comparative: Option<bool>,
    pub customer_id: Option<Uuid>,
    pub vendor_id: Option<Uuid>,
    pub account_code: Option<String>,
    pub bank_account_id: Option<Uuid>,
    pub statement_id: Option<Uuid>,
    pub period_id: Option<Uuid>,
}

/// Report data — the generated report content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportData {
    pub report_type: ReportType,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub entity_id: Uuid,
    pub title: String,
    pub subtitle: Option<String>,
    pub content: ReportContent,
}

/// Content varies by report type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportContent {
    TrialBalance(TrialBalanceReport),
    BalanceSheet(BalanceSheetReport),
    ProfitAndLoss(ProfitAndLossReport),
    CashFlow(CashFlowReport),
    ArAgeing(AgeingReport),
    ApAgeing(AgeingReport),
    GlDetail(GlDetailReport),
    VatReturn(VatReturnReport),
    PartyStatement(PartyStatementReport),
    PayrollSummary(PayrollSummaryReport),
    Generic(serde_json::Value),
}

/// Trial balance report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialBalanceReport {
    pub as_at: NaiveDate,
    pub lines: Vec<TrialBalanceLine>,
    pub total_debits: Decimal,
    pub total_credits: Decimal,
    /// True when total debits equal total credits (within 0.01). A trial balance
    /// that does not balance signals a posting integrity problem.
    pub is_balanced: bool,
    /// total_debits - total_credits (0.00 when balanced).
    pub difference: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialBalanceLine {
    pub account_code: String,
    pub account_name: String,
    pub opening_debit: Decimal,
    pub opening_credit: Decimal,
    pub movement_debit: Decimal,
    pub movement_credit: Decimal,
    pub closing_debit: Decimal,
    pub closing_credit: Decimal,
}

/// Balance sheet report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetReport {
    pub as_at: NaiveDate,
    pub assets: Vec<BalanceSheetSection>,
    pub liabilities: Vec<BalanceSheetSection>,
    pub equity: Vec<BalanceSheetSection>,
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    pub total_equity: Decimal,
    /// Net income for the year-to-date (as-at), folded into equity as
    /// "Current Year Earnings" so the sheet balances before year-end close.
    pub current_year_earnings: Decimal,
    /// Comparative as-at date (set when a comparative was requested).
    #[serde(default)]
    pub comparative_as_at: Option<NaiveDate>,
    #[serde(default)]
    pub total_assets_comparative: Option<Decimal>,
    #[serde(default)]
    pub total_liabilities_comparative: Option<Decimal>,
    #[serde(default)]
    pub total_equity_comparative: Option<Decimal>,
    /// True when Assets == Liabilities + Equity (within 0.01).
    pub is_balanced: bool,
    /// total_assets - (total_liabilities + total_equity); 0.00 when balanced.
    pub difference: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetSection {
    pub name: String,
    pub lines: Vec<BalanceSheetLine>,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetLine {
    pub account_code: String,
    pub account_name: String,
    pub amount: Decimal,
    pub comparative: Option<Decimal>,
}

/// Profit & Loss report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfitAndLossReport {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub revenue: Vec<PnlSection>,
    pub cost_of_sales: Vec<PnlSection>,
    pub operating_expenses: Vec<PnlSection>,
    pub other_income_expense: Vec<PnlSection>,
    pub total_revenue: Decimal,
    pub total_cost_of_sales: Decimal,
    pub gross_profit: Decimal,
    pub total_operating_expenses: Decimal,
    pub operating_profit: Decimal,
    pub net_profit: Decimal,
    /// Comparative period (set when a comparative was requested).
    #[serde(default)]
    pub comparative_from: Option<NaiveDate>,
    #[serde(default)]
    pub comparative_to: Option<NaiveDate>,
    #[serde(default)]
    pub total_revenue_comparative: Option<Decimal>,
    #[serde(default)]
    pub gross_profit_comparative: Option<Decimal>,
    #[serde(default)]
    pub operating_profit_comparative: Option<Decimal>,
    #[serde(default)]
    pub net_profit_comparative: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnlSection {
    pub name: String,
    pub lines: Vec<PnlLine>,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnlLine {
    pub account_code: String,
    pub account_name: String,
    pub amount: Decimal,
    pub comparative: Option<Decimal>,
}

/// Cash flow statement (indirect method).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowReport {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub operating_activities: CashFlowSection,
    pub investing_activities: CashFlowSection,
    pub financing_activities: CashFlowSection,
    pub net_change: Decimal,
    pub opening_cash: Decimal,
    pub closing_cash: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowSection {
    pub lines: Vec<CashFlowLine>,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowLine {
    pub description: String,
    pub amount: Decimal,
}

/// AR/AP ageing report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgeingReport {
    pub as_at: NaiveDate,
    pub lines: Vec<AgeingLine>,
    pub totals: AgeingBuckets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgeingLine {
    pub party_id: Uuid,
    pub party_name: String,
    pub current: Decimal,
    pub days_1_30: Decimal,
    pub days_31_60: Decimal,
    pub days_61_90: Decimal,
    pub over_90: Decimal,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgeingBuckets {
    pub current: Decimal,
    pub days_1_30: Decimal,
    pub days_31_60: Decimal,
    pub days_61_90: Decimal,
    pub over_90: Decimal,
    pub total: Decimal,
}

/// GL detail report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlDetailReport {
    pub account_code: String,
    pub account_name: String,
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub opening_balance: Decimal,
    pub lines: Vec<GlDetailLine>,
    pub closing_balance: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlDetailLine {
    pub date: NaiveDate,
    pub journal_number: String,
    pub description: String,
    pub reference: String,
    pub debit: Decimal,
    pub credit: Decimal,
    pub balance: Decimal,
}

/// VAT return (KRA VAT3 essentials) for a period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VatReturnReport {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    /// VAT charged on sales (net credit movement of the VAT-output account).
    pub output_vat: Decimal,
    /// VAT incurred on purchases (net debit movement of the VAT-input account).
    pub input_vat: Decimal,
    /// output_vat - input_vat. Positive => payable to KRA; negative => credit carried forward.
    pub net_vat: Decimal,
    /// True when net_vat > 0 (a payment is due to KRA).
    pub is_payable: bool,
    pub vat_output_account: String,
    pub vat_input_account: String,
}

/// Customer or vendor statement — a running-balance account activity report
/// for one party over a period. Charges (invoices/bills) increase the balance
/// outstanding; payments (receipts/payments) reduce it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyStatementReport {
    pub party_id: Uuid,
    pub party_name: String,
    /// "customer" or "vendor".
    pub party_kind: String,
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    /// Balance outstanding immediately before period_from.
    pub opening_balance: Decimal,
    pub lines: Vec<StatementLine>,
    /// Sum of charges (invoices for a customer; bills for a vendor) in the period.
    pub total_charges: Decimal,
    /// Sum of payments (receipts for a customer; payments for a vendor) in the period.
    pub total_payments: Decimal,
    /// Balance outstanding at period_to (opening + charges - payments).
    pub closing_balance: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementLine {
    pub date: NaiveDate,
    /// "Invoice" / "Bill" / "Receipt" / "Payment".
    pub doc_type: String,
    pub reference: String,
    /// Amount that increases the outstanding balance (invoice/bill).
    pub charge: Decimal,
    /// Amount that reduces the outstanding balance (receipt/payment).
    pub payment: Decimal,
    /// Running outstanding balance after this line.
    pub balance: Decimal,
}

/// Payroll summary — statutory and net-pay totals across the pay runs whose
/// pay date falls in the period, with a per-run and a per-employee breakdown.
/// Draft runs are excluded (not yet real payroll). NSSF/SHA/housing figures are
/// the employee-side deductions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollSummaryReport {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub runs: Vec<PayrollRunLine>,
    pub employees: Vec<PayrollEmployeeLine>,
    pub totals: PayrollTotals,
    /// Distinct employees paid in the period.
    pub employee_count: u32,
    pub run_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollTotals {
    pub gross: Decimal,
    pub paye: Decimal,
    pub nssf: Decimal,
    pub sha: Decimal,
    pub housing_levy: Decimal,
    pub helb: Decimal,
    pub net: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollRunLine {
    pub pay_run_id: Uuid,
    pub pay_date: NaiveDate,
    pub status: String,
    pub employee_count: u32,
    pub gross: Decimal,
    pub paye: Decimal,
    pub nssf: Decimal,
    pub sha: Decimal,
    pub housing_levy: Decimal,
    pub helb: Decimal,
    pub net: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollEmployeeLine {
    pub employee_id: Uuid,
    pub employee_name: String,
    pub gross: Decimal,
    pub paye: Decimal,
    pub nssf: Decimal,
    pub sha: Decimal,
    pub housing_levy: Decimal,
    pub helb: Decimal,
    pub net: Decimal,
}

/// Export output from report generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOutput {
    pub format: ExportFormat,
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

/// Dashboard summary — single-call overview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub as_at: chrono::DateTime<chrono::Utc>,
    pub total_receivable: Decimal,
    pub overdue_receivable: Decimal,
    pub overdue_invoice_count: u32,
    pub total_payable: Decimal,
    pub overdue_payable: Decimal,
    pub overdue_bill_count: u32,
    pub cash_and_bank: Decimal,
    pub net_income_mtd: Decimal,
    pub net_income_prior: Decimal,
    pub revenue_6m: Vec<MonthlyAmount>,
    pub expenses_6m: Vec<MonthlyAmount>,
    pub recent_transactions: Vec<TransactionSummary>,
    pub outstanding_invoices: Vec<InvoiceSummary>,
    pub pending_approvals: u32,
    pub uncategorised_txns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub id: Uuid,
    pub date: NaiveDate,
    pub description: String,
    pub amount: Decimal,
    pub transaction_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceSummary {
    pub id: Uuid,
    pub number: String,
    pub customer_name: String,
    pub amount: Decimal,
    pub balance_due: Decimal,
    pub due_date: NaiveDate,
    pub is_overdue: bool,
}
