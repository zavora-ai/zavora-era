// === Shared Types ===

export interface DashboardSummary {
  as_at: string;
  total_receivable: number;
  overdue_receivable: number;
  overdue_invoice_count: number;
  total_payable: number;
  overdue_payable: number;
  overdue_bill_count: number;
  cash_and_bank: number;
  net_income_mtd: number;
  net_income_prior: number;
  revenue_6m: MonthlyAmount[];
  expenses_6m: MonthlyAmount[];
  recent_transactions: TransactionSummary[];
  outstanding_invoices: InvoiceSummary[];
  pending_approvals: number;
  uncategorised_txns: number;
  // QuickBooks-style widgets.
  invoices_bar: InvoicesBar;
  expense_breakdown: CategoryAmount[];
  bank_accounts: BankAccountBalance[];
  pnl_mtd: PnlSnapshot;
  // Entity activity counts (used to detect a brand-new tenant for the empty state).
  invoice_count?: number;
  bill_count?: number;
  payment_count?: number;
}

export interface InvoicesBar {
  overdue: number;
  overdue_count: number;
  due_soon: number;
  due_soon_count: number;
  open: number;
  open_count: number;
  paid_last_30: number;
}

export interface CategoryAmount {
  code: string;
  name: string;
  amount: number;
}

export interface BankAccountBalance {
  id: string;
  name: string;
  bank_name: string;
  balance: number;
}

export interface PnlSnapshot {
  income: number;
  expenses: number;
  net_income: number;
}

export interface MonthlyAmount {
  year: number;
  month: number;
  amount: number;
}

export interface TransactionSummary {
  id: string;
  date: string;
  description: string;
  amount: number;
  transaction_type: string;
}

export interface InvoiceSummary {
  id: string;
  number: string;
  customer_name: string;
  amount: number;
  balance_due: number;
  due_date: string;
  is_overdue: boolean;
}

// === Account ===
export interface Account {
  id: string;
  entity_id: string;
  code: string;
  name: string;
  account_type: string;
  parent_code?: string;
  currency?: string;
  is_control: boolean;
  is_active: boolean;
  tags: string[];
  created_at: string;
}

// === Customer ===
export interface Customer {
  id: string;
  entity_id: string;
  name: string;
  kra_pin?: string;
  vat_number?: string;
  email: ContactEmail[];
  phone: ContactPhone[];
  address?: Address;
  currency: string;
  payment_terms: string;
  credit_limit?: number;
  ar_account: string;
  reminder_policy: string;
  portal_enabled: boolean;
  is_active: boolean;
  created_at: string;
}

export interface ContactEmail {
  email: string;
  label?: string;
  is_primary: boolean;
}

export interface ContactPhone {
  number: string;
  label?: string;
  is_primary: boolean;
  whatsapp_enabled: boolean;
}

export interface Address {
  line1: string;
  line2?: string;
  city: string;
  county?: string;
  postal_code?: string;
  country: string;
}

// === Vendor ===
export interface Vendor {
  id: string;
  entity_id: string;
  name: string;
  kra_pin?: string;
  vat_number?: string;
  email: ContactEmail[];
  phone: ContactPhone[];
  address?: Address;
  currency: string;
  payment_terms: string;
  wht_category?: string;
  resident: boolean;
  ap_account: string;
  default_expense_account?: string;
  bank_details?: BankDetails;
  notes?: string;
  is_active: boolean;
  created_at: string;
}

export interface BankDetails {
  bank_name: string;
  branch?: string;
  account_name: string;
  account_number: string;
  swift_code?: string;
}

// === Product ===
export interface Product {
  id: string;
  entity_id: string;
  name: string;
  description?: string;
  product_type: 'Service' | 'Goods' | 'Expense';
  unit_price?: number;
  currency: string;
  uom: string;
  sales_account: string;
  purchase_account: string;
  vat_treatment: string;
  track_inventory: boolean;
  inventory_item_id?: string;
  is_active: boolean;
  created_at: string;
}

// === Invoice ===
export interface Invoice {
  id: string;
  entity_id: string;
  number: string;
  invoice_type: 'Invoice' | 'CreditNote';
  customer_id: string;
  issue_date: string;
  due_date: string;
  currency: string;
  fx_rate: number;
  subtotal: number;
  discount_total: number;
  tax_total: number;
  gross_total: number;
  amount_paid: number;
  balance_due: number;
  status: InvoiceStatus;
  source_estimate?: string;
  credit_note_for?: string;
  journal_entry_id?: string;
  etims_status?: 'not_transmitted' | 'transmitted' | 'transmission_failed';
  etims_invoice_number?: string;
  etims_transmitted_at?: string;
  sent_at?: string;
  viewed_at?: string;
  paid_at?: string;
  template_id?: string;
  notes?: string;
  created_at: string;
}

export type InvoiceStatus = 'draft' | 'sent' | 'viewed' | 'partially_paid' | 'paid' | 'overdue' | 'voided';

// === Bill ===
export interface Bill {
  id: string;
  entity_id: string;
  number: string;
  vendor_id: string;
  vendor_invoice_number?: string;
  issue_date: string;
  due_date: string;
  currency: string;
  fx_rate: number;
  subtotal: number;
  tax_total: number;
  wht_amount: number;
  gross_total: number;
  amount_paid: number;
  balance_due: number;
  status: BillStatus;
  journal_entry_id?: string;
  approved_by?: string;
  approved_at?: string;
  notes?: string;
  created_at: string;
}

export type BillStatus = 'draft' | 'pending_approval' | 'approved' | 'posted' | 'partially_paid' | 'paid' | 'disputed' | 'cancelled';

// === Payment ===
export interface PaymentApplication {
  document_id: string;
  document_type: 'Invoice' | 'Bill';
  amount_applied: number;
}

export interface Payment {
  id: string;
  entity_id: string;
  number: string;
  payment_type: 'customer_payment' | 'vendor_payment';
  party_id: string;
  payment_date: string;
  amount: number;
  currency: string;
  fx_rate: number;
  method: any;
  reference: string;
  bank_account_id?: string;
  applications: PaymentApplication[];
  unapplied: number;
  journal_entry_id?: string;
  status: string;
  created_at: string;
}

// === Fiscal Period ===
export interface FiscalPeriod {
  id: string;
  name: string;
  start_date: string;
  end_date: string;
  status: 'future' | 'open' | 'soft_closed' | 'hard_closed';
  fiscal_year: number;
  period_number: number;
  closed_at?: string;
}

// === Payroll ===
export interface PayRun {
  id: string;
  entity_id: string;
  period_id: string;
  pay_date: string;
  total_gross: number;
  total_paye: number;
  total_nssf: number;
  total_sha: number;
  total_housing_levy: number;
  total_helb: number;
  total_net: number;
  status: 'draft' | 'approved' | 'posted' | 'paid';
  journal_entry_id?: string;
  created_by: any;
  created_at: string;
  approved_by?: any;
  approved_at?: string;
}

// === Settings ===
export interface ErpConfig {
  entity_id: string;
  base_currency: string;
  fiscal_year_end: { month: number; day: number };
  branding: BrandingConfig;
  sequences: DocumentSequences;
  tax_config: TaxConfig;
  payment_config: PaymentConfig;
  posting: PostingSetup;
}

// === Posting Setup (GL account determination) ===
export interface PostingSetup {
  accounts_receivable: string;
  accounts_payable: string;
  unapplied_payments: string;
  vat_output: string;
  vat_input: string;
  wht_payable: string;
  realised_fx_gain: string;
  realised_fx_loss: string;
  unrealised_fx_gain: string;
  unrealised_fx_loss: string;
  retained_earnings: string;
  default_bank: string;
  default_sales: string;
  default_purchase: string;
  default_expense: string;
  salaries_expense: string;
  nssf_employer_expense: string;
  housing_levy_employer_expense: string;
  paye_payable: string;
  nssf_payable: string;
  sha_payable: string;
  helb_payable: string;
  housing_levy_payable: string;
  net_pay_payable: string;
}

export interface BrandingConfig {
  company_name: string;
  logo_url?: string;
  primary_color: string;
  kra_pin?: string;
  vat_number?: string;
  registration_number?: string;
  address?: string;
  phone?: string;
}

export interface DocumentSequences {
  invoice_prefix: string;
  invoice_next: number;
  estimate_prefix: string;
  estimate_next: number;
  credit_note_prefix: string;
  credit_note_next: number;
  bill_prefix: string;
  bill_next: number;
  journal_prefix: string;
  journal_next: number;
  payment_prefix: string;
  payment_next: number;
  year_reset: boolean;
}

export interface TaxConfig {
  vat_registered: boolean;
  vat_number?: string;
  standard_vat_rate: number;
  wht_enabled: boolean;
  paye_enabled: boolean;
}

export interface PaymentConfig {
  mpesa_enabled: boolean;
  mpesa_paybill?: string;
  flutterwave_enabled: boolean;
  bank_transfer_enabled: boolean;
}

// === Employee ===
export interface Employee {
  id: string;
  entity_id: string;
  staff_number: string;
  full_name: string;
  kra_pin: string;
  nssf_number?: string;
  nhif_number?: string;
  helb_deduction?: number;
  employment_type: string;
  basic_salary: number;
  allowances: any;
  bank_account: any;
  tax_relief: number;
  disability_exemption: boolean;
  start_date: string;
  end_date?: string;
  is_active: boolean;
  created_at: string;
}

// === Journal Entry ===
export interface JournalEntry {
  id: string;
  entity_id: string;
  number: string;
  date: string;
  period_id: string;
  source: string;
  reference: string;
  description: string;
  status: string;
  created_by: any;
  created_at: string;
  posted_at?: string;
}

// === Estimate ===
export interface Estimate {
  id: string;
  entity_id: string;
  number: string;
  customer_id: string;
  issue_date: string;
  expiry_date: string;
  currency: string;
  subtotal: number;
  tax_total: number;
  gross_total: number;
  status: EstimateStatus;
  notes?: string;
  converted_to?: string;
  created_at: string;
}

export type EstimateStatus = 'draft' | 'sent' | 'accepted' | 'declined' | 'expired' | 'converted';

// === Supplier Credit Note (AP) ===
export interface SupplierCreditNote {
  id: string;
  vendor_id: string;
  credit_note_number: string;
  credit_note_date: string;
  applies_to_bill?: string;
  gross_total: number;
  status: string;
  journal_entry_id?: string;
  created_at: string;
}

// === Inventory ===
export interface InventoryItem {
  id: string;
  entity_id: string;
  product_id?: string;
  sku: string;
  description: string;
  uom: string;
  costing_method: string;
  gl_inventory: string;
  gl_cogs: string;
  warehouse_id?: string;
  on_hand: number;
  committed: number;
  available: number;
  unit_cost: number;
  total_value: number;
  reorder_point?: number;
  reorder_quantity?: number;
  is_active: boolean;
  created_at: string;
}

// === Fixed Assets ===
export interface FixedAsset {
  id: string;
  entity_id: string;
  asset_number: string;
  description: string;
  category: string;
  acquisition_date: string;
  cost: number;
  residual_value: number;
  useful_life_months: number;
  depreciation_method: any;
  accumulated_depreciation: number;
  net_book_value: number;
  gl_asset_account: string;
  gl_accum_depr_account: string;
  gl_depr_expense: string;
  status: string;
  disposal_date?: string;
  disposal_proceeds?: number;
  created_at: string;
}

// === FX Rates ===
export interface ExchangeRateEntry {
  id: string;
  entity_id: string;
  from_ccy: string;
  to_ccy: string;
  rate_date: string;
  rate_type: string;
  rate: string; // serialised Decimal (string); coerce with Number() before maths
  source: string;
}

// === Categorisation Queue Transaction ===
export interface CategorisationTransaction {
  id: string;
  entity_id: string;
  bank_account_id: string;
  date: string;
  description: string;
  amount: number;
  reference: string;
  status: 'uncategorised' | 'categorised' | 'excluded' | 'split' | 'merged';
  suggestion?: {
    account_code: string;
    account_name: string;
    confidence: number;
  };
  assigned_account_code?: string;
  assigned_account_name?: string;
  created_at: string;
}

// === Audit ===
export interface AuditEventEntry {
  id: string;
  entity_id: string;
  event_type: string;
  object_type: string;
  object_id: string;
  actor: { type?: string; id?: string } | string;
  actor_name?: string;
  actor_email?: string;
  before?: any;
  after?: any;
  metadata?: any;
  timestamp: string;
}

// === Bank Account ===
export interface BankAccount {
  id: string;
  entity_id: string;
  name: string;
  bank_name: string;
  account_number: string;
  currency: string;
  gl_account: string;
  feed_enabled: boolean;
  feed_provider?: string | null;
  last_sync?: string | null;
  is_active: boolean;
  created_at: string;
}
