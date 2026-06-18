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
export const getUsers = () => api.get('/users');
export const createUser = (data: {
  email: string;
  display_name: string;
  role: string;
  password?: string;
}) => api.post('/users', data);

// === Dashboard ===
export const getDashboard = () => api.get('/dashboard');

// === Accounts ===
export const getAccounts = () => api.get('/accounts');
export const createAccount = (data: any) => api.post('/accounts', data);
export const updateAccount = (code: string, data: any) => api.put(`/accounts/${code}`, data);

// === Periods ===
export const getPeriods = () => api.get('/periods');
export const getBudgets = () => api.get('/budgets');
export const setBudget = (data: { period_id: string; account_code: string; amount: number }) =>
  api.put('/budgets', data);
export const generatePeriods = (data: { fiscal_year: number; year_start_month: number }) =>
  api.post('/periods', data);
export const closePeriod = (id: string, data: { close_type: 'Soft' | 'Hard' }) =>
  api.post(`/periods/${id}/close`, data);
export const reopenPeriod = (id: string, data: { reason: string }) =>
  api.post(`/periods/${id}/reopen`, data);

// === Journal Entries ===
export const getJournalEntries = () => api.get('/journal-entries');
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
export const createProduct = (data: any) => api.post('/products', data);

// === Invoices ===
export const getInvoices = () => api.get('/invoices');
export const createInvoice = (data: any) => api.post('/invoices', data);
export const updateInvoice = (id: string, data: any) => api.put(`/invoices/${id}`, data);
export const deleteInvoice = (id: string) => api.delete(`/invoices/${id}`);
export const postInvoice = (id: string) => api.post(`/invoices/${id}/post`);
export const sendInvoice = (id: string, data?: any) => api.post(`/invoices/${id}/send`, data || {});

// === Estimates ===
export const getEstimates = () => api.get('/estimates');
export const createEstimate = (data: any) => api.post('/estimates', data);
export const getEstimate = (id: string) => api.get(`/estimates/${id}`);
export const convertEstimate = (id: string, data?: any) => api.post(`/estimates/${id}/convert`, data || {});
export const sendEstimate = (id: string) => api.post(`/estimates/${id}/send`, {});
export const acceptEstimate = (id: string) => api.post(`/estimates/${id}/accept`, {});
export const declineEstimate = (id: string) => api.post(`/estimates/${id}/decline`, {});

// === Recurring Invoices ===
export const getRecurringInvoices = () => api.get('/recurring-invoices');
export const createRecurringInvoice = (data: any) => api.post('/recurring-invoices', data);

// === Bills ===
export const getBills = () => api.get('/bills');
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
export const getPayments = (params?: { status?: string }) => api.get('/payments', { params });
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
export const getBankAccounts = () => api.get('/bank-accounts');
export const createBankAccount = (data: any) => api.post('/bank-accounts', data);
export const deleteBankAccount = (id: string) => api.delete(`/bank-accounts/${id}`);
export const importStatement = (data: any) => api.post('/bank/import', data);
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

// === Vendors (additional) ===
export const getVendor = (id: string) => api.get(`/vendors/${id}`);
export const updateVendor = (id: string, data: any) => api.put(`/vendors/${id}`, data);

// === Reports (additional) ===
export const exportReport = (data: any) => api.post('/reports/export', data, { responseType: 'blob' });
