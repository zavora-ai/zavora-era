import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  bootstrapPlatformAuth,
  clearPlatformSession,
  getPlatformIdentity,
  getPlatformAccessToken,
  platformArchiveTenant,
  platformCreateOperator,
  platformGetTenant,
  platformImpersonateTenant,
  platformListAudit,
  platformListOperators,
  platformListTenants,
  platformLogout,
  platformMe,
  platformMetrics,
  platformSetOperatorActive,
  platformSuspendTenant,
  platformUnarchiveTenant,
  platformUnsuspendTenant,
  platformUpdateTenant,
} from '../../api/platformClient';
import { storeSession } from '../../api/client';
import { useNavigate } from 'react-router-dom';
import { formatDate } from '../../utils/format';
import {
  Archive,
  ArchiveRestore,
  Building2,
  ChevronLeft,
  ChevronRight,
  LogOut,
  Search,
  ShieldAlert,
  ShieldCheck,
  UserRoundSearch,
  X,
} from 'lucide-react';

interface Tenant {
  entity_id: string;
  organization_name: string;
  organization_type?: string;
  plan_key?: string | null;
  plan_status: string;
  suspended: boolean;
  suspended_at?: string;
  suspended_reason?: string;
  archived: boolean;
  created_at: string;
  last_activity_at?: string;
  user_count: number;
  invoice_count: number;
  /** Primary Owner (or first active user) email */
  primary_email?: string;
  primary_contact?: string;
}

interface TenantUser {
  id: string;
  email: string;
  display_name: string;
  role: string;
  is_active: boolean;
  last_login?: string;
  created_at?: string;
}

interface AuditEvent {
  id: string;
  actor_email?: string;
  action: string;
  target_entity_id?: string;
  organization_name?: string;
  metadata?: Record<string, unknown>;
  created_at: string;
}

interface TenantDetail extends Tenant {
  users: TenantUser[];
  recent_audit: AuditEvent[];
}

const PAGE_SIZE = 50;

export default function PlatformTenantsPage() {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [ready, setReady] = useState(false);
  const [q, setQ] = useState('');
  const [status, setStatus] = useState('');
  const [hideEmpty, setHideEmpty] = useState(true);
  const [hideArchived, setHideArchived] = useState(true);
  const [page, setPage] = useState(0);
  const [tab, setTab] = useState<'tenants' | 'audit' | 'metrics' | 'operators'>('tenants');
  const [actionError, setActionError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  useEffect(() => {
    bootstrapPlatformAuth().then((ok) => {
      if (!ok) {
        navigate('/platform/login', { replace: true });
        return;
      }
      setReady(true);
    });
  }, [navigate]);

  // Reset page when filters change.
  useEffect(() => {
    setPage(0);
  }, [q, status, hideEmpty, hideArchived]);

  const { data: me } = useQuery({
    queryKey: ['platform-me'],
    queryFn: () => platformMe().then((r) => r.data),
    enabled: ready && !!getPlatformAccessToken(),
  });

  const { data, isLoading, isError, refetch } = useQuery({
    queryKey: ['platform-tenants', q, status, hideEmpty, hideArchived, page],
    queryFn: () =>
      platformListTenants({
        q: q || undefined,
        plan_status: status || undefined,
        hide_empty: hideEmpty,
        hide_archived: hideArchived,
        limit: PAGE_SIZE,
        offset: page * PAGE_SIZE,
      }).then((r) => r.data),
    enabled: ready && tab === 'tenants',
  });

  const { data: auditData, isLoading: auditLoading } = useQuery({
    queryKey: ['platform-audit'],
    queryFn: () => platformListAudit({ limit: 100 }).then((r) => r.data),
    enabled: ready && tab === 'audit',
  });

  const { data: metricsData, isLoading: metricsLoading } = useQuery({
    queryKey: ['platform-metrics'],
    queryFn: () => platformMetrics().then((r) => r.data?.data),
    enabled: ready && tab === 'metrics',
  });

  const { data: operatorsData, isLoading: operatorsLoading, refetch: refetchOps } = useQuery({
    queryKey: ['platform-operators'],
    queryFn: () => platformListOperators().then((r) => r.data?.data ?? []),
    enabled: ready && tab === 'operators',
  });

  const { data: detailRes, isLoading: detailLoading } = useQuery({
    queryKey: ['platform-tenant', selectedId],
    queryFn: () => platformGetTenant(selectedId!).then((r) => r.data?.data as TenantDetail),
    enabled: ready && !!selectedId,
  });

  const tenants: Tenant[] = data?.data ?? [];
  const total: number = data?.total_count ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const auditEvents: AuditEvent[] = auditData?.data ?? [];
  const identity = (me ?? getPlatformIdentity()) as { email?: string; display_name?: string; role?: string } | null;
  const isSuperAdmin = ((identity?.role || 'PlatformSuperAdmin')).toLowerCase() === 'platformsuperadmin';

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ['platform-tenants'] });
    qc.invalidateQueries({ queryKey: ['platform-tenant'] });
    qc.invalidateQueries({ queryKey: ['platform-audit'] });
  };

  const runAction = async (entityId: string, fn: () => Promise<unknown>) => {
    setBusyId(entityId);
    setActionError(null);
    try {
      await fn();
      invalidate();
    } catch (e) {
      setActionError(extractErr(e, 'Action failed'));
    } finally {
      setBusyId(null);
    }
  };

  const onSuspend = (t: Tenant) => {
    const reason =
      window.prompt(
        `Suspend “${t.organization_name}”?\n\nOptional reason (shown in audit trail):`,
        t.suspended_reason || '',
      ) ?? null;
    if (reason === null) return;
    void runAction(t.entity_id, () =>
      platformSuspendTenant(t.entity_id, reason.trim() || undefined),
    );
  };

  const onUnsuspend = (t: Tenant) => {
    if (!window.confirm(`Restore access for “${t.organization_name}”?`)) return;
    void runAction(t.entity_id, () => platformUnsuspendTenant(t.entity_id));
  };

  const onArchive = (t: Tenant) => {
    if (!window.confirm(`Archive “${t.organization_name}”? Sessions will be revoked.`)) return;
    void runAction(t.entity_id, () => platformArchiveTenant(t.entity_id));
  };

  const onUnarchive = (t: Tenant) => {
    if (!window.confirm(`Unarchive “${t.organization_name}”?`)) return;
    void runAction(t.entity_id, () => platformUnarchiveTenant(t.entity_id));
  };

  const onImpersonate = (t: Tenant, userId?: string, email?: string) => {
    const who = email ? ` as ${email}` : ' as the primary Owner';
    const reason =
      window.prompt(
        `Open support session in “${t.organization_name}”${who}?\n\nRequired reason (ticket / customer request, min 5 chars):`,
        '',
      ) ?? null;
    if (reason === null) return;
    if (reason.trim().length < 5) {
      setActionError('Support session reason must be at least 5 characters.');
      return;
    }
    const readOnly = window.confirm(
      'Open as READ-ONLY (Viewer)?\n\nOK = read-only · Cancel = full Owner/target permissions',
    );
    // confirm returns true for OK (read-only). For Cancel we want full access.
    // Actually user asked: OK = read-only is fine.

    setBusyId(t.entity_id);
    setActionError(null);
    platformImpersonateTenant(t.entity_id, {
      userId,
      reason: reason.trim(),
      readOnly,
    })
      .then((resp) => {
        storeSession(resp.data);
        try {
          sessionStorage.setItem(
            'era_support_session',
            JSON.stringify({
              organization_name: resp.data?.tenant?.organization_name,
              entity_id: resp.data?.tenant?.entity_id,
              target_email: resp.data?.user?.email,
              suspended: resp.data?.tenant?.suspended,
              read_only: resp.data?.read_only ?? readOnly,
              reason: reason.trim(),
            }),
          );
        } catch {
          /* ignore */
        }
        window.location.href = '/';
      })
      .catch((e) => {
        setActionError(extractErr(e, 'Impersonate failed'));
        setBusyId(null);
      });
  };

  const planMut = useMutation({
    mutationFn: ({
      entityId,
      plan_key,
      plan_status,
    }: {
      entityId: string;
      plan_key?: string | null;
      plan_status?: string;
    }) => platformUpdateTenant(entityId, { plan_key, plan_status }),
    onSuccess: () => {
      setActionError(null);
      invalidate();
    },
    onError: (e: unknown) => setActionError(extractErr(e, 'Plan update failed')),
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

  if (!ready) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-slate-950 text-slate-400 text-sm">
        Loading platform…
      </div>
    );
  }

  const detail = detailRes;

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
              <h1 className="text-lg font-semibold text-white">Ops console</h1>
            </div>
            <nav className="ml-6 flex gap-1 rounded-lg border border-slate-800 bg-slate-950/60 p-0.5 text-sm">
              {(
                [
                  ['tenants', 'Tenants'],
                  ['metrics', 'Metrics'],
                  ['audit', 'Audit'],
                  ...(isSuperAdmin ? ([['operators', 'Operators']] as const) : []),
                ] as [string, string][]
              ).map(([id, label]) => (
                <button
                  key={id}
                  type="button"
                  onClick={() => setTab(id as 'tenants' | 'audit' | 'metrics' | 'operators')}
                  className={`rounded-md px-3 py-1.5 ${tab === id ? 'bg-slate-800 text-white' : 'text-slate-400 hover:text-slate-200'}`}
                >
                  {label}
                </button>
              ))}
            </nav>
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
        {actionError && (
          <div className="mb-4 rounded-lg border border-red-900 bg-red-950/50 px-4 py-3 text-sm text-red-300">
            {actionError}
          </div>
        )}

        {tab === 'tenants' && (
          <>
            <div className="mb-6 flex flex-wrap items-end gap-3">
              <div className="relative flex-1 min-w-[200px]">
                <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
                <input
                  className="w-full rounded-lg border border-slate-700 bg-slate-900 py-2 pl-9 pr-3 text-sm text-white outline-none focus:border-indigo-500"
                  placeholder="Search name, email, or entity id…"
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
              <label className="flex items-center gap-2 text-xs text-slate-400">
                <input
                  type="checkbox"
                  checked={hideEmpty}
                  onChange={(e) => setHideEmpty(e.target.checked)}
                  className="rounded border-slate-600 bg-slate-900"
                />
                Hide empty
              </label>
              <label className="flex items-center gap-2 text-xs text-slate-400">
                <input
                  type="checkbox"
                  checked={hideArchived}
                  onChange={(e) => setHideArchived(e.target.checked)}
                  className="rounded border-slate-600 bg-slate-900"
                />
                Hide archived
              </label>
              <button
                type="button"
                onClick={() => refetch()}
                className="rounded-lg bg-slate-800 px-3 py-2 text-sm text-slate-200 hover:bg-slate-700"
              >
                Refresh
              </button>
            </div>

            <p className="mb-3 text-sm text-slate-400">
              {total} tenant{total === 1 ? '' : 's'}
              {q || status || hideEmpty || hideArchived ? ' (filtered)' : ''}
              {total > PAGE_SIZE ? ` · page ${page + 1} of ${totalPages}` : ''}
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
                      <th className="px-4 py-3">Tenant</th>
                      <th className="px-4 py-3">Contact</th>
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
                        <tr
                          key={t.entity_id}
                          className={`border-b border-slate-800/80 hover:bg-slate-800/40 cursor-pointer ${
                            selectedId === t.entity_id ? 'bg-slate-800/60' : ''
                          }`}
                          onClick={() => setSelectedId(t.entity_id)}
                        >
                          <td className="px-4 py-3">
                            <div className="font-medium text-white">{t.organization_name}</div>
                            <div className="font-mono text-[11px] text-slate-500">{t.entity_id}</div>
                            {t.suspended_reason && (
                              <div
                                className="mt-0.5 text-[11px] text-amber-500/90 truncate max-w-[220px]"
                                title={t.suspended_reason}
                              >
                                {t.suspended_reason}
                              </div>
                            )}
                          </td>
                          <td className="px-4 py-3">
                            {t.primary_email || t.primary_contact ? (
                              <>
                                <div className="text-slate-200 truncate max-w-[200px]" title={t.primary_email}>
                                  {t.primary_email || '—'}
                                </div>
                                {t.primary_contact && (
                                  <div className="text-[11px] text-slate-500 truncate max-w-[200px]">
                                    {t.primary_contact}
                                  </div>
                                )}
                              </>
                            ) : (
                              <span className="text-slate-600">—</span>
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
                          <td className="px-4 py-3" onClick={(e) => e.stopPropagation()}>
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
                        <td colSpan={9} className="px-4 py-10 text-center text-slate-500">
                          No tenants found.
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              )}
            </div>

            {total > PAGE_SIZE && (
              <div className="mt-4 flex items-center justify-between text-sm text-slate-400">
                <button
                  type="button"
                  disabled={page === 0}
                  onClick={() => setPage((p) => Math.max(0, p - 1))}
                  className="inline-flex items-center gap-1 rounded-lg border border-slate-700 px-3 py-1.5 disabled:opacity-40 hover:bg-slate-900"
                >
                  <ChevronLeft className="h-4 w-4" /> Prev
                </button>
                <span>
                  Page {page + 1} / {totalPages}
                </span>
                <button
                  type="button"
                  disabled={page + 1 >= totalPages}
                  onClick={() => setPage((p) => p + 1)}
                  className="inline-flex items-center gap-1 rounded-lg border border-slate-700 px-3 py-1.5 disabled:opacity-40 hover:bg-slate-900"
                >
                  Next <ChevronRight className="h-4 w-4" />
                </button>
              </div>
            )}
          </>
        )}


        {tab === 'metrics' && (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {metricsLoading && <p className="text-sm text-slate-500 col-span-full">Loading metrics…</p>}
            {metricsData && (
              <>
                {[
                  ['Tenants', metricsData.tenants_total],
                  ['Active', metricsData.tenants_active],
                  ['Suspended', metricsData.tenants_suspended],
                  ['Archived', metricsData.tenants_archived],
                  ['Trial', metricsData.tenants_trial],
                  ['Past due', metricsData.tenants_past_due],
                  ['With users', metricsData.tenants_with_users],
                  ['Active users', metricsData.users_total],
                  ['Operators', metricsData.operators_active + '/' + metricsData.operators_total],
                  ['Signups (7d)', metricsData.signups_7d],
                  ['Impersonations (7d)', metricsData.impersonations_7d],
                  ['Suspensions (7d)', metricsData.suspensions_7d],
                ].map(([label, val]) => (
                  <div key={String(label)} className="rounded-xl border border-slate-800 bg-slate-900 p-4">
                    <p className="text-xs uppercase tracking-wide text-slate-500">{label}</p>
                    <p className="mt-1 text-2xl font-semibold tabular-nums text-white">{val}</p>
                  </div>
                ))}
              </>
            )}
          </div>
        )}

        {tab === 'operators' && isSuperAdmin && (
          <OperatorsPanel
            loading={operatorsLoading}
            operators={operatorsData ?? []}
            onRefresh={() => refetchOps()}
            setActionError={setActionError}
          />
        )}

        {tab === 'audit' && (
          <div className="overflow-hidden rounded-xl border border-slate-800 bg-slate-900">
            {auditLoading && <p className="p-8 text-center text-sm text-slate-500">Loading audit…</p>}
            {!auditLoading && (
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-slate-800 text-left text-xs uppercase tracking-wide text-slate-500">
                    <th className="px-4 py-3">When</th>
                    <th className="px-4 py-3">Actor</th>
                    <th className="px-4 py-3">Action</th>
                    <th className="px-4 py-3">Tenant</th>
                    <th className="px-4 py-3">Detail</th>
                  </tr>
                </thead>
                <tbody>
                  {auditEvents.map((e) => (
                    <tr key={e.id} className="border-b border-slate-800/80">
                      <td className="px-4 py-3 text-slate-400 whitespace-nowrap">
                        {formatDate(e.created_at)}
                      </td>
                      <td className="px-4 py-3 text-slate-300">{e.actor_email || '—'}</td>
                      <td className="px-4 py-3">
                        <code className="rounded bg-slate-800 px-1.5 py-0.5 text-xs text-indigo-300">
                          {e.action}
                        </code>
                      </td>
                      <td className="px-4 py-3 text-slate-300">
                        {e.organization_name || e.target_entity_id || '—'}
                      </td>
                      <td className="px-4 py-3 text-xs text-slate-500 font-mono truncate max-w-xs">
                        {e.metadata ? JSON.stringify(e.metadata) : '—'}
                      </td>
                    </tr>
                  ))}
                  {auditEvents.length === 0 && (
                    <tr>
                      <td colSpan={5} className="px-4 py-10 text-center text-slate-500">
                        No audit events yet.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            )}
          </div>
        )}

        <p className="mt-6 text-xs text-slate-600">
          Phase 3: metrics, operators, required Open reason, read-only support sessions, mid-request suspend gate. See docs/PLATFORM_ADMIN.md.
        </p>
      </main>

      {/* Tenant detail drawer */}
      {selectedId && (
        <div className="fixed inset-0 z-40 flex justify-end">
          <button
            type="button"
            className="absolute inset-0 bg-black/50"
            aria-label="Close drawer"
            onClick={() => setSelectedId(null)}
          />
          <aside className="relative z-10 flex h-full w-full max-w-md flex-col border-l border-slate-800 bg-slate-950 shadow-2xl">
            <div className="flex items-start justify-between border-b border-slate-800 px-5 py-4">
              <div>
                <p className="text-xs uppercase tracking-wide text-slate-500">Tenant</p>
                <h2 className="text-lg font-semibold text-white">
                  {detail?.organization_name || 'Loading…'}
                </h2>
                <p className="mt-1 font-mono text-[11px] text-slate-500 break-all">{selectedId}</p>
              </div>
              <button
                type="button"
                onClick={() => setSelectedId(null)}
                className="rounded-lg p-1.5 text-slate-400 hover:bg-slate-900 hover:text-white"
              >
                <X className="h-5 w-5" />
              </button>
            </div>

            <div className="flex-1 overflow-y-auto px-5 py-4 space-y-6">
              {detailLoading && <p className="text-sm text-slate-500">Loading detail…</p>}
              {detail && (
                <>
                  <section>
                    <div className="flex flex-wrap items-center gap-2 mb-3">
                      <StatusBadge
                        status={detail.plan_status}
                        suspended={detail.suspended}
                        archived={detail.archived}
                      />
                      <span className="text-xs text-slate-500">
                        {detail.user_count} users · {detail.invoice_count} invoices
                      </span>
                    </div>
                    {detail.suspended_reason && (
                      <p className="text-sm text-amber-400/90 mb-2">Reason: {detail.suspended_reason}</p>
                    )}
                    <dl className="grid grid-cols-2 gap-2 text-xs">
                      <div className="col-span-2">
                        <dt className="text-slate-500">Primary contact</dt>
                        <dd className="text-slate-200">
                          {detail.primary_email || '—'}
                          {detail.primary_contact ? (
                            <span className="text-slate-500"> · {detail.primary_contact}</span>
                          ) : null}
                        </dd>
                      </div>
                      <div>
                        <dt className="text-slate-500">Type</dt>
                        <dd className="text-slate-200">{detail.organization_type || '—'}</dd>
                      </div>
                      <div>
                        <dt className="text-slate-500">Created</dt>
                        <dd className="text-slate-200">{formatDate(detail.created_at)}</dd>
                      </div>
                      <div>
                        <dt className="text-slate-500">Last activity</dt>
                        <dd className="text-slate-200">
                          {detail.last_activity_at ? formatDate(detail.last_activity_at) : '—'}
                        </dd>
                      </div>
                      <div>
                        <dt className="text-slate-500">Plan key</dt>
                        <dd className="text-slate-200">{detail.plan_key || '—'}</dd>
                      </div>
                    </dl>
                  </section>

                  {isSuperAdmin && (
                  <section>
                    <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500 mb-2">
                      Plan
                    </h3>
                    <div className="flex flex-wrap gap-2">
                      <select
                        className="rounded-lg border border-slate-700 bg-slate-900 px-2 py-1.5 text-sm text-white"
                        value={detail.plan_key || ''}
                        disabled={detail.suspended || planMut.isPending}
                        onChange={(e) => {
                          const v = e.target.value;
                          planMut.mutate({
                            entityId: detail.entity_id,
                            plan_key: v === '' ? null : v,
                          });
                        }}
                      >
                        <option value="">No plan</option>
                        <option value="starter">starter</option>
                        <option value="business">business</option>
                        <option value="scale">scale</option>
                      </select>
                      <select
                        className="rounded-lg border border-slate-700 bg-slate-900 px-2 py-1.5 text-sm text-white"
                        value={detail.suspended ? 'suspended' : detail.plan_status}
                        disabled={detail.suspended || planMut.isPending}
                        onChange={(e) => {
                          const v = e.target.value;
                          if (v === 'suspended') return;
                          planMut.mutate({ entityId: detail.entity_id, plan_status: v });
                        }}
                      >
                        <option value="active">active</option>
                        <option value="trial">trial</option>
                        <option value="past_due">past_due</option>
                        {detail.suspended && <option value="suspended">suspended</option>}
                      </select>
                    </div>
                    {detail.suspended && (
                      <p className="mt-1 text-[11px] text-slate-500">Restore before changing plan status.</p>
                    )}
                  </section>
                  )}

                  <section>
                    <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500 mb-2">
                      Actions
                    </h3>
                    <div className="flex flex-wrap gap-2">
                      <button
                        type="button"
                        disabled={busyId === detail.entity_id || detail.user_count === 0}
                        onClick={() => onImpersonate(detail)}
                        className="inline-flex items-center gap-1 rounded-md border border-slate-700 px-2.5 py-1.5 text-xs text-slate-200 hover:bg-slate-900 disabled:opacity-40"
                      >
                        <UserRoundSearch className="h-3.5 w-3.5" /> Open as Owner
                      </button>
                      {isSuperAdmin &&
                        (detail.suspended ? (
                          <button
                            type="button"
                            onClick={() => onUnsuspend(detail)}
                            className="inline-flex items-center gap-1 rounded-md border border-emerald-900 bg-emerald-950/40 px-2.5 py-1.5 text-xs text-emerald-300"
                          >
                            <ShieldCheck className="h-3.5 w-3.5" /> Restore
                          </button>
                        ) : (
                          <button
                            type="button"
                            onClick={() => onSuspend(detail)}
                            className="inline-flex items-center gap-1 rounded-md border border-red-900/80 bg-red-950/30 px-2.5 py-1.5 text-xs text-red-300"
                          >
                            <ShieldAlert className="h-3.5 w-3.5" /> Suspend
                          </button>
                        ))}
                      {isSuperAdmin &&
                        (detail.archived ? (
                          <button
                            type="button"
                            onClick={() => onUnarchive(detail)}
                            className="inline-flex items-center gap-1 rounded-md border border-slate-700 px-2.5 py-1.5 text-xs text-slate-200"
                          >
                            <ArchiveRestore className="h-3.5 w-3.5" /> Unarchive
                          </button>
                        ) : (
                          <button
                            type="button"
                            onClick={() => onArchive(detail)}
                            className="inline-flex items-center gap-1 rounded-md border border-slate-700 px-2.5 py-1.5 text-xs text-slate-200"
                          >
                            <Archive className="h-3.5 w-3.5" /> Archive
                          </button>
                        ))}
                    </div>
                  </section>

                  <section>
                    <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500 mb-2">
                      Users ({detail.users?.length ?? 0})
                    </h3>
                    <ul className="space-y-2">
                      {(detail.users || []).map((u) => (
                        <li
                          key={u.id}
                          className="flex items-center justify-between gap-2 rounded-lg border border-slate-800 bg-slate-900/60 px-3 py-2"
                        >
                          <div className="min-w-0">
                            <div className="truncate text-sm text-white">
                              {u.display_name}{' '}
                              <span className="text-slate-500">· {u.role}</span>
                              {!u.is_active && (
                                <span className="ml-1 text-xs text-red-400">inactive</span>
                              )}
                            </div>
                            <div className="truncate text-[11px] text-slate-500">{u.email}</div>
                          </div>
                          {u.is_active && (
                            <button
                              type="button"
                              disabled={busyId === detail.entity_id}
                              onClick={() => onImpersonate(detail, u.id, u.email)}
                              className="shrink-0 rounded border border-slate-700 px-2 py-1 text-[11px] text-slate-300 hover:bg-slate-800"
                            >
                              Open
                            </button>
                          )}
                        </li>
                      ))}
                      {(detail.users || []).length === 0 && (
                        <li className="text-sm text-slate-500">No users.</li>
                      )}
                    </ul>
                  </section>

                  <section>
                    <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500 mb-2">
                      Recent audit
                    </h3>
                    <ul className="space-y-2">
                      {(detail.recent_audit || []).map((e) => (
                        <li key={e.id} className="text-xs border-b border-slate-800/80 pb-2">
                          <div className="flex justify-between gap-2">
                            <code className="text-indigo-300">{e.action}</code>
                            <span className="text-slate-500 whitespace-nowrap">{formatDate(e.created_at)}</span>
                          </div>
                          <div className="text-slate-500">{e.actor_email || '—'}</div>
                        </li>
                      ))}
                      {(detail.recent_audit || []).length === 0 && (
                        <li className="text-sm text-slate-500">No events for this tenant.</li>
                      )}
                    </ul>
                  </section>
                </>
              )}
            </div>
          </aside>
        </div>
      )}
    </div>
  );
}


function OperatorsPanel({
  loading,
  operators,
  onRefresh,
  setActionError,
}: {
  loading: boolean;
  operators: {
    id: string;
    email: string;
    display_name: string;
    role: string;
    is_active: boolean;
    last_login?: string;
    created_at: string;
  }[];
  onRefresh: () => void;
  setActionError: (s: string | null) => void;
}) {
  const [email, setEmail] = useState('');
  const [name, setName] = useState('');
  const [password, setPassword] = useState('');
  const [role, setRole] = useState('PlatformSupport');
  const [busy, setBusy] = useState(false);

  const create = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setActionError(null);
    try {
      await platformCreateOperator({
        email: email.trim(),
        display_name: name.trim(),
        password,
        role,
      });
      setEmail('');
      setName('');
      setPassword('');
      onRefresh();
    } catch (err) {
      setActionError(extractErr(err, 'Create operator failed'));
    } finally {
      setBusy(false);
    }
  };

  const toggle = async (id: string, is_active: boolean) => {
    setBusy(true);
    setActionError(null);
    try {
      await platformSetOperatorActive(id, is_active);
      onRefresh();
    } catch (err) {
      setActionError(extractErr(err, 'Update operator failed'));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-6">
      <form
        onSubmit={create}
        className="rounded-xl border border-slate-800 bg-slate-900 p-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-5"
      >
        <input
          className="rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white"
          placeholder="Email"
          type="email"
          required
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />
        <input
          className="rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white"
          placeholder="Display name"
          required
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <input
          className="rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white"
          placeholder="Temp password"
          type="password"
          required
          minLength={8}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        <select
          className="rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white"
          value={role}
          onChange={(e) => setRole(e.target.value)}
        >
          <option value="PlatformSupport">PlatformSupport</option>
          <option value="PlatformSuperAdmin">PlatformSuperAdmin</option>
        </select>
        <button
          type="submit"
          disabled={busy}
          className="rounded-lg bg-indigo-600 px-3 py-2 text-sm font-medium text-white hover:bg-indigo-500 disabled:opacity-50"
        >
          Add operator
        </button>
      </form>

      <div className="overflow-hidden rounded-xl border border-slate-800 bg-slate-900">
        {loading && <p className="p-8 text-center text-sm text-slate-500">Loading operators…</p>}
        {!loading && (
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-800 text-left text-xs uppercase tracking-wide text-slate-500">
                <th className="px-4 py-3">Name</th>
                <th className="px-4 py-3">Email</th>
                <th className="px-4 py-3">Role</th>
                <th className="px-4 py-3">Status</th>
                <th className="px-4 py-3 text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {operators.map((op) => (
                <tr key={op.id} className="border-b border-slate-800/80">
                  <td className="px-4 py-3 text-white">{op.display_name}</td>
                  <td className="px-4 py-3 text-slate-300">{op.email}</td>
                  <td className="px-4 py-3 text-slate-300">{op.role}</td>
                  <td className="px-4 py-3">
                    <span
                      className={`text-xs ${op.is_active ? 'text-emerald-400' : 'text-red-400'}`}
                    >
                      {op.is_active ? 'active' : 'inactive'}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => toggle(op.id, !op.is_active)}
                      className="rounded border border-slate-700 px-2 py-1 text-xs text-slate-200 hover:bg-slate-800"
                    >
                      {op.is_active ? 'Deactivate' : 'Activate'}
                    </button>
                  </td>
                </tr>
              ))}
              {operators.length === 0 && (
                <tr>
                  <td colSpan={5} className="px-4 py-8 text-center text-slate-500">
                    No operators yet.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        )}
      </div>
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
