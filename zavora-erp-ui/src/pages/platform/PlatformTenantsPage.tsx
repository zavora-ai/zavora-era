import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  bootstrapPlatformAuth,
  clearPlatformSession,
  getPlatformIdentity,
  getPlatformAccessToken,
  platformImpersonateTenant,
  platformListTenants,
  platformLogout,
  platformMe,
  platformSuspendTenant,
  platformUnsuspendTenant,
} from '../../api/platformClient';
import { storeSession } from '../../api/client';
import { useNavigate } from 'react-router-dom';
import { formatDate } from '../../utils/format';
import { Building2, LogOut, Search, ShieldAlert, ShieldCheck, UserRoundSearch } from 'lucide-react';

interface Tenant {
  entity_id: string;
  organization_name: string;
  organization_type?: string;
  plan_key?: string;
  plan_status: string;
  suspended: boolean;
  suspended_reason?: string;
  archived: boolean;
  created_at: string;
  last_activity_at?: string;
  user_count: number;
  invoice_count: number;
}

export default function PlatformTenantsPage() {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [ready, setReady] = useState(false);
  const [q, setQ] = useState('');
  const [status, setStatus] = useState('');
  const [actionError, setActionError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  useEffect(() => {
    bootstrapPlatformAuth().then((ok) => {
      if (!ok) {
        navigate('/platform/login', { replace: true });
        return;
      }
      setReady(true);
    });
  }, [navigate]);

  const { data: me } = useQuery({
    queryKey: ['platform-me'],
    queryFn: () => platformMe().then((r) => r.data),
    enabled: ready && !!getPlatformAccessToken(),
  });

  const { data, isLoading, isError, refetch } = useQuery({
    queryKey: ['platform-tenants', q, status],
    queryFn: () =>
      platformListTenants({
        q: q || undefined,
        plan_status: status || undefined,
        limit: 100,
      }).then((r) => r.data),
    enabled: ready,
  });

  const tenants: Tenant[] = data?.data ?? [];
  const total: number = data?.total_count ?? 0;
  const identity = (me ?? getPlatformIdentity()) as { email?: string; display_name?: string } | null;

  const invalidate = () => qc.invalidateQueries({ queryKey: ['platform-tenants'] });

  const suspendMut = useMutation({
    mutationFn: ({ entityId, reason }: { entityId: string; reason?: string }) =>
      platformSuspendTenant(entityId, reason),
    onSuccess: () => {
      setActionError(null);
      invalidate();
    },
    onError: (e: unknown) => {
      setActionError(extractErr(e, 'Suspend failed'));
    },
    onSettled: () => setBusyId(null),
  });

  const unsuspendMut = useMutation({
    mutationFn: (entityId: string) => platformUnsuspendTenant(entityId),
    onSuccess: () => {
      setActionError(null);
      invalidate();
    },
    onError: (e: unknown) => {
      setActionError(extractErr(e, 'Unsuspend failed'));
    },
    onSettled: () => setBusyId(null),
  });

  const impersonateMut = useMutation({
    mutationFn: (entityId: string) => platformImpersonateTenant(entityId),
    onSuccess: (resp) => {
      setActionError(null);
      // Install tenant session (access token + refresh cookie already set by API).
      storeSession(resp.data);
      try {
        sessionStorage.setItem(
          'era_support_session',
          JSON.stringify({
            organization_name: resp.data?.tenant?.organization_name,
            entity_id: resp.data?.tenant?.entity_id,
            target_email: resp.data?.user?.email,
            suspended: resp.data?.tenant?.suspended,
          }),
        );
      } catch {
        /* ignore */
      }
      // Full navigation so RequireAuth + bootstrap pick up the new session.
      window.location.href = '/';
    },
    onError: (e: unknown) => {
      setActionError(extractErr(e, 'Impersonate failed'));
      setBusyId(null);
    },
  });

  const logout = async () => {
    try {
      await platformLogout();
    } catch {
      /* ignore */
    }
    clearPlatformSession();
    navigate('/platform/login', { replace: true });
  };

  const onSuspend = (t: Tenant) => {
    const reason =
      window.prompt(
        `Suspend “${t.organization_name}”?\n\nOptional reason (shown in audit trail):`,
        t.suspended_reason || '',
      ) ?? null;
    if (reason === null) return; // cancelled
    setBusyId(t.entity_id);
    suspendMut.mutate({ entityId: t.entity_id, reason: reason.trim() || undefined });
  };

  const onUnsuspend = (t: Tenant) => {
    if (!window.confirm(`Restore access for “${t.organization_name}”?`)) return;
    setBusyId(t.entity_id);
    unsuspendMut.mutate(t.entity_id);
  };

  const onImpersonate = (t: Tenant) => {
    const note = t.suspended
      ? `\n\nThis tenant is suspended — you will open a short-lived support session anyway.`
      : '';
    if (
      !window.confirm(
        `Open a support session in “${t.organization_name}” as the primary Owner?\n\nThis is audited and expires in ~30 minutes.${note}`,
      )
    ) {
      return;
    }
    setBusyId(t.entity_id);
    impersonateMut.mutate(t.entity_id);
  };

  if (!ready) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-slate-950 text-slate-400 text-sm">
        Loading platform…
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100">
      <header className="border-b border-slate-800 bg-slate-900/80 backdrop-blur sticky top-0 z-10">
        <div className="mx-auto max-w-7xl flex items-center justify-between px-6 py-4">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-indigo-600">
              <Building2 className="h-5 w-5 text-white" />
            </div>
            <div>
              <p className="text-xs font-semibold uppercase tracking-widest text-indigo-400">Zavora Platform</p>
              <h1 className="text-lg font-semibold text-white">Tenants</h1>
            </div>
          </div>
          <div className="flex items-center gap-4 text-sm">
            <span className="text-slate-400">{identity?.display_name || identity?.email || 'Operator'}</span>
            <button
              type="button"
              onClick={logout}
              className="inline-flex items-center gap-1.5 rounded-lg border border-slate-700 px-3 py-1.5 text-slate-300 hover:bg-slate-800"
            >
              <LogOut className="h-4 w-4" /> Sign out
            </button>
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-7xl px-6 py-8">
        <div className="mb-6 flex flex-wrap items-end gap-3">
          <div className="relative flex-1 min-w-[200px]">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
            <input
              className="w-full rounded-lg border border-slate-700 bg-slate-900 py-2 pl-9 pr-3 text-sm text-white outline-none focus:border-indigo-500"
              placeholder="Search name or entity id…"
              value={q}
              onChange={(e) => setQ(e.target.value)}
            />
          </div>
          <select
            className="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-sm text-white"
            value={status}
            onChange={(e) => setStatus(e.target.value)}
          >
            <option value="">All statuses</option>
            <option value="active">active</option>
            <option value="trial">trial</option>
            <option value="past_due">past_due</option>
            <option value="suspended">suspended</option>
          </select>
          <button
            type="button"
            onClick={() => refetch()}
            className="rounded-lg bg-slate-800 px-3 py-2 text-sm text-slate-200 hover:bg-slate-700"
          >
            Refresh
          </button>
        </div>

        {actionError && (
          <div className="mb-4 rounded-lg border border-red-900 bg-red-950/50 px-4 py-3 text-sm text-red-300">
            {actionError}
          </div>
        )}

        <p className="mb-3 text-sm text-slate-400">
          {total} tenant{total === 1 ? '' : 's'}
          {q || status ? ' (filtered)' : ''}
        </p>

        <div className="overflow-hidden rounded-xl border border-slate-800 bg-slate-900">
          {isLoading && <p className="p-8 text-center text-sm text-slate-500">Loading tenants…</p>}
          {isError && (
            <p className="p-8 text-center text-sm text-red-400">
              Could not load tenants. Check you are signed in as a platform operator.
            </p>
          )}
          {!isLoading && !isError && (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-800 text-left text-xs uppercase tracking-wide text-slate-500">
                  <th className="px-4 py-3">Organization</th>
                  <th className="px-4 py-3">Plan</th>
                  <th className="px-4 py-3">Status</th>
                  <th className="px-4 py-3 text-right">Users</th>
                  <th className="px-4 py-3 text-right">Invoices</th>
                  <th className="px-4 py-3">Created</th>
                  <th className="px-4 py-3">Last activity</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {tenants.map((t) => {
                  const busy = busyId === t.entity_id;
                  return (
                    <tr key={t.entity_id} className="border-b border-slate-800/80 hover:bg-slate-800/40">
                      <td className="px-4 py-3">
                        <div className="font-medium text-white">{t.organization_name}</div>
                        <div className="font-mono text-[11px] text-slate-500">{t.entity_id}</div>
                        {t.suspended_reason && (
                          <div className="mt-0.5 text-[11px] text-amber-500/90 truncate max-w-[220px]" title={t.suspended_reason}>
                            {t.suspended_reason}
                          </div>
                        )}
                      </td>
                      <td className="px-4 py-3 text-slate-300">{t.plan_key || '—'}</td>
                      <td className="px-4 py-3">
                        <StatusBadge status={t.plan_status} suspended={t.suspended} archived={t.archived} />
                      </td>
                      <td className="px-4 py-3 text-right tabular-nums text-slate-300">{t.user_count}</td>
                      <td className="px-4 py-3 text-right tabular-nums text-slate-300">{t.invoice_count}</td>
                      <td className="px-4 py-3 text-slate-400">{formatDate(t.created_at)}</td>
                      <td className="px-4 py-3 text-slate-400">
                        {t.last_activity_at ? formatDate(t.last_activity_at) : '—'}
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            type="button"
                            disabled={busy || t.user_count === 0}
                            title={t.user_count === 0 ? 'No users to impersonate' : 'Open support session'}
                            onClick={() => onImpersonate(t)}
                            className="inline-flex items-center gap-1 rounded-md border border-slate-700 px-2 py-1 text-xs text-slate-200 hover:bg-slate-800 disabled:opacity-40"
                          >
                            <UserRoundSearch className="h-3.5 w-3.5" />
                            Open
                          </button>
                          {t.suspended ? (
                            <button
                              type="button"
                              disabled={busy}
                              onClick={() => onUnsuspend(t)}
                              className="inline-flex items-center gap-1 rounded-md border border-emerald-900 bg-emerald-950/40 px-2 py-1 text-xs text-emerald-300 hover:bg-emerald-950 disabled:opacity-40"
                            >
                              <ShieldCheck className="h-3.5 w-3.5" />
                              Restore
                            </button>
                          ) : (
                            <button
                              type="button"
                              disabled={busy || t.archived}
                              onClick={() => onSuspend(t)}
                              className="inline-flex items-center gap-1 rounded-md border border-red-900/80 bg-red-950/30 px-2 py-1 text-xs text-red-300 hover:bg-red-950/60 disabled:opacity-40"
                            >
                              <ShieldAlert className="h-3.5 w-3.5" />
                              Suspend
                            </button>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
                {tenants.length === 0 && (
                  <tr>
                    <td colSpan={8} className="px-4 py-10 text-center text-slate-500">
                      No tenants found.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          )}
        </div>

        <p className="mt-6 text-xs text-slate-600">
          Phase 1: suspend / restore kicks active sessions; Open starts a short-lived, audited support session as the
          primary Owner.
        </p>
      </main>
    </div>
  );
}

function extractErr(e: unknown, fallback: string): string {
  const ax = e as { response?: { data?: { error?: string; message?: string } } };
  return ax?.response?.data?.error || ax?.response?.data?.message || fallback;
}

function StatusBadge({
  status,
  suspended,
  archived,
}: {
  status: string;
  suspended: boolean;
  archived: boolean;
}) {
  const label = archived ? 'archived' : suspended ? 'suspended' : status;
  const cls =
    label === 'active'
      ? 'bg-emerald-950 text-emerald-300 border-emerald-800'
      : label === 'trial'
        ? 'bg-sky-950 text-sky-300 border-sky-800'
        : label === 'past_due'
          ? 'bg-amber-950 text-amber-300 border-amber-800'
          : 'bg-red-950 text-red-300 border-red-800';
  return (
    <span className={`inline-flex rounded-full border px-2 py-0.5 text-xs font-medium ${cls}`}>
      {label}
    </span>
  );
}
