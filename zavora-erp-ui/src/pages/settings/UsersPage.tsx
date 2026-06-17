import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getUsers, createUser, getIdentity } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { UserPlus } from 'lucide-react';

interface UserRow {
  id: string;
  email: string;
  display_name: string;
  role: string;
  is_active: boolean;
  status?: string;
  last_login?: string | null;
}

const ROLES = ['Owner', 'Admin', 'Accountant', 'Approver', 'Editor', 'Viewer'];

export default function UsersPage() {
  const queryClient = useQueryClient();
  const identity = getIdentity() as { role?: string } | null;
  const canManage = identity?.role === 'Owner' || identity?.role === 'Admin';

  const { data: users, isLoading } = useQuery<UserRow[]>({
    queryKey: ['users'],
    queryFn: () => getUsers().then((r) => r.data),
    enabled: canManage,
  });

  const [form, setForm] = useState({ email: '', display_name: '', role: 'Viewer', password: '' });
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

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
          : `${form.email} invited (no password set — they cannot sign in until one is set).`,
      );
      setForm({ email: '', display_name: '', role: 'Viewer', password: '' });
      queryClient.invalidateQueries({ queryKey: ['users'] });
    },
    onError: (e: any) => {
      setMsg(null);
      setErr(e?.response?.data?.error ?? 'Could not create the user.');
    },
  });

  if (!canManage) {
    return (
      <div>
        <PageHeader title="Users & Roles" subtitle="Manage who can access this workspace" />
        <div className="card p-6 text-sm text-gray-600">
          You need the Owner or Admin role to manage users.
        </div>
      </div>
    );
  }

  const roleBadge = (role: string) => {
    const tone =
      role === 'Owner' || role === 'Admin'
        ? 'bg-indigo-50 text-indigo-700'
        : role === 'Viewer'
        ? 'bg-gray-100 text-gray-600'
        : 'bg-emerald-50 text-emerald-700';
    return <span className={`px-2 py-0.5 rounded text-xs font-medium ${tone}`}>{role}</span>;
  };

  return (
    <div>
      <PageHeader title="Users & Roles" subtitle="Manage who can access this workspace" />

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* User list */}
        <div className="lg:col-span-2 card p-0 overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-gray-50 text-gray-500 text-left">
              <tr>
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Email</th>
                <th className="px-4 py-3 font-medium">Role</th>
                <th className="px-4 py-3 font-medium">Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {isLoading && (
                <tr><td colSpan={4} className="px-4 py-6 text-gray-400">Loading…</td></tr>
              )}
              {users?.map((u) => (
                <tr key={u.id}>
                  <td className="px-4 py-3 font-medium text-gray-900">{u.display_name}</td>
                  <td className="px-4 py-3 text-gray-600">{u.email}</td>
                  <td className="px-4 py-3">{roleBadge(u.role)}</td>
                  <td className="px-4 py-3 text-gray-600">
                    {u.status ?? (u.is_active ? 'active' : 'inactive')}
                  </td>
                </tr>
              ))}
              {users && users.length === 0 && (
                <tr><td colSpan={4} className="px-4 py-6 text-gray-400">No users yet.</td></tr>
              )}
            </tbody>
          </table>
        </div>

        {/* Invite form */}
        <div className="card p-5 space-y-3 h-fit">
          <h3 className="text-sm font-semibold text-gray-900 flex items-center gap-2">
            <UserPlus className="w-4 h-4" /> Add user
          </h3>
          {msg && (
            <div className="rounded-lg bg-green-50 border border-green-200 px-3 py-2 text-xs text-green-700">{msg}</div>
          )}
          {err && (
            <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-xs text-red-700">{err}</div>
          )}
          <div>
            <label className="label">Full name</label>
            <input className="input" value={form.display_name}
              onChange={(e) => setForm({ ...form, display_name: e.target.value })} />
          </div>
          <div>
            <label className="label">Email</label>
            <input className="input" type="email" value={form.email}
              onChange={(e) => setForm({ ...form, email: e.target.value })} />
          </div>
          <div>
            <label className="label">Role</label>
            <select className="input" value={form.role}
              onChange={(e) => setForm({ ...form, role: e.target.value })}>
              {ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Temporary password</label>
            <input className="input" type="password" placeholder="Min 8 chars (optional)"
              value={form.password}
              onChange={(e) => setForm({ ...form, password: e.target.value })} />
            <p className="text-xs text-gray-500 mt-1">
              Set a password so the user can sign in immediately. Leave blank to create an
              invited account (cannot sign in until a password is set).
            </p>
          </div>
          <button className="btn-primary w-full justify-center"
            disabled={invite.isPending || !form.email || !form.display_name}
            onClick={() => invite.mutate()}>
            {invite.isPending ? 'Adding…' : 'Add user'}
          </button>
        </div>
      </div>
    </div>
  );
}
