import axios from 'axios';

// ── Vendor-portal session ────────────────────────────────────────────────────
// Entirely separate from the staff session in `client.ts`: its own in-memory
// access token and its own httpOnly refresh cookie (`vendor_refresh`, scoped to
// `/api/v1/portal`). A vendor is never authenticated as staff and vice-versa.
let vendorToken: string | null = null;
let vendorIdentity: unknown = null;

export const getVendorToken = () => vendorToken;
export const getVendorIdentity = () => vendorIdentity;

export function storeVendorSession(data: { access_token: string; vendor?: unknown }) {
  vendorToken = data.access_token ?? null;
  if (data.vendor !== undefined) vendorIdentity = data.vendor;
}

export function clearVendorSession() {
  vendorToken = null;
  vendorIdentity = null;
}

const portal = axios.create({
  baseURL: '/api/v1/portal',
  headers: { 'Content-Type': 'application/json' },
  withCredentials: true,
});

portal.interceptors.request.use((config) => {
  if (vendorToken) config.headers['Authorization'] = `Bearer ${vendorToken}`;
  return config;
});

let refreshing: Promise<string | null> | null = null;

async function tryRefresh(): Promise<string | null> {
  try {
    const resp = await axios.post('/api/v1/portal/refresh', {}, { withCredentials: true });
    storeVendorSession(resp.data);
    return resp.data.access_token as string;
  } catch {
    clearVendorSession();
    return null;
  }
}

/** Restore a vendor session on load using the refresh cookie. */
export async function bootstrapVendorAuth(): Promise<boolean> {
  if (vendorToken) return true;
  const token = await tryRefresh();
  return token != null;
}

portal.interceptors.response.use(
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
        return portal(original);
      }
      clearVendorSession();
      if (window.location.pathname.startsWith('/vendorportal') && window.location.pathname !== '/vendorportal/login') {
        window.location.href = '/vendorportal/login';
      }
    }
    return Promise.reject(error);
  }
);

export default portal;

// ── Portal endpoints ─────────────────────────────────────────────────────────
export const portalRegister = (data: {
  company_name: string; display_name: string; email: string; password: string;
  kra_pin?: string; phone?: string;
}) => portal.post('/register', data);
export const portalLogin = (email: string, password: string) => portal.post('/login', { email, password });
export const portalLogout = () => portal.post('/logout', {});
export const getPortalMe = () => portal.get('/me');

export const getPortalTenders = () => portal.get('/tenders');
export const getPortalTender = (id: string) => portal.get(`/tenders/${id}`);
export const submitPortalBid = (id: string, data: {
  currency?: string; notes?: string;
  lines: { tender_line_id?: string; description: string; quantity?: number; unit_price: number }[];
}) => portal.post(`/tenders/${id}/bid`, data);
export const getPortalBids = () => portal.get('/bids');

export const getPortalPurchaseOrders = () => portal.get('/purchase-orders');
export const getPortalPurchaseOrder = (id: string) => portal.get(`/purchase-orders/${id}`);
/** The vendor's copy of the legal LPO document as a PDF blob. */
export const getPortalPurchaseOrderPdf = (id: string) =>
  portal.get(`/purchase-orders/${id}/document?format=pdf`, { responseType: 'blob' });
/**
 * Lodge an invoice against an LPO. Multipart because the eTIMS invoice number
 * and the eTIMS invoice file are both mandatory (enforced server-side too).
 */
export const lodgePortalInvoice = (
  id: string,
  data: { vendor_invoice_number: string; issue_date?: string; notes?: string; etims_file: File },
) => {
  const fd = new FormData();
  fd.append('vendor_invoice_number', data.vendor_invoice_number);
  if (data.issue_date) fd.append('issue_date', data.issue_date);
  if (data.notes) fd.append('notes', data.notes);
  fd.append('etims_file', data.etims_file);
  return portal.post(`/purchase-orders/${id}/invoice`, fd, {
    headers: { 'Content-Type': 'multipart/form-data' },
  });
};
export const getPortalInvoices = () => portal.get('/invoices');
export const getPortalStatement = () => portal.get('/statement');
