import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getAccounts, createAccount } from '../../api/client';
import type { Account } from '../../types';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus } from 'lucide-react';

export default function AccountsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const { data: accounts = [], isLoading } = useQuery<Account[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then(r => Array.isArray(r.data) ? r.data : []) });

  const columns: Column<Account>[] = [
    { key: 'code', header: 'Code', render: (r) => <span className="font-mono text-sm">{r.code}</span> },
    { key: 'name', header: 'Account Name', render: (r) => <span className="font-medium">{r.name}</span> },
    { key: 'account_type', header: 'Type', render: (r) => <span className="badge-info capitalize">{r.account_type.replace('_', ' ')}</span> },
    { key: 'parent_code', header: 'Parent', render: (r) => r.parent_code || '—' },
    { key: 'is_control', header: 'Control', render: (r) => r.is_control ? '✓' : '' },
    { key: 'is_active', header: 'Active', render: (r) => r.is_active ? <span className="badge-success">Active</span> : <span className="badge-gray">Inactive</span> },
  ];

  return (
    <div>
      <PageHeader title="Chart of Accounts" subtitle="Kenya standard chart of accounts" actions={<button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> New Account</button>} />
      <DataTable columns={columns} data={accounts} loading={isLoading} emptyMessage="No accounts. Seed from Kenya Standard template." />
      {showCreate && <CreateAccountModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateAccountModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ code: '', name: '', account_type: 'Expense', parent_code: '', is_control: false });
  const mutation = useMutation({ mutationFn: (data: any) => createAccount(data), onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['accounts'] }); onClose(); } });

  const handleSubmit = (e: React.FormEvent) => { e.preventDefault(); mutation.mutate({ ...form, parent_code: form.parent_code || undefined, tags: [] }); };

  return (
    <Modal open={true} onClose={onClose} title="New Account">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-2 gap-4">
          <div><label className="label">Code *</label><input className="input font-mono" value={form.code} onChange={(e) => setForm({ ...form, code: e.target.value })} placeholder="e.g. 7150" required /></div>
          <div><label className="label">Type *</label><select className="input" value={form.account_type} onChange={(e) => setForm({ ...form, account_type: e.target.value })}><option>Asset</option><option>Liability</option><option>Equity</option><option>Revenue</option><option>Expense</option><option>ContraAsset</option><option>ContraRevenue</option></select></div>
        </div>
        <div><label className="label">Name *</label><input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} required /></div>
        <div className="grid grid-cols-2 gap-4">
          <div><label className="label">Parent Code</label><input className="input font-mono" value={form.parent_code} onChange={(e) => setForm({ ...form, parent_code: e.target.value })} placeholder="e.g. 7000" /></div>
          <div className="flex items-end"><label className="flex items-center gap-2 pb-2"><input type="checkbox" checked={form.is_control} onChange={(e) => setForm({ ...form, is_control: e.target.checked })} /><span className="text-sm">Control Account</span></label></div>
        </div>
        <div className="flex justify-end gap-3 pt-4 border-t"><button type="button" onClick={onClose} className="btn-secondary">Cancel</button><button type="submit" className="btn-primary" disabled={mutation.isPending}>{mutation.isPending ? 'Creating...' : 'Create Account'}</button></div>
      </form>
    </Modal>
  );
}
