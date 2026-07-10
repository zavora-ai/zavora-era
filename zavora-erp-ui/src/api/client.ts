import axios from 'axios';

// ── Session state ───────────────────────────────────────────────────────────
// The access token lives in memory only (never localStorage), so an XSS payload
// cannot read it from storage and it is gone when the tab closes. The refresh
// token is held in an httpOnly cookie the browser sends automatically and JS
// cannot read at all.
let accessToken: string | null = null;
let identity: unknown = null;

export const getAccessToken = () => accessToken;
export const getIdentity = () => identity;

export function storeSession(data: { access_token: string; user?: unknown }) {
  accessToken = data.access_token ?? null;
  if (data.user !== undefined) identity = data.user;
}

export function clearSession() {
  accessToken = null;
  identity = null;
}

const api = axios.create({
  baseURL: '/api/v1',
  headers: { 'Content-Type': 'application/json' },
  withCredentials: true, // send/receive the httpOnly refresh cookie
});

// Attach the in-memory access token as a Bearer credential on every request.
api.interceptors.request.use((config) => {
  if (accessToken) config.headers['Authorization'] = `Bearer ${accessToken}`;
  return config;
});

// Exchange the httpOnly refresh cookie for a fresh access token. The cookie is
// sent automatically; no token is read or written by JS.
let refreshing: Promise<string | null> | null = null;

async function tryRefresh(): Promise<string | null> {
  try {
    const resp = await axios.post('/api/v1/auth/refresh', {}, { withCredentials: true });
    storeSession(resp.data);
    return resp.data.access_token as string;
  } catch {
    clearSession();
    return null;
  }
}

/** Restore a session on app load using the refresh cookie. Returns true if authed. */
export async function bootstrapAuth(): Promise<boolean> {
  if (accessToken) return true;
  const token = await tryRefresh();
  return token != null;
}

api.interceptors.response.use(
  (response) => response,
  async (error) => {
    const original = error.config;
    if (error.response?.status === 401 && original && !original._retried) {
      original._retried = true;
      refreshing = refreshing ?? tryRefresh();
      const newToken = await refreshing;
      refreshing = null;
      if (newToken) {
        original.headers['Authorization'] = `Bearer ${newToken}`;
        return api(original);
      }
      clearSession();
      if (window.location.pathname !== '/login') {
        window.location.href = '/login';
      }
    }
    return Promise.reject(error);
  }
);

export default api;

// === Auth & Users ===
export const login = (email: string, password: string) =>
  api.post('/auth/login', { email, password });
export const register = (data: { email: string; display_name: string; password: string }) =>
  api.post('/auth/register', data);
export const signup = (data: {
  organization_name: string;
  organization_type: string;
  kra_pin?: string;
  email: string;
  display_name: string;
  password: string;
  with_sample_data?: boolean;
  plan?: string;
}) => api.post('/auth/signup', data);
export const logout = () => api.post('/auth/logout', {});
// Subscription billing — start a Paystack checkout for the caller's plan.
// Free plans return { free: true }; paid plans return { authorization_url }.
export const billingCheckout = (plan: string, callback_url?: string) =>
  api.post('/billing/checkout', { plan, callback_url });
export const getSubscription = () => api.get('/billing/subscription');
export const cancelSubscription = () => api.post('/billing/cancel', {});
// === Tenant management (multi-tenant: a user may belong to several entities) ===
export const getMyTenants = (includeArchived = false) =>
  api.get('/auth/tenants', { params: includeArchived ? { include_archived: true } : undefined });
export const switchTenant = (entity_id: string) => api.post('/auth/switch-tenant', { entity_id });
export const createTenant = (data: { organization_name: string; organization_type: string; kra_pin?: string; with_sample_data?: boolean }) =>
  api.post('/auth/tenants', data);
export const archiveTenant = (entity_id: string) => api.post(`/auth/tenants/${entity_id}/archive`);
export const unarchiveTenant = (entity_id: string) => api.post(`/auth/tenants/${entity_id}/unarchive`);
export const leaveTenant = (entity_id: string) => api.post(`/auth/tenants/${entity_id}/leave`);
export const getUsers = () => api.get('/users');
export const createUser = (data: {
  email: string;
  display_name: string;
  role: string;
  password?: string;
}) => api.post('/users', data);
export const updateUser = (id: string, data: { display_name?: string; role?: string; is_active?: boolean }) =>
  api.put(`/users/${id}`, data);
export const resendInvite = (id: string) => api.post(`/users/${id}/resend-invite`, {});
// Assignable roles (system + tenant custom) for user dropdowns.
export const getRoles = () => api.get('/roles');
// Roles administration (Phase 3).
export const getPermissionsCatalog = () => api.get('/permissions');
export const getRole = (id: string) => api.get(`/roles/${id}`);
export const createRole = (data: { name: string; description?: string; permissions: string[] }) =>
  api.post('/roles', data);
export const updateRole = (id: string, data: { name?: string; description?: string; permissions?: string[] }) =>
  api.put(`/roles/${id}`, data);
export const deleteRole = (id: string) => api.delete(`/roles/${id}`);
// The current user's effective permissions (single source of truth for UI gating).
export const getMyPermissions = () => api.get('/auth/permissions');
// Internal-user activation + recovery (public; token-gated on the server).
export const setPassword = (token: string, password: string) =>
  api.post('/auth/set-password', { token, password });
export const forgotPassword = (email: string) => api.post('/auth/forgot-password', { email });

// === Dashboard ===
export const getDashboard = (asAt?: string) => api.get('/dashboard', { params: asAt ? { as_at: asAt } : {} });

// === Accounts ===
export const getAccounts = () => api.get('/accounts');
export const createAccount = (data: any) => api.post('/accounts', data);
export const updateAccount = (code: string, data: any) => api.put(`/accounts/${code}`, data);

// === Periods ===
export const getPeriods = () => api.get('/periods');
export const getBudgets = () => api.get('/budgets');
export const setBudget = (data: { period_id: string; account_code: string; amount: number }) =>
  api.put('/budgets', data);
export const getCustomReports = () => api.get('/custom-reports');
export const getCustomReport = (id: string) => api.get(`/custom-reports/${id}`);
export const saveCustomReport = (data: { id?: string; name: string; definition: any }) =>
  api.post('/custom-reports', data);
export const deleteCustomReport = (id: string) => api.delete(`/custom-reports/${id}`);
export const runCustomReport = (id: string, from: string, to: string) =>
  api.get(`/custom-reports/${id}/run`, { params: { from, to } });
export const getReportSchedules = () => api.get('/report-schedules');
export const saveReportSchedule = (data: { id?: string; name: string; report_type: string; cadence: string; recipients: string; is_active?: boolean }) =>
  api.post('/report-schedules', data);
export const deleteReportSchedule = (id: string) => api.delete(`/report-schedules/${id}`);
export const getConsolidationEntities = () => api.get('/consolidation/entities');
// === Posting groups (BC-style matrices) ===
export const getPostingGroups = () => api.get('/posting-groups');
export const createPostingGroup = (data: { kind: string; code: string; name: string }) =>
  api.post('/posting-groups/group', data);
export const assignPostingGroups = (data: { kind: 'customer' | 'vendor' | 'product'; id: string; general_group_id?: string; vat_group_id?: string }) =>
  api.post('/posting-groups/assign', data);
export const upsertGeneralMatrix = (data: { gen_biz_group_id: string; gen_prod_group_id: string; sales_account?: string; purchase_account?: string; cogs_account?: string }) =>
  api.post('/posting-groups/general-matrix', data);
export const upsertVatMatrix = (data: { vat_biz_group_id: string; vat_prod_group_id: string; vat_rate: number; vat_output_account?: string; vat_input_account?: string }) =>
  api.post('/posting-groups/vat-matrix', data);
export const upsertBusinessControl = (data: { gen_biz_group_id: string; receivables_account?: string; payables_account?: string }) =>
  api.post('/posting-groups/business-control', data);
export const runConsolidatedTrialBalance = (data: { entity_ids: string[]; as_at: string }) =>
  api.post('/consolidation/trial-balance', data);
export const postOpeningBalances = (data: { as_of_date: string; lines: { account_code: string; debit?: number; credit?: number }[] }) =>
  api.post('/opening-balances', data);
export const getRecurringJournals = () => api.get('/recurring-journals');
export const saveRecurringJournal = (data: any) => api.post('/recurring-journals', data);
export const deleteRecurringJournal = (id: string) => api.delete(`/recurring-journals/${id}`);
export const runRecurringJournals = () => api.post('/recurring-journals/run', {});
export const getTaxFilings = () => api.get('/tax-filings');
export const fileTaxReturn = (data: { tax_type: string; period_from: string; period_to: string; amount: number }) =>
  api.post('/tax-filings', data);
export const remitTaxFiling = (id: string, data: { liability_account: string; bank_account_code: string; payment_date: string }) =>
  api.post(`/tax-filings/${id}/remit`, data);
export const getCitEstimate = (params?: { fiscal_year?: number; adjustments?: number }) =>
  api.get('/tax/cit/estimate', { params });
export const getCashForecast = (weeks = 13) => api.get('/forecasts/cash', { params: { weeks } });
export const getWhtRates = () => api.get('/wht-rates');
export const updateWhtRate = (data: { category: string; resident_rate: number; non_resident_rate: number }) =>
  api.put('/wht-rates', data);
export const getDimensions = () => api.get('/dimensions');
export const createDimensionType = (data: { code: string; name: string }) =>
  api.post('/dimension-types', data);
export const createDimensionValue = (data: { type_code: string; code: string; name: string }) =>
  api.post('/dimension-values', data);
export const generatePeriods = (data: { fiscal_year: number; year_start_month: number }) =>
  api.post('/periods', data);
export const closePeriod = (id: string, data: { close_type: 'Soft' | 'Hard' }) =>
  api.post(`/periods/${id}/close`, data);
export const reopenPeriod = (id: string, data: { reason: string }) =>
  api.post(`/periods/${id}/reopen`, data);
export const yearEndClose = (data: { fiscal_year: number }) =>
  api.post('/periods/year-end-close', data);

// === Journal Entries ===
export type PageParams = { limit?: number; offset?: number };
export const getJournalEntries = (params?: PageParams) => api.get('/journal-entries', { params });
export const getJournalEntry = (id: string) => api.get(`/journal-entries/${id}`);
export const createJournalEntry = (data: any) => api.post('/journal-entries', data);
export const validateJournalEntry = (data: any) => api.post('/journal-entries/validate', data);
export const reverseJournalEntry = (id: string, data: { reason?: string }) =>
  api.post(`/journal-entries/${id}/reverse`, data);

// === Customers ===
export const getCustomers = () => api.get('/customers');
export const createCustomer = (data: any) => api.post('/customers', data);

// === Vendors ===
export const getVendors = () => api.get('/vendors');
export const createVendor = (data: any) => api.post('/vendors', data);

// === Employees ===
export const createEmployee = (data: any) => api.post('/employees', data);

// === Products ===
export const getProducts = () => api.get('/products');
export const getProduct = (id: string) => api.get(`/products/${id}`);
export const createProduct = (data: any) => api.post('/products', data);
export const updateProduct = (id: string, data: any) => api.put(`/products/${id}`, data);
export const deleteProduct = (id: string) => api.delete(`/products/${id}`);

// === Invoices ===
export const getInvoices = (params?: PageParams) => api.get('/invoices', { params });
export const createInvoice = (data: any) => api.post('/invoices', data);
export const updateInvoice = (id: string, data: any) => api.put(`/invoices/${id}`, data);
export const deleteInvoice = (id: string) => api.delete(`/invoices/${id}`);
export const postInvoice = (id: string) => api.post(`/invoices/${id}/post`);
export const sendInvoice = (id: string, data?: any) => api.post(`/invoices/${id}/send`, data || {});
export const getInvoiceTemplates = () => api.get('/invoice-templates');
/** Fetch the shared invoice document (source of truth for screen + PDF). */
export const getInvoiceDocumentHtml = (id: string) =>
  api.get(`/invoices/${id}/document`, { params: { format: 'html' }, responseType: 'text' });
export const getInvoiceDocumentPdf = (id: string) =>
  api.get(`/invoices/${id}/document`, { params: { format: 'pdf' }, responseType: 'blob' });
export const writeOffInvoice = (id: string, data: { expense_account: string; amount?: number; reason?: string }) =>
  api.post(`/invoices/${id}/write-off`, data);

// === Estimates ===
export const getEstimates = (params?: PageParams) => api.get('/estimates', { params });
export const createEstimate = (data: any) => api.post('/estimates', data);
export const updateEstimate = (id: string, data: any) => api.put(`/estimates/${id}`, data);
export const deleteEstimate = (id: string) => api.delete(`/estimates/${id}`);
export const getEstimate = (id: string) => api.get(`/estimates/${id}`);
/** Shared estimate document (same renderer as invoices) for screen + PDF. */
export const getEstimateDocumentHtml = (id: string) =>
  api.get(`/estimates/${id}/document`, { params: { format: 'html' }, responseType: 'text' });
export const getEstimateDocumentPdf = (id: string) =>
  api.get(`/estimates/${id}/document`, { params: { format: 'pdf' }, responseType: 'blob' });
export const convertEstimate = (id: string, data?: any) => api.post(`/estimates/${id}/convert`, data || {});
export const sendEstimate = (id: string) => api.post(`/estimates/${id}/send`, {});
export const acceptEstimate = (id: string) => api.post(`/estimates/${id}/accept`, {});
export const declineEstimate = (id: string) => api.post(`/estimates/${id}/decline`, {});

// === Recurring Invoices ===
export const getRecurringInvoices = () => api.get('/recurring-invoices');
export const createRecurringInvoice = (data: any) => api.post('/recurring-invoices', data);
export const updateRecurringInvoice = (id: string, data: any) => api.put(`/recurring-invoices/${id}`, data);
export const deleteRecurringInvoice = (id: string) => api.delete(`/recurring-invoices/${id}`);
/** Preview document (next invoice the schedule will generate). */
export const getRecurringDocumentHtml = (id: string) =>
  api.get(`/recurring-invoices/${id}/document`, { params: { format: 'html' }, responseType: 'text' });
export const getRecurringDocumentPdf = (id: string) =>
  api.get(`/recurring-invoices/${id}/document`, { params: { format: 'pdf' }, responseType: 'blob' });
/** Invoices actually generated by this recurring template. */
export const getRecurringInvoiceHistory = (id: string) =>
  api.get(`/recurring-invoices/${id}/invoices`);

// === Bills ===
export const getBills = (params?: PageParams) => api.get('/bills', { params });
export const getBill = (id: string) => api.get(`/bills/${id}`);
export const createBill = (data: any) => api.post('/bills', data);
export const updateBill = (id: string, data: any) => api.put(`/bills/${id}`, data);
export const deleteBill = (id: string) => api.delete(`/bills/${id}`);
export const approveBill = (id: string) => api.post(`/bills/${id}/approve`);
export const postBill = (id: string) => api.post(`/bills/${id}/post`);

// === Supplier Credit Notes (AP) ===
export const getSupplierCreditNotes = () => api.get('/supplier-credit-notes');
export const getSupplierCreditNote = (id: string) => api.get(`/supplier-credit-notes/${id}`);
export const createSupplierCreditNote = (data: any) => api.post('/supplier-credit-notes', data);

// === Payments ===
export const getPayments = (params?: { status?: string } & PageParams) => api.get('/payments', { params });
export const getPayment = (id: string) => api.get(`/payments/${id}`);
export const recordPayment = (data: any) => api.post('/payments', data);
export const applyPayment = (data: { payment_id: string; document_id: string; amount: number }) =>
  api.post('/payments/apply', data);

// === Payroll ===
export const runPayroll = (data: any) => api.post('/payroll/run', data);
export const approvePayRun = (id: string) => api.post(`/payroll/${id}/approve`);
export const postPayRun = (id: string) => api.post(`/payroll/${id}/post`);
export const markPayRunPaid = (id: string) => api.post(`/payroll/${id}/paid`);
export const listPayRuns = () => api.get('/payroll');
export const getPayRun = (id: string) => api.get(`/payroll/${id}`);
export const recomputePayRun = (id: string) => api.post(`/payroll/${id}/recompute`);
export const deletePayRun = (id: string) => api.delete(`/payroll/${id}`);
export const listRunInputs = (id: string) => api.get(`/payroll/${id}/inputs`);
export const addRunInput = (id: string, data: any) => api.post(`/payroll/${id}/inputs`, data);
export const deleteRunInput = (id: string, inputId: string) => api.delete(`/payroll/${id}/inputs/${inputId}`);
// Payroll masters & config
export const listEarningTypes = () => api.get('/payroll/earning-types');
export const createEarningType = (data: any) => api.post('/payroll/earning-types', data);
export const setEarningTypeActive = (id: string, active: boolean) => api.put(`/payroll/earning-types/${id}/active`, { active });
export const listDeductionTypes = () => api.get('/payroll/deduction-types');
export const createDeductionType = (data: any) => api.post('/payroll/deduction-types', data);
export const setDeductionTypeActive = (id: string, active: boolean) => api.put(`/payroll/deduction-types/${id}/active`, { active });
export const listDepartments = () => api.get('/payroll/departments');
export const createDepartment = (data: any) => api.post('/payroll/departments', data);
export const listStatutoryConfig = () => api.get('/payroll/statutory-config');
export const upsertStatutoryConfig = (data: any) => api.post('/payroll/statutory-config', data);
export const listRecurringItems = (employeeId: string) => api.get('/payroll/recurring-items', { params: { employee_id: employeeId } });
export const createRecurringItem = (data: any) => api.post('/payroll/recurring-items', data);
export const deleteRecurringItem = (id: string) => api.delete(`/payroll/recurring-items/${id}`);
export const listLoans = (employeeId: string) => api.get('/payroll/loans', { params: { employee_id: employeeId } });
export const createLoan = (data: any) => api.post('/payroll/loans', data);

// === Reports ===
export const generateReport = (data: any) => api.post('/reports', data);

// === Settings ===
export const getSettings = () => api.get('/settings');

// === Public invoice pay-link (no auth; the token is the credential) ===
export interface PublicInvoiceView {
  number: string;
  company_name: string;
  currency: string;
  gross_total: string;
  amount_paid: string;
  balance_due: string;
  status: string;
  issue_date: string;
  due_date: string;
  payable: boolean;
}
export const getPublicInvoice = (token: string) => api.get<PublicInvoiceView>(`/public/invoices/${token}`);
export const payPublicInvoice = (token: string, body: { email?: string; callback_url?: string }) =>
  api.post<{ authorization_url: string; reference: string }>(`/public/invoices/${token}/pay`, body);
export const updateSettings = (data: any) => api.put('/settings', data);

// === Transactions (categorisation queue) ===
export const getTransactions = (params?: any) => api.get('/transactions', { params });
export const categoriseTransaction = (id: string, data: any) => api.post(`/transactions/${id}/categorise`, data);
export const splitTransaction = (id: string, data: any) => api.post(`/transactions/${id}/split`, data);
export const mergeTransactions = (data: any) => api.post('/transactions/merge', data);
export const excludeTransaction = (id: string, data: any) => api.post(`/transactions/${id}/exclude`, data);

// === Bank ===
export const adjustInventory = (data: { item_id: string; counted_quantity: number; adjustment_account: string; reason?: string }) =>
  api.post('/inventory/adjust', data);
export const getBankAccounts = () => api.get('/bank-accounts');
export const computeBankRec = (data: { bank_account_id: string; statement_date: string }) =>
  api.post('/bank/reconciliations/compute', data);
export const completeBankRec = (data: { bank_account_id: string; statement_date: string; statement_closing_balance: number; cleared_entry_ids: string[] }) =>
  api.post('/bank/reconciliations/complete', data);
export const getBankRecs = (bankAccountId?: string) =>
  api.get('/bank/reconciliations', { params: bankAccountId ? { bank_account_id: bankAccountId } : {} });
export const createBankAccount = (data: any) => api.post('/bank-accounts', data);
export const deleteBankAccount = (id: string) => api.delete(`/bank-accounts/${id}`);
export const importStatement = (data: any) => api.post('/bank/import', data);
// PDF / Excel bank-statement extraction (review-before-commit). Returns candidate rows.
export const extractBankStatement = (file: File) => {
  const fd = new FormData();
  fd.append('file', file);
  return api.post('/bank/import/extract', fd, { headers: { 'Content-Type': 'multipart/form-data' } });
};
export const reconcileStatement = (id: string) => api.post(`/bank/reconcile/${id}`);
export const confirmMatch = (data: any) => api.post('/bank/confirm-match', data);

// === Inventory ===
export const getInventory = () => api.get('/inventory');
export const createInventoryItem = (data: any) => api.post('/inventory', data);
export const receiveInventory = (data: any) => api.post('/inventory/receive', data);
export const issueInventory = (data: any) => api.post('/inventory/issue', data);

// ── Point of Sale ────────────────────────────────────────────────────────────
export const getPosSession = () => api.get('/pos/session');
export const getPosSessions = () => api.get('/pos/sessions');
export const openPosSession = (data: { register_name?: string; opening_float: number }) => api.post('/pos/session/open', data);
export const completePosSale = (sessionId: string, data: {
  customer_id?: string; tender: 'cash' | 'mpesa' | 'card'; amount_tendered?: number;
  mpesa_reference?: string; mpesa_phone?: string;
  lines: { product_id: string; quantity: number; unit_price?: number }[];
}) => api.post(`/pos/session/${sessionId}/sale`, data);
export const getZReport = (sessionId: string) => api.get(`/pos/session/${sessionId}/z-report`);
export const closePosSession = (sessionId: string, data: { counted_cash: number; notes?: string }) =>
  api.post(`/pos/session/${sessionId}/close`, data);
export const getPosReceipt = (invoiceId: string, tendered?: number) =>
  api.get(`/pos/receipt/${invoiceId}`, { params: tendered != null ? { tendered } : {}, responseType: 'text' });

// ── KRA eTIMS OSCU/VSCU ──
export const getEtimsConfig = () => api.get('/etims/config');
export const saveEtimsConfig = (data: {
  enabled?: boolean; environment?: string; pin?: string; bhf_id?: string; dvc_srl_no?: string;
}) => api.put('/etims/config', data);
export const initializeEtims = () => api.post('/etims/initialize', {});
// Real OSCU/VSCU transmission (distinct from the legacy manual mark below).
export const transmitInvoiceKra = (invoiceId: string) => api.post(`/etims/invoices/${invoiceId}/transmit`, {});
export const registerEtimsProduct = (productId: string) => api.post(`/etims/products/${productId}/register`, {});

// === Assets ===
export const getAssets = () => api.get('/assets');
// Amortisation schedules (prepayments & deferred revenue).
export const getAmortization = () => api.get('/amortization');
export const createAmortization = (data: {
  kind: string; description: string; balance_account: string; pnl_account: string;
  total_amount: number; periods: number; start_date: string;
}) => api.post('/amortization', data);
export const runAmortization = () => api.post('/amortization/run', {});
export const cancelAmortization = (id: string) => api.post(`/amortization/${id}/cancel`, {});
export const createAsset = (data: any) => api.post('/assets', data);
export const runDepreciation = () => api.post('/assets/depreciation/run');

// === FX Rates ===
export const getFxRates = () => api.get('/fx-rates');
export const upsertFxRate = (data: any) => api.post('/fx-rates', data);
export const deleteFxRate = (id: string) => api.delete(`/fx-rates/${id}`);
export const runFxRevaluation = () => api.post('/fx/revaluation');

// === Audit ===
export const getAuditEvents = (params?: any) => api.get('/audit', { params });
export const getAuditForObject = (type: string, id: string) => api.get(`/audit/${type}/${id}`);

// === M-Pesa ===
export const mpesaStkPush = (data: { invoice_id: string; phone: string }) =>
  api.post('/payments/mpesa-stk-push', data);

// === Receipts ===
export const captureReceipt = (formData: FormData) =>
  api.post('/receipts/capture', formData, { headers: { 'Content-Type': 'multipart/form-data' } });
export const confirmReceipt = (data: { capture_id: string; vendor_id: string; adjustments: any }) =>
  api.post('/receipts/confirm', data);

// === Document attachments (source files linked to bills/invoices/etc.) ===
export const getAttachments = (linked_type: string, linked_id: string) =>
  api.get('/attachments', { params: { linked_type, linked_id } });
export const uploadAttachment = (linked_type: string, linked_id: string, file: File) => {
  const fd = new FormData();
  fd.append('linked_type', linked_type);
  fd.append('linked_id', linked_id);
  fd.append('file', file);
  return api.post('/attachments', fd, { headers: { 'Content-Type': 'multipart/form-data' } });
};
export const getAttachment = (id: string) => api.get(`/attachments/${id}`);
export const deleteAttachment = (id: string) => api.delete(`/attachments/${id}`);

// === Accounts Seed ===
export const seedAccounts = () => api.post('/accounts/seed');

// === Invoices (additional) ===
export const getInvoice = (id: string) => api.get(`/invoices/${id}`);
export const createCreditNote = (id: string, data: any) => api.post(`/invoices/${id}/credit-note`, data);
export const transmitInvoiceEtims = (id: string, data: { etims_invoice_number?: string }) =>
  api.post(`/invoices/${id}/etims-transmit`, data);

// === Employees (additional) ===
export const getEmployees = () => api.get('/employees');
export const getEmployee = (id: string) => api.get(`/employees/${id}`);
export const createEmployeeApi = (data: any) => api.post('/employees', data);
export const updateEmployee = (id: string, data: any) => api.put(`/employees/${id}`, data);

// === Customers (additional) ===
export const getCustomer = (id: string) => api.get(`/customers/${id}`);
export const updateCustomer = (id: string, data: any) => api.put(`/customers/${id}`, data);
export const getCustomerStatement = (id: string) => api.get(`/customers/${id}/statement`);
export const sendCustomerStatement = (id: string, channel: string) => api.post(`/customers/${id}/send-statement`, { channel });

// === Vendors (additional) ===
export const getVendor = (id: string) => api.get(`/vendors/${id}`);
export const updateVendor = (id: string, data: any) => api.put(`/vendors/${id}`, data);

// === Notifications (in-app inbox) ===
export const getNotifications = (params?: { unread_only?: boolean } & PageParams) => api.get('/notifications', { params });
export const getUnreadCount = () => api.get('/notifications/unread-count');
export const markNotificationRead = (id: string) => api.patch(`/notifications/${id}/read`);
export const markAllNotificationsRead = () => api.post('/notifications/mark-all-read', {});
// Admin delivery history (Owner/Admin): all channels with status/recipient/error.
export interface DeliveryFilters {
  channel?: string;
  status?: string;
  event_type?: string;
  search?: string;
  from?: string;
  to?: string;
}
export const getNotificationDelivery = (params?: DeliveryFilters & PageParams) =>
  api.get('/notifications/delivery', { params });
export const getNotificationDeliveryStats = () => api.get('/notifications/delivery/stats');
// Notification event preferences (Owner/Admin): per-event enabled + channels.
export interface EventPref {
  event_type: string;
  enabled: boolean;
  channels: string[];
  is_default: boolean;
}
export interface ChannelStatus {
  channel: string;
  configured: boolean;
}
export const getNotificationSettings = () => api.get('/notification-settings');
export const updateNotificationSettings = (events: Omit<EventPref, 'is_default'>[]) =>
  api.put('/notification-settings', { events });
// Per-tenant notification providers (Owner/Admin). Secrets are write-only.
export interface MaskedProvider {
  channel: string;
  enabled: boolean;
  settings: Record<string, any>;
  has_secret: boolean;
}
export const getNotificationProviders = () => api.get('/notification-providers');
export const putNotificationProvider = (data: {
  channel: string;
  enabled: boolean;
  settings: Record<string, any>;
  secret?: string;
}) => api.put('/notification-providers', data);
export const testNotificationProvider = (channel: string, recipient: string) =>
  api.post(`/notification-providers/${channel}/test`, { recipient });

// === Reports (additional) ===
export const exportReport = (data: any) => api.post('/reports/export', data, { responseType: 'blob' });

// === HR — Leave & ESS ===
export const getLeaveTypes = () => api.get('/leave-types');
export const createLeaveType = (data: any) => api.post('/leave-types', data);
export const setLeaveTypeActive = (id: string, active: boolean) => api.put(`/leave-types/${id}/active`, { active });
export const getHolidays = () => api.get('/holidays');
export const getLeaveCalendar = (from?: string, to?: string) =>
  api.get('/leave-calendar', { params: { ...(from ? { from } : {}), ...(to ? { to } : {}) } });
export const getPayslipPdf = (runId: string, employeeId: string) =>
  api.get(`/payroll/${runId}/payslips/${employeeId}/pdf`, { responseType: 'blob' });
export const createHoliday = (data: { date: string; name: string; recurring?: boolean }) => api.post('/holidays', data);
export const deleteHoliday = (id: string) => api.delete(`/holidays/${id}`);
export const getLeaveBalances = (employeeId: string, year?: number) =>
  api.get('/leave-balances', { params: { employee_id: employeeId, ...(year ? { year } : {}) } });
export const getLeaveRequests = (params?: { employee_id?: string; status?: string; mine?: boolean }) =>
  api.get('/leave-requests', { params: params || {} });
export const createLeaveRequest = (data: any) => api.post('/leave-requests', data);
export const approveLeave = (id: string, note?: string) => api.post(`/leave-requests/${id}/approve`, { note });
export const declineLeave = (id: string, note?: string) => api.post(`/leave-requests/${id}/decline`, { note });
export const inviteEss = (employeeId: string, email: string, password?: string) =>
  api.post(`/employees/${employeeId}/invite-ess`, { email, ...(password ? { password } : {}) });

// === HR — Onboarding ===
export const getOnboardingCases = () => api.get('/onboarding');
export const createOnboarding = (data: any) => api.post('/onboarding', data);
export const getOnboardingCase = (id: string) => api.get(`/onboarding/${id}`);
export const setOnboardingTask = (caseId: string, taskId: string, done: boolean) => api.put(`/onboarding/${caseId}/tasks/${taskId}`, { done });
export const completeOnboarding = (id: string) => api.post(`/onboarding/${id}/complete`, {});

// === Procurement (P2P) — staff/buyer side ===
export const getVendorApplications = () => api.get('/vendor-applications');
export const approveVendorApplication = (id: string, data?: { vendor_id?: string }) =>
  api.post(`/vendor-applications/${id}/approve`, data || {});
export const rejectVendorApplication = (id: string) => api.post(`/vendor-applications/${id}/reject`, {});
export const getTenders = () => api.get('/tenders');
export const getTender = (id: string) => api.get(`/tenders/${id}`);
export const createTender = (data: {
  title: string; description?: string; category?: string; closing_date?: string;
  lines: { description: string; quantity?: number; uom?: string }[];
}) => api.post('/tenders', data);
export const publishTender = (id: string) => api.post(`/tenders/${id}/publish`, {});
export const getTenderBids = (id: string) => api.get(`/tenders/${id}/bids`);
export const awardTender = (id: string, data: { bid_id: string; delivery_date?: string; notes?: string }) =>
  api.post(`/tenders/${id}/award`, data);
export const getPurchaseOrders = () => api.get('/purchase-orders');
export const getPurchaseOrder = (id: string) => api.get(`/purchase-orders/${id}`);
/** Direct procurement — raise an LPO straight against a vendor master. */
export const createPurchaseOrder = (data: {
  vendor_id: string; currency?: string; delivery_date?: string; notes?: string;
  lines: { description: string; quantity: number; uom: string; unit_price: number; account_code?: string }[];
}) => api.post('/purchase-orders', data);

// ── Purchase requisitions ────────────────────────────────────────────────────
export const getRequisitions = () => api.get('/requisitions');
export const getRequisition = (id: string) => api.get(`/requisitions/${id}`);
export const createRequisition = (data: {
  title: string; justification?: string; department?: string; cost_center?: string;
  currency?: string; needed_by?: string; notes?: string;
  lines: { description: string; quantity: number; uom: string; estimated_unit_price: number; account_code?: string }[];
}) => api.post('/requisitions', data);
export const submitRequisition = (id: string) => api.post(`/requisitions/${id}/submit`, {});
export const approveRequisition = (id: string) => api.post(`/requisitions/${id}/approve`, {});
export const rejectRequisition = (id: string, reason?: string) => api.post(`/requisitions/${id}/reject`, { reason });
export const convertRequisition = (id: string, data: {
  target: 'tender' | 'purchase_order'; vendor_id?: string; delivery_date?: string; closing_date?: string;
}) => api.post(`/requisitions/${id}/convert`, data);
/** The legal LPO document as a PDF blob (bank-ready). */
export const getPurchaseOrderPdf = (id: string) =>
  api.get(`/purchase-orders/${id}/document`, { params: { format: 'pdf' }, responseType: 'blob' });
/** Goods receipts + 3-way match for a PO. */
export const getGoodsReceipts = (poId: string) => api.get(`/purchase-orders/${poId}/receipts`);
export const createGoodsReceipt = (poId: string, data: {
  receipt_date?: string; notes?: string;
  lines: { po_line_id?: string; description: string; quantity_received: number }[];
}) => api.post(`/purchase-orders/${poId}/receipts`, data);
export const getPoMatch = (poId: string) => api.get(`/purchase-orders/${poId}/match`);
export const getProcurementAnalytics = () => api.get('/procurement/analytics');
export const getBudgetControl = () => api.get('/procurement/budget-control');
/** Email the LPO PDF to the vendor. */
export const sendPurchaseOrder = (id: string, data: { recipient_email?: string; message?: string }) =>
  api.post(`/purchase-orders/${id}/send`, data);

// ── Approval spend-limits (DoA) ──────────────────────────────────────────────
export const getApprovalLimits = () => api.get('/approval-limits');
export const setApprovalLimit = (role: string, max_amount: number | null) =>
  api.put('/approval-limits', { role, max_amount });

// ── Purchase debit notes ─────────────────────────────────────────────────────
export const getDebitNotes = () => api.get('/debit-notes');
export const getDebitNote = (id: string) => api.get(`/debit-notes/${id}`);
export const createDebitNote = (data: {
  vendor_id: string; applies_to_bill?: string; po_id?: string; reason?: string; currency?: string;
  lines: { description: string; quantity: number; unit_price: number; account_code?: string }[];
}) => api.post('/debit-notes', data);

// ── Expense claims ───────────────────────────────────────────────────────────
export const getExpenseClaims = () => api.get('/expense-claims');
export const getExpenseClaim = (id: string) => api.get(`/expense-claims/${id}`);
export const createExpenseClaim = (data: {
  title: string; currency?: string; notes?: string;
  lines: { expense_date?: string; description: string; account_code?: string; amount: number }[];
}) => api.post('/expense-claims', data);
export const submitExpenseClaim = (id: string) => api.post(`/expense-claims/${id}/submit`, {});
export const approveExpenseClaim = (id: string) => api.post(`/expense-claims/${id}/approve`, {});
export const rejectExpenseClaim = (id: string, reason?: string) => api.post(`/expense-claims/${id}/reject`, { reason });


// === CRM (optional, feature-flagged add-in) ===
// Settings (reachable even when disabled, so an admin can turn it on).
export const getCrmSettings = () => api.get('/crm/settings');
export const setCrmEnabled = (enabled: boolean) => api.put('/crm/settings', { enabled });
// Pipelines & stages
export const getCrmPipelines = () => api.get('/crm/pipelines');
export const getCrmStages = (pipelineId: string) => api.get(`/crm/pipelines/${pipelineId}/stages`);
// Leads
export const getCrmLeads = (status?: string) => api.get('/crm/leads', { params: status ? { status } : {} });
export const createCrmLead = (data: {
  name: string; company?: string; email?: string; phone?: string; source?: string;
  estimated_value?: number; notes?: string;
}) => api.post('/crm/leads', data);
export const updateCrmLead = (id: string, data: any) => api.put(`/crm/leads/${id}`, data);
export const convertCrmLead = (id: string, data?: { customer_id?: string; amount?: number }) =>
  api.post(`/crm/leads/${id}/convert`, data || {});
// Opportunities
export const getCrmOpportunities = (status?: string) =>
  api.get('/crm/opportunities', { params: status ? { status } : {} });
export const createCrmOpportunity = (data: {
  name: string; amount?: number; customer_id?: string; pipeline_id?: string; stage_id?: string;
  expected_close?: string; notes?: string;
}) => api.post('/crm/opportunities', data);
export const moveCrmOpportunity = (id: string, stage_id: string) =>
  api.post(`/crm/opportunities/${id}/move`, { stage_id });
export const winCrmOpportunity = (id: string, data?: { amount?: number }) =>
  api.post(`/crm/opportunities/${id}/win`, data || {});
export const loseCrmOpportunity = (id: string, reason?: string) =>
  api.post(`/crm/opportunities/${id}/lose`, { reason });
// Activities
export const getCrmActivities = (params?: { related_type?: string; related_id?: string }) =>
  api.get('/crm/activities', { params: params || {} });
export const createCrmActivity = (data: {
  kind: string; subject: string; due_at?: string; related_type?: string; related_id?: string; notes?: string;
}) => api.post('/crm/activities', data);
export const completeCrmActivity = (id: string) => api.post(`/crm/activities/${id}/done`, {});
// Tickets (back-office side)
export const getCrmTickets = (status?: string) => api.get('/crm/tickets', { params: status ? { status } : {} });
export const getCrmTicket = (id: string) => api.get(`/crm/tickets/${id}`);
export const replyCrmTicket = (id: string, body: string) => api.post(`/crm/tickets/${id}/reply`, { body });
export const setCrmTicketStatus = (id: string, status: string) => api.post(`/crm/tickets/${id}/status`, { status });
// Analytics
export const getCrmAnalytics = () => api.get('/crm/analytics');
// Assisted portal invite
export const inviteCustomerPortal = (data: { email: string; display_name?: string; customer_id?: string; password?: string }) =>
  api.post('/crm/customers/invite-portal', data);
