import { useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Building2, ChevronDown, Plus, Check, Loader2, X } from 'lucide-react';
import { getMyTenants, switchTenant, createTenant, storeSession, getIdentity } from '../../api/client';

interface Tenant {
  entity_id: string;
  name: string;
  currency: string;
  role: string;
  current: boolean;
}

/// In-app tenant switcher: lists the tenants the signed-in user belongs to,
/// switches between them (re-issues the session JWT and resets cached data), and
/// creates a new tenant. Lives in the app header.
export default function TenantSwitcher() {
  const [open, setOpen] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [switching, setSwitching] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const queryClient = useQueryClient();

  const { data } = useQuery<{ tenants: Tenant[] }>({
    queryKey: ['my-tenants'],
    queryFn: () => getMyTenants().then((r) => r.data),
  });
  const tenants = data?.tenants ?? [];
  const identity = getIdentity() as { entity_id?: string } | null;
  const current = tenants.find((t) => t.current) ?? tenants.find((t) => t.entity_id === identity?.entity_id);

  useEffect(() => {
    function onClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
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

  // Single-tenant users still see their org name, but no dropdown chrome is
  // needed unless they have more than one (or want to create one).
  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 rounded-lg py-1 px-2 hover:bg-gray-50 transition-colors max-w-[220px]"
        title="Switch tenant"
      >
        <div className="w-7 h-7 rounded-md bg-indigo-50 flex items-center justify-center shrink-0">
          <Building2 className="w-4 h-4 text-indigo-600" />
        </div>
        <span className="hidden md:block text-[13px] font-medium text-gray-700 truncate">
          {current?.name ?? 'Workspace'}
        </span>
        <ChevronDown className="w-4 h-4 text-gray-400 shrink-0" />
      </button>

      {open && (
        <div className="absolute left-0 mt-2 w-72 rounded-xl border border-gray-100 bg-white shadow-lg shadow-gray-200/60 py-1 z-50">
          <div className="px-3 py-2 text-[11px] font-semibold uppercase tracking-wide text-gray-400">
            Your tenants
          </div>
          <div className="max-h-64 overflow-y-auto">
            {tenants.map((t) => (
              <button
                key={t.entity_id}
                onClick={() => doSwitch(t.entity_id)}
                disabled={switching !== null}
                className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-50 transition-colors disabled:opacity-60"
              >
                <div className="w-7 h-7 rounded-md bg-gray-100 flex items-center justify-center shrink-0">
                  <Building2 className="w-4 h-4 text-gray-500" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-gray-800 truncate">{t.name}</p>
                  <p className="text-[11px] text-gray-400">{t.role} · {t.currency}</p>
                </div>
                {switching === t.entity_id ? (
                  <Loader2 className="w-4 h-4 text-indigo-500 animate-spin" />
                ) : t.current ? (
                  <Check className="w-4 h-4 text-green-500" />
                ) : null}
              </button>
            ))}
          </div>
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

  return (
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
    </div>
  );
}
