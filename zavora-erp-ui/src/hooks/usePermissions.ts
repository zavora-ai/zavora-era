import { useQuery } from '@tanstack/react-query';
import { getMyPermissions, getAccessToken } from '../api/client';

interface MyPermissions {
  role: string;
  permissions: string[];
}

/**
 * The current user's effective permissions, fetched from the server
 * (`GET /auth/permissions`). This is the single source of truth for UI gating —
 * `can('journal.post')` — so the frontend can never drift from the backend's
 * role→permission model. The backend still enforces on every request; this only
 * decides what to show.
 */
export function usePermissions() {
  const { data, isLoading } = useQuery<MyPermissions>({
    queryKey: ['auth-permissions'],
    queryFn: () => getMyPermissions().then((r) => r.data),
    enabled: !!getAccessToken(),
    staleTime: 5 * 60 * 1000,
  });
  const permissions = data?.permissions ?? [];
  const can = (key: string) => permissions.includes(key);
  return { role: data?.role, permissions, can, loaded: !!data, isLoading };
}
