// Customer-portal API client — a SEPARATE principal from the back-office
// `era_users` session (client.ts) and from staff/vendor portals. Customers log
// in via /customerportal/login (or self-register), which issues a
// 'Customer'-role token kept in memory here and refreshed via the customer
// refresh cookie (Path=/api/v1/customerportal). Mirrors staffClient's isolation.
import axios from 'axios';

let customerToken: string | null = null;
let customerIdentity: unknown = null;

export const getCustomerToken = () => customerToken;
export const getCustomerIdentity = () => customerIdentity as
  | { id?: string; email?: string; display_name?: string; customer_id?: string | null; status?: string }
  | null;

export function storeCustomerSession(data: { access_token: string; customer?: unknown }) {
  customerToken = data.access_token ?? null;
  if (data.customer !== undefined) customerIdentity = data.customer;
}
export function clearCustomerSession() {
  customerToken = null;
  customerIdentity = null;
}

const customerApi = axios.create({
  baseURL: '/api/v1',
  headers: { 'Content-Type': 'application/json' },
  withCredentials: true,
});

customerApi.interceptors.request.use((config) => {
  if (customerToken) config.headers['Authorization'] = `Bearer ${customerToken}`;
  return config;
});

async function tryCustomerRefresh(): Promise<string | null> {
  try {
    const resp = await axios.post('/api/v1/customerportal/refresh', {}, { withCredentials: true });
    storeCustomerSession(resp.data);
    return resp.data.access_token as string;
  } catch {
    clearCustomerSession();
    return null;
  }
}

/** Restore a customer session on portal load using the refresh cookie. */
export async function bootstrapCustomerAuth(): Promise<boolean> {
  if (customerToken) return true;
  return (await tryCustomerRefresh()) != null;
}

customerApi.interceptors.response.use(
  (r) => r,
  async (error) => {
    const original = error.config;
    if (error.response?.status === 401 && original && !original._retried) {
      original._retried = true;
      const t = await tryCustomerRefresh();
      if (t) {
        original.headers['Authorization'] = `Bearer ${t}`;
        return customerApi(original);
      }
      clearCustomerSession();
      if (window.location.pathname !== '/customerportal/login') window.location.href = '/customerportal/login';
    }
    return Promise.reject(error);
  }
);

// ── Customer auth ──
export const customerLogin = (email: string, password: string) => customerApi.post('/customerportal/login', { email, password });
export const customerRegister = (data: { display_name: string; email: string; company?: string; phone?: string; password: string }) =>
  customerApi.post('/customerportal/register', data);
export const customerLogout = () => customerApi.post('/customerportal/logout', {});
export const customerForgotPassword = (email: string) => customerApi.post('/customerportal/forgot-password', { email });
export const customerSetPassword = (token: string, password: string) => customerApi.post('/customerportal/set-password', { token, password });

// ── Customer self-service (own records only) ──
export const customerGetProfile = () => customerApi.get('/customerportal/profile');
export const customerUpdateProfile = (data: { display_name?: string }) => customerApi.put('/customerportal/profile', data);
export const customerGetInvoices = () => customerApi.get('/customerportal/invoices');
export const customerGetStatement = () => customerApi.get('/customerportal/statement');
export const customerGetTickets = () => customerApi.get('/customerportal/tickets');
export const customerCreateTicket = (data: { subject: string; description?: string; priority?: string }) =>
  customerApi.post('/customerportal/tickets', data);
export const customerGetTicket = (id: string) => customerApi.get(`/customerportal/tickets/${id}`);
export const customerReplyTicket = (id: string, body: string) => customerApi.post(`/customerportal/tickets/${id}/reply`, { body });

export default customerApi;
