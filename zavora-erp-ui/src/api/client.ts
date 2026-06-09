import axios from 'axios';

const api = axios.create({
  baseURL: '/api/v1',
  headers: { 'Content-Type': 'application/json' },
});

// Request interceptor for auth token
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('era_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

// Response interceptor for error handling
api.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      localStorage.removeItem('era_token');
      window.location.href = '/login';
    }
    return Promise.reject(error);
  }
);

export default api;

// === Dashboard ===
export const getDashboard = () => api.get('/dashboard');

// === Accounts ===
export const getAccounts = () => api.get('/accounts');
export const createAccount = (data: any) => api.post('/accounts', data);
export const updateAccount = (code: string, data: any) => api.put(`/accounts/${code}`, data);

// === Periods ===
export const getPeriods = () => api.get('/periods');
export const generatePeriods = (data: any) => api.post('/periods', data);
export const closePeriod = (id: string, data: any) => api.post(`/periods/${id}/close`, data);

// === Journal Entries ===
export const createJournalEntry = (data: any) => api.post('/journal-entries', data);
export const validateJournalEntry = (data: any) => api.post('/journal-entries/validate', data);

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
export const postInvoice = (id: string) => api.post(`/invoices/${id}/post`);
export const sendInvoice = (id: string, data: any) => api.post(`/invoices/${id}/send`, data);

// === Estimates ===
export const getEstimates = () => api.get('/estimates');
export const createEstimate = (data: any) => api.post('/estimates', data);
export const getEstimate = (id: string) => api.get(`/estimates/${id}`);
export const convertEstimate = (id: string, data?: any) => api.post(`/estimates/${id}/convert`, data || {});

// === Recurring Invoices ===
export const getRecurringInvoices = () => api.get('/recurring-invoices');
export const createRecurringInvoice = (data: any) => api.post('/recurring-invoices', data);

// === Bills ===
export const getBills = () => api.get('/bills');
export const getBill = (id: string) => api.get(`/bills/${id}`);
export const createBill = (data: any) => api.post('/bills', data);
export const approveBill = (id: string) => api.post(`/bills/${id}/approve`);
export const postBill = (id: string) => api.post(`/bills/${id}/post`);

// === Payments ===
export const getPayments = () => api.get('/payments');
export const recordPayment = (data: any) => api.post('/payments', data);

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

// === Accounts Seed ===
export const seedAccounts = () => api.post('/accounts/seed');

// === Invoices (additional) ===
export const getInvoice = (id: string) => api.get(`/invoices/${id}`);
export const createCreditNote = (id: string, data: any) => api.post(`/invoices/${id}/credit-note`, data);

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
export const exportReport = (data: any) => api.post('/reports/export', data);
