import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getUsers, createUser, updateUser, resendInvite, getRoles, getIdentity } from '../../api/client';
import { usePermissions } from '../../hooks/usePermissions';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import { UserPlus, Pencil, Mail, Power } from 'lucide-react';

interface UserRow {
  id: string;
  email: string;
  display_name: string;
  role: string;
  is_active: boolean;
  status?: string;
  last_login?: string | null;
}
interface Role { key: string; name: string; description?: string; is_system: boolean }

export default function UsersPage() {
  const queryClient = useQueryClient();
  const identity = getIdentity() as { role?: string; user_id?: string } | null;
  const { can, loaded } = usePermissions();
  const canManage = can('user.manage');
  const myId = identity?.user_id;

  const { data: users, isLoading } = useQuery<UserRow[]>({
    queryKey: ['users'],
    queryFn: () => getUsers().then((r) => r.data),
    enabled: canManage,
  });
  const { data: roles = [] } = useQuery<Role[]>({
    queryKey: ['roles'],
    queryFn: () => getRoles().then((r) => r.data),
    enabled: canManage,
  });

  const [form, setForm] = useState({ email: '', display_name: '', role: 'Viewer', password: '' });
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [editing, setEditing] = useState<UserRow | null>(null);

  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['users'] });

  const invite = useMutation({
    mutationFn: () =>
      createUser({
        email: form.email.trim(),
        display_name: form.display_name.trim(),
        role: form.role,
        password: form.password ? form.password : undefined,
      }),
    onSuccess: (resp: any) => {
      setErr(null);
      setMsg(
        resp?.data?.status === 'active'
          ? `${form.email} created and can sign in now.`
          : `${form.email} invited — an activation email has been sent.`,
      );
      setForm({ email: '', display_name: '', role: 'Viewer', password: '' });
      invalidate();
    },
    onError: (e: any) => { setMsg(null); setErr(e?.response?.data?.error ?? 'Could not create the user.'); },
  });

  const setActive = useMutation({
    mutationFn: (v: { id: string; is_active: boolean }) => updateUser(v.id, { is_active: v.is_active }),
    onSuccess: () => { setErr(null); invalidate(); },
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Could not update the user.'),
  });
  const resend = useMutation({
    mutationFn: (id: string) => resendInvite(id),
    onSuccess: () => { setErr(null); setMsg('Invitation re-sent.'); },
    onError: (e: any) => { setMsg(null); setErr(e?.response?.data?.error ?? 'Could not resend the invite.'); },
  });

  if (!canManage) {
    return (
      <div>
        <PageHeader title="Users & Roles" subtitle="Manage who can access this workspace" />
        <div className="card p-6 text-sm text-gray-600">
          {loaded ? 'You need the Manage-workspace permission to manage users.' : 'Loading…'}
        </div>
      </div>
    );
  }

  const roleBadge = (role: string) => {
    const tone = role === 'Owner' || role === 'Admin' ? 'bg-indigo-50 text-indigo-700'
      : role === 'Viewer' ? 'bg-gray-100 text-gray-600' : 'bg-emerald-50 text-emerald-700';
    return <span className={`px-2 py-0.5 rounded text-xs font-medium ${tone}`}>{role}</span>;
  };
  const statusBadge = (u: UserRow) => {
    const s = u.status === 'invited' ? 'invited' : u.is_active ? 'active' : 'deactivated';
    const tone = s === 'active' ? 'bg-green-100 text-green-700' : s === 'invited' ? 'bg-amber-100 text-amber-700' : 'bg-gray-100 text-gray-500';
    return <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${tone}`}>{s}</span>;
  };

  return (
    <div>
      <PageHeader title="Users & Roles" subtitle="Manage who can access this workspace" />
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2 card p-0 overflow-hidden">
          {err && <div className="m-3 rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-xs text-red-700">{err}</div>}
          <table className="w-full text-sm">
            <thead className="bg-gray-50 text-gray-500 text-left">
              <tr>
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Email</th>
                <th className="px-4 py-3 font-medium">Role</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {isLoading && <tr><td colSpan={5} className="px-4 py-6 text-gray-400">Loading…</td></tr>}
              {users?.map((u) => {
                const isSelf = u.id === myId;
                const invited = u.status === 'invited';
                return (
                  <tr key={u.id}>
                    <td className="px-4 py-3 font-medium text-gray-900">
                      {u.display_name}{isSelf && <span className="ml-2 text-[10px] text-gray-400">(you)</span>}
                    </td>
                    <td className="px-4 py-3 text-gray-600">{u.email}</td>
                    <td className="px-4 py-3">{roleBadge(u.role)}</td>
                    <td className="px-4 py-3">{statusBadge(u)}</td>
                    <td className="px-4 py-3">
                      <div className="flex items-center justify-end gap-1.5">
                        <button title="Edit" className="p-1.5 text-gray-500 hover:bg-gray-100 rounded" onClick={() => { setErr(null); setEditing(u); }}>
                          <Pencil className="w-4 h-4" />
                        </button>
                        {invited && (
                          <button title="Resend invite" className="p-1.5 text-indigo-600 hover:bg-indigo-50 rounded" disabled={resend.isPending} onClick={() => resend.mutate(u.id)}>
                            <Mail className="w-4 h-4" />
                          </button>
                        )}
                        <button
                          title={isSelf ? "You can't deactivate yourself" : u.is_active ? 'Deactivate' : 'Reactivate'}
                          className={`p-1.5 rounded ${u.is_active ? 'text-red-600 hover:bg-red-50' : 'text-green-600 hover:bg-green-50'} disabled:opacity-30 disabled:cursor-not-allowed`}
                          disabled={isSelf || setActive.isPending}
                          onClick={() => setActive.mutate({ id: u.id, is_active: !u.is_active })}
                        >
                          <Power className="w-4 h-4" />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
              {users && users.length === 0 && <tr><td colSpan={5} className="px-4 py-6 text-gray-400">No users yet.</td></tr>}
            </tbody>
          </table>
        </div>

        {/* Invite form */}
        <div className="card p-5 space-y-3 h-fit">
          <h3 className="text-sm font-semibold text-gray-900 flex items-center gap-2"><UserPlus className="w-4 h-4" /> Add user</h3>
          {msg && <div className="rounded-lg bg-green-50 border border-green-200 px-3 py-2 text-xs text-green-700">{msg}</div>}
          <div><label className="label">Full name</label>
            <input className="input" value={form.display_name} onChange={(e) => setForm({ ...form, display_name: e.target.value })} /></div>
          <div><label className="label">Email</label>
            <input className="input" type="email" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} /></div>
          <div><label className="label">Role</label>
            <select className="input" value={form.role} onChange={(e) => setForm({ ...form, role: e.target.value })}>
              {roles.map((r) => <option key={r.key} value={r.key}>{r.name}{r.is_system ? '' : ' (custom)'}</option>)}
            </select>
            {roles.find((r) => r.key === form.role)?.description && (
              <p className="text-xs text-gray-500 mt-1">{roles.find((r) => r.key === form.role)?.description}</p>
            )}
          </div>
          <div><label className="label">Temporary password</label>
            <input className="input" type="password" placeholder="Min 8 chars (optional)" value={form.password}
              onChange={(e) => setForm({ ...form, password: e.target.value })} />
            <p className="text-xs text-gray-500 mt-1">Leave blank to email an activation link so the user sets their own password.</p>
          </div>
          <button className="btn-primary w-full justify-center" disabled={invite.isPending || !form.email || !form.display_name}
            onClick={() => invite.mutate()}>{invite.isPending ? 'Adding…' : 'Add user'}</button>
        </div>
      </div>

      {editing && (
        <EditUserModal user={editing} roles={roles} isSelf={editing.id === myId}
          onClose={() => setEditing(null)}
          onSaved={() => { setEditing(null); invalidate(); }} />
      )}
    </div>
  );
}

function EditUserModal({ user, roles, isSelf, onClose, onSaved }: {
  user: UserRow; roles: Role[]; isSelf: boolean; onClose: () => void; onSaved: () => void;
}) {
  const [role, setRole] = useState(user.role);
  const [isActive, setIsActive] = useState(user.is_active);
  const [err, setErr] = useState<string | null>(null);
  const save = useMutation({
    mutationFn: () => updateUser(user.id, { role, is_active: isActive }),
    onSuccess: onSaved,
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Could not save changes.'),
  });
  return (
    <Modal open onClose={onClose} title={`Edit ${user.display_name}`} subtitle={user.email}>
      <div className="space-y-4">
        {err && <div className="text-sm text-red-600 bg-red-50 rounded-lg px-3 py-2">{err}</div>}
        <div>
          <label className="label">Role</label>
          <select className="input" value={role} disabled={isSelf} onChange={(e) => setRole(e.target.value)}>
            {roles.map((r) => <option key={r.key} value={r.key}>{r.name}{r.is_system ? '' : ' (custom)'}</option>)}
          </select>
          {isSelf && <p className="text-xs text-amber-600 mt-1">You cannot change your own role.</p>}
        </div>
        <label className={`flex items-center gap-2 text-sm ${isSelf ? 'opacity-50' : ''}`}>
          <input type="checkbox" checked={isActive} disabled={isSelf} onChange={(e) => setIsActive(e.target.checked)} />
          Active (can sign in)
        </label>
        {isSelf && <p className="text-xs text-amber-600 -mt-2">You cannot deactivate your own account.</p>}
        <div className="flex justify-end gap-2 pt-2">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={save.isPending} onClick={() => { setErr(null); save.mutate(); }}>
            {save.isPending ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
