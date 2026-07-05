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
    IncomeByCustomer,
    ExpenseByVendor,
    InventoryValuation,
    FixedAssetRegister,
    BudgetVsActual,
    DimensionalAnalysis,
    EquityChanges,
    CashFlowDirect,
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
    /// Dimension type code to group by (Dimensional Analysis report).
    #[serde(default)]
    pub dimension_type: Option<String>,
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
    PayeP10(PayeP10Report),
    WhtReport(WhtReport),
    VatDetail(VatDetailReport),
    PartyRanking(PartyRankingReport),
    InventoryValuation(InventoryValuationReport),
    FixedAssetRegister(FixedAssetRegisterReport),
    BankReconSummary(BankReconSummaryReport),
    BudgetVsActual(BudgetVsActualReport),
    DimensionalAnalysis(DimensionalAnalysisReport),
    EquityChanges(EquityChangesReport),
    CashFlowDirect(CashFlowDirectReport),
    CustomerPaymentHistory(CustomerPaymentHistoryReport),
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
    pub entry_id: Uuid,
    pub journal_number: String,
    pub description: String,
    pub reference: String,
    pub source: String,
    /// Id of the originating document (invoice/bill/credit note), when any.
    pub source_id: Option<Uuid>,
    pub debit: Decimal,
    pub credit: Decimal,
    pub balance: Decimal,
    /// Account this line belongs to. Populated on an "all accounts" run so the
    /// UI can group/label lines; for a single-account run it echoes the report's
    /// account. Optional for backward compatibility with older serialized data.
    #[serde(default)]
    pub account_code: String,
    #[serde(default)]
    pub account_name: String,
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

/// Customer payment history — every payment received from one customer over a
/// period, with the payment method, reference, amount, and how much of each
/// receipt is still unapplied (sitting as on-account credit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerPaymentHistoryReport {
    pub customer_id: Uuid,
    pub customer_name: String,
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub lines: Vec<PaymentHistoryLine>,
    /// Total received in the period.
    pub total_received: Decimal,
    /// Total still unapplied (on-account) across the period's receipts.
    pub total_unapplied: Decimal,
    /// Number of receipts in the period.
    pub payment_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentHistoryLine {
    pub date: NaiveDate,
    /// Payment number (e.g. PAY-2026-0001).
    pub number: String,
    /// Payment method label (Cash / Bank Transfer / M-Pesa / Cheque / …).
    pub method: String,
    /// External reference / receipt number.
    pub reference: String,
    pub amount: Decimal,
    /// Portion of this receipt not yet applied to a document (on-account).
    pub unapplied: Decimal,
    pub status: String,
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

/// PAYE return (KRA P10) — per-employee PAYE for the period, with the relief
/// breakdown KRA's P10 schedule requires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayeP10Report {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub lines: Vec<PayeP10Line>,
    pub total_gross: Decimal,
    pub total_taxable: Decimal,
    pub total_paye: Decimal,
    pub total_relief: Decimal,
    pub total_payable: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayeP10Line {
    pub staff_number: String,
    pub employee_name: String,
    pub kra_pin: String,
    pub gross_pay: Decimal,
    pub taxable_pay: Decimal,
    /// PAYE charged before reliefs.
    pub tax: Decimal,
    pub personal_relief: Decimal,
    pub insurance_relief: Decimal,
    /// PAYE payable after reliefs (net_paye).
    pub paye_payable: Decimal,
}

/// Withholding tax withheld from suppliers in the period — the data behind WHT
/// certificates (one line per bill that carried WHT).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhtReport {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub lines: Vec<WhtLine>,
    pub total_base: Decimal,
    pub total_wht: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhtLine {
    pub date: NaiveDate,
    pub document_number: String,
    pub vendor_name: String,
    pub kra_pin: Option<String>,
    pub wht_category: Option<String>,
    /// Amount WHT was computed on (the bill's net/subtotal).
    pub base_amount: Decimal,
    pub wht_amount: Decimal,
}

/// VAT summary by rate band (KRA VAT3 essentials): sales (output) and purchases
/// (input) broken down by VAT treatment, with the net VAT position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VatDetailReport {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub output: Vec<VatBand>,
    pub input: Vec<VatBand>,
    pub total_output_taxable: Decimal,
    pub total_output_vat: Decimal,
    pub total_input_taxable: Decimal,
    pub total_input_vat: Decimal,
    /// output VAT - input VAT. Positive => payable to KRA.
    pub net_vat: Decimal,
    pub is_payable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VatBand {
    /// VAT treatment (e.g. Standard16, ZeroRated, Exempt).
    pub treatment: String,
    pub taxable_amount: Decimal,
    pub vat_amount: Decimal,
    pub document_count: u32,
}

/// Income-by-customer / expense-by-vendor: net (ex-VAT) invoiced or billed
/// amounts grouped per party for the period, ranked high to low with each
/// party's share of the total.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyRankingReport {
    /// "customer" or "vendor".
    pub party_kind: String,
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub lines: Vec<PartyRankingLine>,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyRankingLine {
    pub party_id: Uuid,
    pub party_name: String,
    pub document_count: u32,
    pub amount: Decimal,
    /// Share of the period total, as a percentage (0–100).
    pub percent: Decimal,
}

/// Inventory valuation — current on-hand quantity, unit cost and value per
/// stock item (the running figures inventory maintenance keeps up to date).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryValuationReport {
    pub as_at: NaiveDate,
    pub lines: Vec<InventoryValuationLine>,
    pub total_value: Decimal,
    pub item_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryValuationLine {
    pub sku: String,
    pub description: String,
    pub uom: String,
    pub on_hand: Decimal,
    pub unit_cost: Decimal,
    pub total_value: Decimal,
    pub costing_method: String,
}

/// Fixed-asset register — cost, accumulated depreciation and net book value per
/// non-disposed asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedAssetRegisterReport {
    pub as_at: NaiveDate,
    pub lines: Vec<FixedAssetLine>,
    pub total_cost: Decimal,
    pub total_accumulated_depreciation: Decimal,
    pub total_net_book_value: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedAssetLine {
    pub asset_number: String,
    pub description: String,
    pub category: String,
    pub acquisition_date: NaiveDate,
    pub cost: Decimal,
    pub accumulated_depreciation: Decimal,
    pub net_book_value: Decimal,
    pub status: String,
}

/// Bank reconciliation summary — for each bank account, the statement balance
/// (the bank's own running balance) against the GL balance of its control
/// account, with the matched/unmatched feed items that explain any difference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankReconSummaryReport {
    pub as_at: NaiveDate,
    pub accounts: Vec<BankReconLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankReconLine {
    pub bank_account_id: Uuid,
    pub account_name: String,
    pub bank_name: String,
    pub gl_account: String,
    /// Latest running balance from the imported bank feed (as-at).
    pub statement_balance: Decimal,
    /// GL balance of the bank control account (debit positive).
    pub gl_balance: Decimal,
    /// Feed lines already posted to the GL.
    pub matched_count: u32,
    /// Feed lines not yet posted (the reconciling items).
    pub unmatched_count: u32,
    /// Net of the unmatched feed lines (money in − money out).
    pub unreconciled_amount: Decimal,
    /// statement_balance − gl_balance.
    pub difference: Decimal,
    /// True when the difference is fully explained by the unmatched items
    /// (|difference − unreconciled_amount| < 0.01).
    pub is_reconciled: bool,
}

/// Budget vs Actual — for each P&L account, actual ledger movement in the period
/// against the budgeted figure, with variance and variance %. Amounts are in the
/// account's natural sign (revenue positive when earned, expense positive when
/// incurred). Variance = actual − budget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetVsActualReport {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub lines: Vec<BudgetVsActualLine>,
    pub total_actual: Decimal,
    pub total_budget: Decimal,
    pub total_variance: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetVsActualLine {
    pub account_code: String,
    pub account_name: String,
    pub account_type: String,
    pub actual: Decimal,
    pub budget: Decimal,
    pub variance: Decimal,
    /// variance / budget * 100; None when budget is zero.
    pub variance_pct: Option<Decimal>,
}

/// Dimensional analysis — ledger movement grouped by the values of one
/// dimension type (e.g. by Cost Centre) for a period. Lines with no value for
/// that type are grouped under "(unassigned)".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionalAnalysisReport {
    pub dimension_type: String,
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub lines: Vec<DimensionalLine>,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub total_net: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionalLine {
    pub value_code: String,
    pub value_name: String,
    pub debit: Decimal,
    pub credit: Decimal,
    /// debit − credit.
    pub net: Decimal,
}

/// Statement of Changes in Equity — opening balance, profit for the period, and
/// per-account equity movements (capital introduced, drawings), to closing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityChangesReport {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub lines: Vec<EquityChangeLine>,
    pub opening_total: Decimal,
    /// Net profit for the period (not yet booked to equity accounts).
    pub profit_for_period: Decimal,
    pub closing_total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityChangeLine {
    pub account_code: String,
    pub account_name: String,
    pub opening: Decimal,
    pub movement: Decimal,
    pub closing: Decimal,
}

/// Direct-method cash flow — actual cash receipts and payments in the period,
/// grouped by the contra account (what the cash was for). Cash accounts are the
/// GL accounts behind the entity's bank accounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowDirectReport {
    pub period_from: NaiveDate,
    pub period_to: NaiveDate,
    pub receipts: Vec<CashFlowDirectLine>,
    pub payments: Vec<CashFlowDirectLine>,
    pub total_receipts: Decimal,
    pub total_payments: Decimal,
    pub net_change: Decimal,
    pub opening_cash: Decimal,
    pub closing_cash: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowDirectLine {
    pub account_code: String,
    pub account_name: String,
    pub amount: Decimal,
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
    // Dashboard money-bar widgets.
    pub invoices_bar: InvoicesBar,
    pub expense_breakdown: Vec<CategoryAmount>,
    pub bank_accounts: Vec<BankAccountBalance>,
    pub pnl_mtd: PnlSnapshot,
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

/// Invoice money-bar: open receivables split by ageing bucket,
/// plus cash collected recently.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InvoicesBar {
    pub overdue: Decimal,
    pub overdue_count: u32,
    pub due_soon: Decimal,
    pub due_soon_count: u32,
    pub open: Decimal,
    pub open_count: u32,
    pub paid_last_30: Decimal,
}

/// A named amount, e.g. one slice of an expenses-by-category breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryAmount {
    pub code: String,
    pub name: String,
    pub amount: Decimal,
}

/// Per-bank-account book balance for the dashboard bank strip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankAccountBalance {
    pub id: Uuid,
    pub name: String,
    pub bank_name: String,
    pub balance: Decimal,
}

/// Profit & loss totals for a single period (used for the dashboard P&L card).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PnlSnapshot {
    pub income: Decimal,
    pub expenses: Decimal,
    pub net_income: Decimal,
}
