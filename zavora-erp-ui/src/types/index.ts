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
  currency: string;
  payment_terms: string;
  credit_limit?: number;
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

// === Vendor ===
export interface Vendor {
  id: string;
  entity_id: string;
  name: string;
  kra_pin?: string;
  vat_number?: string;
  currency: string;
  payment_terms: string;
  wht_category?: string;
  resident: boolean;
  is_active: boolean;
  created_at: string;
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
  is_active: boolean;
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
  subtotal: number;
  tax_total: number;
  gross_total: number;
  amount_paid: number;
  balance_due: number;
  status: InvoiceStatus;
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
  issue_date: string;
  due_date: string;
  currency: string;
  subtotal: number;
  tax_total: number;
  wht_amount: number;
  gross_total: number;
  amount_paid: number;
  balance_due: number;
  status: BillStatus;
  notes?: string;
  created_at: string;
}

export type BillStatus = 'draft' | 'pending_approval' | 'approved' | 'posted' | 'partially_paid' | 'paid' | 'disputed' | 'cancelled';

// === Payment ===
export interface Payment {
  id: string;
  number: string;
  payment_type: 'CustomerPayment' | 'VendorPayment';
  party_id: string;
  payment_date: string;
  amount: number;
  currency: string;
  method: any;
  reference: string;
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
}

// === Payroll ===
export interface PayRun {
  id: string;
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
}

export interface BrandingConfig {
  company_name: string;
  logo_url?: string;
  primary_color: string;
  kra_pin?: string;
  vat_number?: string;
}

export interface DocumentSequences {
  invoice_prefix: string;
  invoice_next: number;
  estimate_prefix: string;
  estimate_next: number;
  bill_prefix: string;
  bill_next: number;
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
  converted_invoice_id?: string;
  created_at: string;
}

export type EstimateStatus = 'draft' | 'sent' | 'viewed' | 'accepted' | 'rejected' | 'expired' | 'converted';
