// Employee self-service (ESS) API client — a SEPARATE principal from the
// back-office `era_users` session in client.ts. Staff log in via /staff/login,
// which issues an 'Employee'-role token; that token lives here in memory (never
// shared with the ERP session) and is refreshed via the staff refresh cookie
// (Path=/api/v1/staff). This mirrors the vendor portal's isolation.
import axios from 'axios';

let staffToken: string | null = null;
let staffIdentity: unknown = null;

export const getStaffToken = () => staffToken;
export const getStaffIdentity = () => staffIdentity as
  | { id?: string; email?: string; display_name?: string; employee_id?: string; status?: string }
  | null;

export function storeStaffSession(data: { access_token: string; staff?: unknown }) {
  staffToken = data.access_token ?? null;
  if (data.staff !== undefined) staffIdentity = data.staff;
}
export function clearStaffSession() {
  staffToken = null;
  staffIdentity = null;
}

const staffApi = axios.create({
  baseURL: '/api/v1',
  headers: { 'Content-Type': 'application/json' },
  withCredentials: true,
});

staffApi.interceptors.request.use((config) => {
  if (staffToken) config.headers['Authorization'] = `Bearer ${staffToken}`;
  return config;
});

async function tryStaffRefresh(): Promise<string | null> {
  try {
    const resp = await axios.post('/api/v1/staff/refresh', {}, { withCredentials: true });
    storeStaffSession(resp.data);
    return resp.data.access_token as string;
  } catch {
    clearStaffSession();
    return null;
  }
}

/** Restore a staff session on portal load using the staff refresh cookie. */
export async function bootstrapStaffAuth(): Promise<boolean> {
  if (staffToken) return true;
  return (await tryStaffRefresh()) != null;
}

staffApi.interceptors.response.use(
  (r) => r,
  async (error) => {
    const original = error.config;
    if (error.response?.status === 401 && original && !original._retried) {
      original._retried = true;
      const t = await tryStaffRefresh();
      if (t) {
        original.headers['Authorization'] = `Bearer ${t}`;
        return staffApi(original);
      }
      clearStaffSession();
      if (window.location.pathname !== '/staff/login') window.location.href = '/staff/login';
    }
    return Promise.reject(error);
  }
);

// ── Staff auth ──
export const staffLogin = (email: string, password: string) => staffApi.post('/staff/login', { email, password });
export const staffLogout = () => staffApi.post('/staff/logout', {});

// ── Staff self-service (own records only) ──
export const staffGetProfile = () => staffApi.get('/staff/profile');
export const staffUpdateProfile = (data: { phone?: string; personal_email?: string }) => staffApi.put('/staff/profile', data);
export const staffGetLeaveBalances = () => staffApi.get('/staff/leave-balances');
export const staffGetLeaveTypes = () => staffApi.get('/staff/leave-types');
export const staffGetHolidays = () => staffApi.get('/staff/holidays');
export const staffGetLeaveRequests = () => staffApi.get('/staff/leave-requests');
export const staffCreateLeaveRequest = (data: any) => staffApi.post('/staff/leave-requests', data);
export const staffCancelLeaveRequest = (id: string) => staffApi.post(`/staff/leave-requests/${id}/cancel`, {});
export const staffGetPayslips = () => staffApi.get('/staff/payslips');
export const staffGetPayslipPdf = (runId: string) => staffApi.get(`/staff/payslips/${runId}/pdf`, { responseType: 'blob' });

export default staffApi;
