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
}) => api.post('/auth/signup', data);
export const logout = () => api.post('/auth/logout', {});
// === Tenant management (multi-tenant: a user may belong to several entities) ===
export const getMyTenants = (includeArchived = false) =>
  api.get('/auth/tenants', { params: includeArchived ? { include_archived: true } : undefined });
export const switchTenant = (entity_id: string) => api.post('/auth/switch-tenant', { entity_id });
export const createTenant = (data: { organization_name: string; organization_type: string; kra_pin?: string }) =>
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

// === Reports ===
export const generateReport = (data: any) => api.post('/reports', data);

// === Settings ===
export const getSettings = () => api.get('/settings');
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

// === Assets ===
export const getAssets = () => api.get('/assets');
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
