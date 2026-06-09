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
export const convertEstimate = (id: string, data?: any) => api.post(`/estimates/${id}/convert`, data);

// === Bills ===
export const getBills = () => api.get('/bills');
export const createBill = (data: any) => api.post('/bills', data);
export const approveBill = (id: string) => api.post(`/bills/${id}/approve`);

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
