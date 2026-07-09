import axios from 'axios';

/** Separate in-memory access token for the platform plane (not tenant ERP). */
let platformAccessToken: string | null = null;
let platformIdentity: unknown = null;

export const getPlatformAccessToken = () => platformAccessToken;
export const getPlatformIdentity = () => platformIdentity;

export function storePlatformSession(data: { access_token: string; user?: unknown }) {
  platformAccessToken = data.access_token ?? null;
  if (data.user !== undefined) platformIdentity = data.user;
}

export function clearPlatformSession() {
  platformAccessToken = null;
  platformIdentity = null;
}

const platformApi = axios.create({
  baseURL: '/api/v1/platform',
  headers: { 'Content-Type': 'application/json' },
  withCredentials: true,
});

platformApi.interceptors.request.use((config) => {
  if (platformAccessToken) {
    config.headers['Authorization'] = `Bearer ${platformAccessToken}`;
  }
  return config;
});

let refreshing: Promise<string | null> | null = null;

async function tryRefresh(): Promise<string | null> {
  try {
    const resp = await axios.post(
      '/api/v1/platform/auth/refresh',
      {},
      { withCredentials: true },
    );
    storePlatformSession(resp.data);
    return resp.data.access_token as string;
  } catch {
    clearPlatformSession();
    return null;
  }
}

export async function bootstrapPlatformAuth(): Promise<boolean> {
  if (platformAccessToken) return true;
  const token = await tryRefresh();
  return token != null;
}

platformApi.interceptors.response.use(
  (r) => r,
  async (error) => {
    const original = error.config;
    if (error.response?.status === 401 && original && !original._retried) {
      original._retried = true;
      refreshing = refreshing ?? tryRefresh();
      const newToken = await refreshing;
      refreshing = null;
      if (newToken) {
        original.headers['Authorization'] = `Bearer ${newToken}`;
        return platformApi(original);
      }
      clearPlatformSession();
      if (!window.location.pathname.startsWith('/platform/login')) {
        window.location.href = '/platform/login';
      }
    }
    return Promise.reject(error);
  },
);

export const platformLogin = (email: string, password: string) =>
  platformApi.post('/auth/login', { email, password });

export const platformLogout = () => platformApi.post('/auth/logout', {});

export const platformMe = () => platformApi.get('/me');

export const platformListTenants = (params?: {
  q?: string;
  plan_status?: string;
  hide_empty?: boolean;
  hide_archived?: boolean;
  limit?: number;
  offset?: number;
}) => platformApi.get('/tenants', { params });

/** Tenant detail: summary + users + recent audit. */
export const platformGetTenant = (entityId: string) =>
  platformApi.get(`/tenants/${entityId}`);

export const platformUpdateTenant = (
  entityId: string,
  data: { plan_key?: string | null; plan_status?: string },
) => platformApi.patch(`/tenants/${entityId}`, data);

export const platformSuspendTenant = (entityId: string, reason?: string) =>
  platformApi.post(`/tenants/${entityId}/suspend`, reason ? { reason } : {});

export const platformUnsuspendTenant = (entityId: string) =>
  platformApi.post(`/tenants/${entityId}/unsuspend`, {});

export const platformArchiveTenant = (entityId: string) =>
  platformApi.post(`/tenants/${entityId}/archive`, {});

export const platformUnarchiveTenant = (entityId: string) =>
  platformApi.post(`/tenants/${entityId}/unarchive`, {});

/** Open a short-lived support session. `reason` is required (min 5 chars). */
export const platformImpersonateTenant = (
  entityId: string,
  opts: { userId?: string; reason: string; readOnly?: boolean },
) =>
  platformApi.post(`/tenants/${entityId}/impersonate`, {
    user_id: opts.userId,
    reason: opts.reason,
    read_only: opts.readOnly ?? false,
  });

export const platformListAudit = (params?: {
  entity_id?: string;
  action?: string;
  limit?: number;
  offset?: number;
}) => platformApi.get('/audit', { params });

export const platformMetrics = () => platformApi.get('/metrics');

export const platformListOperators = () => platformApi.get('/operators');

export const platformCreateOperator = (data: {
  email: string;
  display_name: string;
  password: string;
  role?: string;
}) => platformApi.post('/operators', data);

export const platformSetOperatorActive = (id: string, is_active: boolean) =>
  platformApi.post(`/operators/${id}/set-active`, { is_active });

export default platformApi;
