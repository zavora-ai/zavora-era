import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Building2, ChevronDown, Plus, Check, Loader2, X, Archive, ArchiveRestore, LogOut } from 'lucide-react';
import {
  getMyTenants,
  switchTenant,
  createTenant,
  archiveTenant,
  unarchiveTenant,
  leaveTenant,
  storeSession,
  getIdentity,
} from '../../api/client';

interface Tenant {
  entity_id: string;
  name: string;
  currency: string;
  role: string;
  current: boolean;
  archived: boolean;
}

type PendingAction =
  | { kind: 'archive' | 'leave'; tenant: Tenant }
  | null;

/// In-app tenant switcher: lists the tenants the signed-in user belongs to,
/// switches between them (re-issues the session JWT and resets cached data),
/// creates a new tenant, and manages each tenant's lifecycle — archive (close),
/// restore, and leave. Lives in the app header.
export default function TenantSwitcher() {
  const [open, setOpen] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [switching, setSwitching] = useState<string | null>(null);
  const [pending, setPending] = useState<PendingAction>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const queryClient = useQueryClient();

  // Include archived tenants so we can render a "Closed" section with a restore
  // affordance; active ones are split out below.
  const { data, refetch } = useQuery<{ tenants: Tenant[] }>({
    queryKey: ['my-tenants', 'with-archived'],
    queryFn: () => getMyTenants(true).then((r) => r.data),
  });
  const tenants = data?.tenants ?? [];
  const active = tenants.filter((t) => !t.archived);
  const archived = tenants.filter((t) => t.archived);
  const identity = getIdentity() as { entity_id?: string } | null;
  const current = active.find((t) => t.current) ?? active.find((t) => t.entity_id === identity?.entity_id);
  // An Owner may archive only when they have more than one active tenant.
  const canArchive = active.length > 1;

  useEffect(() => {
    function onClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener('mousedown', onClick);
    return () => document.removeEventListener('mousedown', onClick);
  }, []);

  const doSwitch = async (entityId: string) => {
    if (current?.entity_id === entityId) { setOpen(false); return; }
    setSwitching(entityId);
    try {
      const resp = await switchTenant(entityId);
      storeSession(resp.data);
      // The session now points at a different tenant — drop all cached data so
      // nothing from the previous tenant lingers, then reload into the new one.
      queryClient.clear();
      window.location.assign('/');
    } catch {
      setSwitching(null);
    }
  };

  const confirmAction = async () => {
    if (!pending) return;
    setBusy(true);
    setActionError(null);
    try {
      if (pending.kind === 'archive') await archiveTenant(pending.tenant.entity_id);
      else await leaveTenant(pending.tenant.entity_id);
      setPending(null);
      await refetch();
    } catch (err: any) {
      setActionError(err?.response?.data?.error || 'Action failed.');
    } finally {
      setBusy(false);
    }
  };

  const doRestore = async (entityId: string) => {
    setBusy(true);
    try {
      await unarchiveTenant(entityId);
      await refetch();
    } catch {
      /* surfaced on next list */
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 rounded-lg py-1 px-2 hover:bg-gray-50 transition-colors shrink-0"
        title="Switch tenant"
      >
        <div className="w-7 h-7 rounded-md bg-indigo-50 flex items-center justify-center shrink-0">
          <Building2 className="w-4 h-4 text-indigo-600" />
        </div>
        <span className="hidden md:block text-[13px] font-medium text-gray-700 whitespace-nowrap">
          {current?.name ?? 'Workspace'}
        </span>
        <ChevronDown className="w-4 h-4 text-gray-400 shrink-0" />
      </button>

      {open && (
        <div className="absolute left-0 mt-2 w-80 rounded-xl border border-gray-100 bg-white shadow-lg shadow-gray-200/60 py-1 z-50">
          <div className="px-3 py-2 text-[11px] font-semibold uppercase tracking-wide text-gray-400">
            Your tenants
          </div>
          <div className="max-h-72 overflow-y-auto">
            {active.map((t) => (
              <div key={t.entity_id} className="flex items-center hover:bg-gray-50 transition-colors">
                <button
                  onClick={() => doSwitch(t.entity_id)}
                  disabled={switching !== null}
                  className="flex-1 flex items-center gap-2 px-3 py-2 text-left disabled:opacity-60 min-w-0"
                >
                  <div className="w-7 h-7 rounded-md bg-gray-100 flex items-center justify-center shrink-0">
                    <Building2 className="w-4 h-4 text-gray-500" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm text-gray-800 break-words">{t.name}</p>
                    <p className="text-[11px] text-gray-400">{t.role} · {t.currency}</p>
                  </div>
                  {switching === t.entity_id ? (
                    <Loader2 className="w-4 h-4 text-indigo-500 animate-spin" />
                  ) : t.current ? (
                    <Check className="w-4 h-4 text-green-500" />
                  ) : null}
                </button>
                {/* Inline, always-visible lifecycle action — Owners archive,
                    members leave. Kept inline (not a popout) so it is never
                    clipped by the scroll container or hard to find. */}
                {t.role === 'Owner' ? (
                  <button
                    onClick={() => { setActionError(null); setPending({ kind: 'archive', tenant: t }); }}
                    disabled={!canArchive}
                    title={canArchive ? 'Archive (close) this tenant' : 'You cannot archive your only active tenant'}
                    aria-label={`Archive ${t.name}`}
                    className="shrink-0 mr-2 p-1.5 rounded-md text-gray-400 hover:text-gray-700 hover:bg-gray-100 disabled:opacity-30 disabled:cursor-not-allowed disabled:hover:bg-transparent"
                  >
                    <Archive className="w-4 h-4" />
                  </button>
                ) : (
                  <button
                    onClick={() => { setActionError(null); setPending({ kind: 'leave', tenant: t }); }}
                    title="Leave this tenant"
                    aria-label={`Leave ${t.name}`}
                    className="shrink-0 mr-2 p-1.5 rounded-md text-gray-400 hover:text-red-600 hover:bg-red-50"
                  >
                    <LogOut className="w-4 h-4" />
                  </button>
                )}
              </div>
            ))}
          </div>

          {archived.length > 0 && (
            <div className="border-t border-gray-100 mt-1 pt-1">
              <div className="px-3 py-2 text-[11px] font-semibold uppercase tracking-wide text-gray-400">
                Closed
              </div>
              {archived.map((t) => (
                <div key={t.entity_id} className="flex items-center gap-2 px-3 py-2">
                  <div className="w-7 h-7 rounded-md bg-gray-50 flex items-center justify-center shrink-0">
                    <Archive className="w-4 h-4 text-gray-300" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm text-gray-400 break-words">{t.name}</p>
                    <p className="text-[11px] text-gray-300">{t.role} · archived</p>
                  </div>
                  {t.role === 'Owner' && (
                    <button
                      onClick={() => doRestore(t.entity_id)}
                      disabled={busy}
                      className="flex items-center gap-1 text-[12px] text-indigo-600 hover:text-indigo-700 disabled:opacity-50 shrink-0"
                      title="Restore tenant"
                    >
                      <ArchiveRestore className="w-3.5 h-3.5" /> Restore
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}

          <div className="border-t border-gray-100 mt-1 pt-1">
            <button
              onClick={() => { setOpen(false); setShowCreate(true); }}
              className="w-full flex items-center gap-2 px-3 py-2.5 text-sm text-indigo-600 hover:bg-indigo-50 transition-colors"
            >
              <Plus className="w-4 h-4" /> Create a new tenant
            </button>
          </div>
        </div>
      )}

      {pending && (
        <ConfirmActionModal
          action={pending}
          busy={busy}
          error={actionError}
          onCancel={() => { setPending(null); setActionError(null); }}
          onConfirm={confirmAction}
        />
      )}

      {showCreate && (
        <CreateTenantModal
          onClose={() => setShowCreate(false)}
          onCreated={(resp) => {
            storeSession(resp);
            queryClient.clear();
            window.location.assign('/');
          }}
        />
      )}
    </div>
  );
}

function ConfirmActionModal({
  action,
  busy,
  error,
  onCancel,
  onConfirm,
}: {
  action: NonNullable<PendingAction>;
  busy: boolean;
  error: string | null;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const isArchive = action.kind === 'archive';
  const title = isArchive ? 'Archive this tenant?' : 'Leave this tenant?';
  const body = isArchive
    ? `"${action.tenant.name}" will be closed and hidden from your workspace. Its books and audit trail are preserved, and you can restore it later.`
    : `You will lose access to "${action.tenant.name}". Other members and the tenant's data are unaffected. An Owner can re-invite you later.`;
  const confirmLabel = isArchive ? 'Archive' : 'Leave';

  return createPortal(
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-[60]">
      <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-lg font-semibold">{title}</h2>
          <button onClick={onCancel} className="text-gray-400 hover:text-gray-600"><X className="w-5 h-5" /></button>
        </div>
        <p className="text-sm text-gray-500 mb-4">{body}</p>
        {error && <div className="bg-red-50 text-red-700 text-sm p-3 rounded-lg mb-4">{error}</div>}
        <div className="flex justify-end gap-3">
          <button type="button" className="btn-secondary" onClick={onCancel} disabled={busy}>Cancel</button>
          <button
            type="button"
            className={isArchive ? 'btn-primary' : 'btn-primary !bg-red-600 hover:!bg-red-700'}
            onClick={onConfirm}
            disabled={busy}
          >
            {busy ? 'Working…' : confirmLabel}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

function CreateTenantModal({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: (session: { access_token: string; user?: unknown }) => void;
}) {
  const [name, setName] = useState('');
  const [type, setType] = useState('limited_company');
  const [kraPin, setKraPin] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) { setError('Organization name is required'); return; }
    setBusy(true);
    setError(null);
    try {
      const resp = await createTenant({
        organization_name: name.trim(),
        organization_type: type,
        kra_pin: kraPin.trim() || undefined,
      });
      onCreated(resp.data);
    } catch (err: any) {
      setError(err?.response?.data?.error || 'Failed to create tenant.');
      setBusy(false);
    }
  };

  return createPortal(
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-[60]">
      <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">Create a new tenant</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600"><X className="w-5 h-5" /></button>
        </div>
        <p className="text-sm text-gray-500 mb-4">
          A fresh, isolated company with its own chart of accounts. You'll be the Owner and switched into it.
        </p>
        {error && <div className="bg-red-50 text-red-700 text-sm p-3 rounded-lg mb-4">{error}</div>}
        <form onSubmit={submit} className="space-y-4">
          <div>
            <label className="label">Organization name *</label>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Acme Holdings Ltd" autoFocus />
          </div>
          <div>
            <label className="label">Organization type</label>
            <select className="input" value={type} onChange={(e) => setType(e.target.value)}>
              <option value="limited_company">Limited Company</option>
              <option value="sole_proprietor">Sole Proprietor</option>
              <option value="partnership">Partnership</option>
              <option value="ngo">NGO</option>
              <option value="other">Other</option>
            </select>
          </div>
          <div>
            <label className="label">KRA PIN <span className="text-gray-400 font-normal">(optional)</span></label>
            <input className="input" value={kraPin} onChange={(e) => setKraPin(e.target.value)} placeholder="P051234567X" />
          </div>
          <div className="flex justify-end gap-3 pt-2">
            <button type="button" className="btn-secondary" onClick={onClose} disabled={busy}>Cancel</button>
            <button type="submit" className="btn-primary" disabled={busy}>
              {busy ? 'Creating…' : 'Create & switch'}
            </button>
          </div>
        </form>
      </div>
    </div>,
    document.body,
  );
}
