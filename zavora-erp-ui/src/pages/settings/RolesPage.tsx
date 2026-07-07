import { useEffect, useMemo, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getRoles, getRole, getPermissionsCatalog, createRole, updateRole, deleteRole,
} from '../../api/client';
import { usePermissions } from '../../hooks/usePermissions';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import { Shield, Lock, Plus, Copy, Trash2 } from 'lucide-react';

interface Role { id: string; key: string; name: string; description?: string; is_system: boolean }
interface Permission { key: string; category: string; label: string; description?: string }

export default function RolesPage() {
  const qc = useQueryClient();
  const { can, loaded } = usePermissions();
  const canManage = can('role.read');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState<{ cloneFrom?: Role } | null>(null);

  const { data: roles = [] } = useQuery<Role[]>({
    queryKey: ['roles'], queryFn: () => getRoles().then((r) => r.data), enabled: canManage,
  });
  const { data: catalog = [] } = useQuery<Permission[]>({
    queryKey: ['permissions-catalog'], queryFn: () => getPermissionsCatalog().then((r) => r.data), enabled: canManage,
  });

  useEffect(() => {
    if (!selectedId && roles.length) setSelectedId(roles[0].id);
  }, [roles, selectedId]);

  if (!canManage) {
    return (
      <div>
        <PageHeader title="Roles" subtitle="Define what each role can do" />
        <div className="card p-6 text-sm text-gray-600">{loaded ? 'You need the Manage-workspace permission to manage roles.' : 'Loading…'}</div>
      </div>
    );
  }

  const selected = roles.find((r) => r.id === selectedId) || null;

  return (
    <div>
      <PageHeader title="Roles" subtitle="Define what each role can do" actions={
        <button className="btn-primary" onClick={() => setShowCreate({})}><Plus className="w-4 h-4" /> New Role</button>
      } />
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Role list */}
        <div className="card p-2 h-fit">
          {roles.map((r) => (
            <button key={r.id} onClick={() => setSelectedId(r.id)}
              className={`w-full text-left px-3 py-2.5 rounded-lg flex items-center gap-2 transition-colors ${
                selectedId === r.id ? 'bg-indigo-50 text-indigo-700' : 'hover:bg-gray-50 text-gray-700'}`}>
              {r.is_system ? <Lock className="w-4 h-4 shrink-0 text-gray-400" /> : <Shield className="w-4 h-4 shrink-0 text-emerald-500" />}
              <span className="flex-1 min-w-0">
                <span className="block text-sm font-medium truncate">{r.name}</span>
                <span className="block text-[11px] text-gray-400">{r.is_system ? 'Built-in' : 'Custom'}</span>
              </span>
            </button>
          ))}
        </div>
        {/* Permission matrix */}
        <div className="lg:col-span-2">
          {selected && (
            <RoleMatrix role={selected} catalog={catalog}
              onDuplicate={() => setShowCreate({ cloneFrom: selected })}
              onDeleted={() => { setSelectedId(null); qc.invalidateQueries({ queryKey: ['roles'] }); }} />
          )}
        </div>
      </div>
      {showCreate && (
        <CreateRoleModal catalog={catalog} cloneFrom={showCreate.cloneFrom}
          onClose={() => setShowCreate(null)}
          onCreated={(id) => { setShowCreate(null); qc.invalidateQueries({ queryKey: ['roles'] }); setSelectedId(id); }} />
      )}
    </div>
  );
}

function groupByCategory(catalog: Permission[]) {
  const groups: Record<string, Permission[]> = {};
  for (const p of catalog) (groups[p.category] ??= []).push(p);
  return Object.entries(groups);
}

function RoleMatrix({ role, catalog, onDuplicate, onDeleted }: {
  role: Role; catalog: Permission[]; onDuplicate: () => void; onDeleted: () => void;
}) {
  const qc = useQueryClient();
  const { data } = useQuery<{ role: Role; permissions: string[] }>({
    queryKey: ['role', role.id], queryFn: () => getRole(role.id).then((r) => r.data),
  });
  const [sel, setSel] = useState<Set<string> | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => { if (data) setSel(new Set(data.permissions)); }, [data]);

  const groups = useMemo(() => groupByCategory(catalog), [catalog]);
  const readOnly = role.is_system;
  const current = sel ?? new Set<string>();
  const dirty = data ? (current.size !== data.permissions.length || data.permissions.some((k) => !current.has(k))) : false;

  const toggle = (key: string) => {
    if (readOnly) return;
    const n = new Set(current); n.has(key) ? n.delete(key) : n.add(key); setSel(n);
  };
  const toggleGroup = (perms: Permission[]) => {
    if (readOnly) return;
    const allOn = perms.every((p) => current.has(p.key));
    const n = new Set(current);
    perms.forEach((p) => (allOn ? n.delete(p.key) : n.add(p.key)));
    setSel(n);
  };

  const save = useMutation({
    mutationFn: () => updateRole(role.id, { permissions: [...current] }),
    onSuccess: () => {
      setErr(null);
      qc.invalidateQueries({ queryKey: ['role', role.id] });
      qc.invalidateQueries({ queryKey: ['auth-permissions'] }); // my own can() may change
    },
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Could not save.'),
  });
  const del = useMutation({
    mutationFn: () => deleteRole(role.id),
    onSuccess: onDeleted,
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Could not delete.'),
  });

  if (!sel) return <div className="card p-6 text-sm text-gray-500">Loading…</div>;

  return (
    <div className="card p-5">
      <div className="flex items-start justify-between mb-4">
        <div>
          <h3 className="text-base font-semibold text-gray-900 flex items-center gap-2">
            {role.is_system ? <Lock className="w-4 h-4 text-gray-400" /> : <Shield className="w-4 h-4 text-emerald-500" />}
            {role.name}
          </h3>
          {role.description && <p className="text-sm text-gray-500 mt-0.5">{role.description}</p>}
        </div>
        <div className="flex items-center gap-2">
          <button className="btn-secondary text-xs py-1" onClick={onDuplicate}><Copy className="w-3.5 h-3.5" /> Duplicate</button>
          {!role.is_system && (
            <button className="btn-secondary text-xs py-1 text-red-600" disabled={del.isPending}
              onClick={() => { if (confirm(`Delete role "${role.name}"?`)) del.mutate(); }}>
              <Trash2 className="w-3.5 h-3.5" /> Delete
            </button>
          )}
        </div>
      </div>

      {readOnly && (
        <div className="mb-4 rounded-lg bg-gray-50 border border-gray-200 px-3 py-2 text-xs text-gray-500">
          Built-in role — permissions are read-only. Duplicate it to create an editable custom role.
        </div>
      )}
      {err && <div className="mb-3 rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-xs text-red-700">{err}</div>}

      <div className="space-y-4">
        {groups.map(([category, perms]) => {
          const allOn = perms.every((p) => current.has(p.key));
          const someOn = perms.some((p) => current.has(p.key));
          return (
            <div key={category}>
              <label className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-gray-500 mb-1.5">
                <input type="checkbox" checked={allOn} ref={(el) => { if (el) el.indeterminate = someOn && !allOn; }}
                  disabled={readOnly} onChange={() => toggleGroup(perms)} />
                {category}
              </label>
              <div className="space-y-1 pl-1">
                {perms.map((p) => (
                  <label key={p.key} className={`flex items-start gap-2 text-sm py-1 ${readOnly ? 'text-gray-500' : 'text-gray-800 cursor-pointer'}`}>
                    <input type="checkbox" className="mt-0.5" checked={current.has(p.key)} disabled={readOnly} onChange={() => toggle(p.key)} />
                    <span>
                      <span className="font-medium">{p.label}</span>
                      {p.description && <span className="block text-xs text-gray-400">{p.description}</span>}
                    </span>
                  </label>
                ))}
              </div>
            </div>
          );
        })}
      </div>

      {!readOnly && (
        <div className="mt-5 flex items-center justify-end gap-3 border-t border-gray-100 pt-4">
          <span className="text-xs text-gray-400">{dirty ? 'Unsaved changes' : 'No changes'}</span>
          <button className="btn-secondary" disabled={!dirty || save.isPending} onClick={() => data && setSel(new Set(data.permissions))}>Discard</button>
          <button className="btn-primary" disabled={!dirty || save.isPending} onClick={() => save.mutate()}>{save.isPending ? 'Saving…' : 'Save changes'}</button>
        </div>
      )}
    </div>
  );
}

function CreateRoleModal({ catalog, cloneFrom, onClose, onCreated }: {
  catalog: Permission[]; cloneFrom?: Role; onClose: () => void; onCreated: (id: string) => void;
}) {
  const [name, setName] = useState(cloneFrom ? `${cloneFrom.name} (copy)` : '');
  const [description, setDescription] = useState('');
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [err, setErr] = useState<string | null>(null);

  // When cloning, preload the source role's permissions.
  const { data: cloneData } = useQuery<{ permissions: string[] }>({
    queryKey: ['role', cloneFrom?.id, 'clone'], enabled: !!cloneFrom,
    queryFn: () => getRole(cloneFrom!.id).then((r) => r.data),
  });
  useEffect(() => { if (cloneData) setSel(new Set(cloneData.permissions)); }, [cloneData]);

  const groups = useMemo(() => groupByCategory(catalog), [catalog]);
  const toggle = (k: string) => { const n = new Set(sel); n.has(k) ? n.delete(k) : n.add(k); setSel(n); };

  const create = useMutation({
    mutationFn: () => createRole({ name: name.trim(), description: description || undefined, permissions: [...sel] }),
    onSuccess: (r: any) => onCreated(r.data.id),
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Could not create the role.'),
  });

  return (
    <Modal open onClose={onClose} title={cloneFrom ? `Duplicate "${cloneFrom.name}"` : 'New role'} subtitle="Create a custom role for this workspace" size="lg">
      <div className="space-y-4">
        {err && <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">{err}</div>}
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Name</label><input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Sales Lead" /></div>
          <div><label className="label">Description</label><input className="input" value={description} onChange={(e) => setDescription(e.target.value)} /></div>
        </div>
        <div>
          <label className="label">Permissions</label>
          <div className="space-y-3 max-h-[40vh] overflow-y-auto border border-gray-100 rounded-lg p-3">
            {groups.map(([category, perms]) => (
              <div key={category}>
                <p className="text-xs font-semibold uppercase tracking-wider text-gray-500 mb-1">{category}</p>
                <div className="space-y-1 pl-1">
                  {perms.map((p) => (
                    <label key={p.key} className="flex items-start gap-2 text-sm py-0.5 cursor-pointer">
                      <input type="checkbox" className="mt-0.5" checked={sel.has(p.key)} onChange={() => toggle(p.key)} />
                      <span><span className="font-medium">{p.label}</span>{p.description && <span className="block text-xs text-gray-400">{p.description}</span>}</span>
                    </label>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
        <div className="flex justify-end gap-2 pt-2">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!name.trim() || create.isPending} onClick={() => { setErr(null); create.mutate(); }}>
            {create.isPending ? 'Creating…' : 'Create role'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
