// Report registry, slug mapping, and request builder.
// Extracted faithfully from the original monolithic ReportsPage.

// The backend overrides entity_id from the JWT, so we keep sending the zero UUID
// exactly as the original implementation did.
export const ZERO_ENTITY = '00000000-0000-0000-0000-000000000000';

export type CtrlKind = 'asAt' | 'period' | 'account' | 'party' | 'dimension';

export type ReportCategory =
  | 'Financial'
  | 'Receivables/Payables'
  | 'Tax'
  | 'Payroll'
  | 'Management';

export interface ReportMeta {
  key: string;
  name: string;
  desc: string;
  controls: CtrlKind[];
  comparable?: boolean;
  party?: 'customer' | 'vendor';
  category: ReportCategory;
}

// Parameters captured by the filter controls.
export interface ReportParams {
  asAt: string;
  from: string;
  to: string;
  account: string;
  partyId: string;
  compare: boolean;
  dimensionType: string;
}

export const reportTypes: ReportMeta[] = [
  { key: 'TrialBalance', name: 'Trial Balance', desc: 'Account balances at a point in time', controls: ['asAt'], category: 'Financial' },
  { key: 'BalanceSheet', name: 'Balance Sheet', desc: 'Assets, liabilities, and equity', controls: ['asAt'], comparable: true, category: 'Financial' },
  { key: 'ProfitAndLoss', name: 'Profit & Loss', desc: 'Revenue and expenses for a period', controls: ['period'], comparable: true, category: 'Financial' },
  { key: 'CashFlow', name: 'Cash Flow Statement', desc: 'Cash movements (indirect method)', controls: ['period'], category: 'Financial' },
  { key: 'GlDetail', name: 'General Ledger', desc: 'Transaction detail by account', controls: ['period', 'account'], category: 'Financial' },
  { key: 'ArAgeing', name: 'AR Ageing', desc: 'Customer balances by age bucket', controls: ['asAt'], category: 'Receivables/Payables' },
  { key: 'ApAgeing', name: 'AP Ageing', desc: 'Vendor balances by age bucket', controls: ['asAt'], category: 'Receivables/Payables' },
  { key: 'CustomerStatement', name: 'Customer Statement', desc: 'Account activity & balance for one customer', controls: ['party', 'period'], party: 'customer', category: 'Receivables/Payables' },
  { key: 'VendorStatement', name: 'Vendor Statement', desc: 'Account activity & balance for one vendor', controls: ['party', 'period'], party: 'vendor', category: 'Receivables/Payables' },
  { key: 'VatReturn', name: 'VAT Return', desc: 'Output vs input VAT, net payable to KRA', controls: ['period'], category: 'Tax' },
  { key: 'SalesTaxSummary', name: 'VAT by Rate', desc: 'Output & input VAT broken down by rate band', controls: ['period'], category: 'Tax' },
  { key: 'WhtCertificate', name: 'WHT Schedule', desc: 'Withholding tax withheld from suppliers', controls: ['period'], category: 'Tax' },
  { key: 'PayrollSummary', name: 'Payroll Summary', desc: 'Gross, PAYE, NSSF, SHA, levy & net by employee', controls: ['period'], category: 'Payroll' },
  { key: 'PayeP10', name: 'PAYE Return (P10)', desc: 'KRA monthly PAYE schedule by employee', controls: ['period'], category: 'Payroll' },
  { key: 'IncomeByCustomer', name: 'Income by Customer', desc: 'Net revenue ranked by customer', controls: ['period'], category: 'Management' },
  { key: 'ExpenseByVendor', name: 'Expense by Vendor', desc: 'Net spend ranked by vendor', controls: ['period'], category: 'Management' },
  { key: 'InventoryValuation', name: 'Inventory Valuation', desc: 'On-hand quantity, cost & value by item', controls: ['asAt'], category: 'Management' },
  { key: 'FixedAssetRegister', name: 'Fixed-Asset Register', desc: 'Cost, depreciation & net book value', controls: ['asAt'], category: 'Management' },
  { key: 'BankReconSummary', name: 'Bank Reconciliation', desc: 'Statement vs GL balance, matched & unmatched', controls: ['asAt'], category: 'Management' },
  { key: 'BudgetVsActual', name: 'Budget vs Actual', desc: 'Actual vs budget by account, with variance', controls: ['period'], category: 'Management' },
  { key: 'DimensionalAnalysis', name: 'Dimensional Analysis', desc: 'Ledger movement grouped by a dimension (cost centre, project…)', controls: ['dimension', 'period'], category: 'Management' },
];

// Display order for the launcher grid.
export const REPORT_CATEGORIES: ReportCategory[] = [
  'Financial',
  'Receivables/Payables',
  'Tax',
  'Payroll',
  'Management',
];

// Kebab-case slug per report key, used for deep-linkable per-report routes.
const KEY_TO_SLUG: Record<string, string> = {
  TrialBalance: 'trial-balance',
  BalanceSheet: 'balance-sheet',
  ProfitAndLoss: 'profit-and-loss',
  CashFlow: 'cash-flow',
  GlDetail: 'general-ledger',
  ArAgeing: 'ar-ageing',
  ApAgeing: 'ap-ageing',
  CustomerStatement: 'customer-statement',
  VendorStatement: 'vendor-statement',
  VatReturn: 'vat',
  SalesTaxSummary: 'vat-by-rate',
  WhtCertificate: 'wht',
  PayrollSummary: 'payroll-summary',
  PayeP10: 'paye-p10',
  IncomeByCustomer: 'income-by-customer',
  ExpenseByVendor: 'expense-by-vendor',
  InventoryValuation: 'inventory-valuation',
  FixedAssetRegister: 'fixed-asset-register',
  BankReconSummary: 'bank-reconciliation',
  BudgetVsActual: 'budget-vs-actual',
  DimensionalAnalysis: 'dimensional-analysis',
};

const SLUG_TO_KEY: Record<string, string> = Object.fromEntries(
  Object.entries(KEY_TO_SLUG).map(([key, slug]) => [slug, key])
);

export function slugFor(key: string): string {
  return KEY_TO_SLUG[key] ?? key;
}

export function keyForSlug(slug: string): string | undefined {
  return SLUG_TO_KEY[slug];
}

export function metaForKey(key: string): ReportMeta | undefined {
  return reportTypes.find((r) => r.key === key);
}

// Build the report request payload — identical to the original buildReq().
export function buildReportRequest(meta: ReportMeta, p: ReportParams) {
  return {
    entity_id: ZERO_ENTITY,
    report_type: meta.key,
    parameters: {
      as_at: meta.controls.includes('asAt') ? p.asAt : null,
      period_from: meta.controls.includes('period') ? p.from : null,
      period_to: meta.controls.includes('period') ? p.to : null,
      account_code: meta.controls.includes('account') ? p.account : null,
      customer_id: meta.party === 'customer' ? p.partyId || null : null,
      vendor_id: meta.party === 'vendor' ? p.partyId || null : null,
      comparative: meta.comparable ? p.compare : false,
      dimension_type: meta.controls.includes('dimension') ? p.dimensionType || null : null,
    },
  };
}
