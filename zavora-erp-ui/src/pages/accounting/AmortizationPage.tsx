import { useState } from 'react';
import { useToast } from '../../components/toast/ToastProvider';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getAmortization, createAmortization, runAmortization, cancelAmortization, getAccounts } from '../../api/client';
import type { Account } from '../../types';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { formatCurrency, formatDate } from '../../utils/format';
import { workToday } from '../../utils/workDate';
import { Plus, Play } from 'lucide-react';

// Prepayments & deferred revenue: spread an upfront amount over months. Each
// month the scheduler (or the Run button) posts an installment releasing the
// balance-sheet holding account into P&L.
export default function AmortizationPage() {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [showCreate, setShowCreate] = useState(false);
  const { data: rows = [], isLoading } = useQuery<any[]>({
    queryKey: ['amortization'],
    queryFn: () => getAmortization().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });
  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['amortization'] });
  const runMut = useMutation({
    mutationFn: () => runAmortization(),
    onSuccess: (r: any) => { invalidate(); toast.success(`Posted installments for ${r.data?.posted_schedules ?? 0} schedule(s).`); },
    onError: (e: any) => toast.fromError(e, 'Run failed.'),
  });
  const cancelMut = useMutation({
    mutationFn: (id: string) => cancelAmortization(id),
    onSuccess: invalidate,
    onError: (e: any) => toast.fromError(e, 'Cancel failed.'),
  });

  const kindLabel = (k: string) => (k === 'deferred_revenue' ? 'Deferred revenue' : 'Prepaid expense');
  const statusColor: Record<string, string> = {
    active: 'text-green-700 bg-green-50',
    completed: 'text-gray-500 bg-gray-100',
    cancelled: 'text-red-600 bg-red-50',
  };

  const columns: Column<any>[] = [
    { key: 'description', header: 'Description', render: (r) => <span className="font-medium">{r.description}</span> },
    { key: 'kind', header: 'Type', render: (r) => kindLabel(r.kind) },
    { key: 'total_amount', header: 'Total', className: 'text-right', render: (r) => <span className="text-right block">{formatCurrency(r.total_amount)}</span> },
    { key: 'progress', header: 'Progress', render: (r) => `${r.amortized_periods} / ${r.periods} months` },
    { key: 'start_date', header: 'Starts', render: (r) => formatDate(r.start_date) },
    { key: 'status', header: 'Status', render: (r) => <span className={`text-xs px-2 py-0.5 rounded-full ${statusColor[r.status] ?? ''}`}>{r.status}</span> },
    {
      key: 'actions', header: '', render: (r) => (
        r.status === 'active' ? (
          <button onClick={(e) => { e.stopPropagation(); if (confirm(`Cancel "${r.description}"? Posted installments stand; future ones stop.`)) cancelMut.mutate(r.id); }} className="btn-secondary text-xs py-1 px-2 text-red-600">Cancel</button>
        ) : null
      )
    },
  ];

  return (
    <div>
      <PageHeader
        title="Amortisation"
        subtitle="Prepayments & deferred revenue — spread an upfront amount to the P&L over months."
        actions={
          <div className="flex gap-2">
            <button onClick={() => runMut.mutate()} disabled={runMut.isPending} className="btn-secondary"><Play className="w-4 h-4" /> Run due installments</button>
            <button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> New schedule</button>
          </div>
        }
      />
      <DataTable columns={columns} data={rows} loading={isLoading} emptyMessage="No amortisation schedules yet. Create one for a prepaid expense or deferred revenue." />
      {showCreate && <CreateModal onClose={() => { setShowCreate(false); invalidate(); }} />}
    </div>
  );
}

function CreateModal({ onClose }: { onClose: () => void }) {
  const [form, setForm] = useState({
    kind: 'prepaid_expense',
    description: '',
    balance_account: '1400',
    pnl_account: '',
    total_amount: '',
    periods: '12',
    start_date: workToday(),
  });
  const [error, setError] = useState('');
  const { data: accounts = [] } = useQuery<Account[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then((r) => (Array.isArray(r.data) ? r.data : [])) });

  const isPrepaid = form.kind === 'prepaid_expense';
  // Balance account: prepaid → asset (1400 default); deferred → liability (3450).
  const balanceAccounts = accounts.filter((a) => (isPrepaid ? a.account_type === 'Asset' : a.account_type === 'Liability') && a.is_active && !a.is_control);
  // P&L account: prepaid → expense; deferred → revenue.
  const pnlAccounts = accounts.filter((a) => (isPrepaid ? a.account_type === 'Expense' : (a.account_type === 'Revenue')) && a.is_active && !a.is_control);

  const mutation = useMutation({
    mutationFn: () => createAmortization({
      kind: form.kind,
      description: form.description.trim(),
      balance_account: form.balance_account,
      pnl_account: form.pnl_account,
      total_amount: parseFloat(form.total_amount),
      periods: parseInt(form.periods, 10),
      start_date: form.start_date,
    }),
    onSuccess: onClose,
    onError: (e: any) => setError(e?.response?.data?.error || 'Could not create schedule.'),
  });

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    if (!form.description.trim()) return setError('Description is required.');
    if (!form.pnl_account) return setError(`Select a ${isPrepaid ? 'expense' : 'revenue'} account.`);
    if (!(parseFloat(form.total_amount) > 0)) return setError('Total must be positive.');
    if (!(parseInt(form.periods, 10) > 0)) return setError('Months must be at least 1.');
    mutation.mutate();
  };

  const monthly = parseFloat(form.total_amount) && parseInt(form.periods, 10)
    ? (parseFloat(form.total_amount) / parseInt(form.periods, 10)) : 0;

  return (
    <Modal open onClose={onClose} title="New amortisation schedule" size="lg">
      <form onSubmit={submit} className="space-y-4">
        <div className="grid grid-cols-2 gap-3">
          <button type="button" onClick={() => setForm({ ...form, kind: 'prepaid_expense', balance_account: '1400', pnl_account: '' })} className={`p-3 rounded-lg border-2 text-left ${isPrepaid ? 'border-indigo-500 bg-indigo-50' : 'border-gray-200'}`}>
            <p className="font-medium text-sm">Prepaid expense</p>
            <p className="text-xs text-gray-500">Paid upfront, expensed monthly (e.g. annual insurance)</p>
          </button>
          <button type="button" onClick={() => setForm({ ...form, kind: 'deferred_revenue', balance_account: '3450', pnl_account: '' })} className={`p-3 rounded-lg border-2 text-left ${!isPrepaid ? 'border-indigo-500 bg-indigo-50' : 'border-gray-200'}`}>
            <p className="font-medium text-sm">Deferred revenue</p>
            <p className="text-xs text-gray-500">Received upfront, earned monthly (e.g. annual subscription)</p>
          </button>
        </div>
        <div><label className="label">Description</label><input className="input" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="e.g. Annual office insurance 2026" /></div>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="label">{isPrepaid ? 'Prepaid asset account' : 'Deferred revenue account'}</label>
            <select className="input" value={form.balance_account} onChange={(e) => setForm({ ...form, balance_account: e.target.value })}>
              {balanceAccounts.map((a) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
            </select>
          </div>
          <div>
            <label className="label">{isPrepaid ? 'Expense account' : 'Revenue account'}</label>
            <select className="input" value={form.pnl_account} onChange={(e) => setForm({ ...form, pnl_account: e.target.value })}>
              <option value="">Select…</option>
              {pnlAccounts.map((a) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
            </select>
          </div>
        </div>
        <div className="grid grid-cols-3 gap-3">
          <div><label className="label">Total amount</label><input type="number" step="0.01" className="input" value={form.total_amount} onChange={(e) => setForm({ ...form, total_amount: e.target.value })} placeholder="0.00" /></div>
          <div><label className="label">Months</label><input type="number" className="input" value={form.periods} onChange={(e) => setForm({ ...form, periods: e.target.value })} /></div>
          <div><label className="label">Start month</label><input type="date" className="input" value={form.start_date} onChange={(e) => setForm({ ...form, start_date: e.target.value })} /></div>
        </div>
        {monthly > 0 && <p className="text-xs text-gray-500">≈ {formatCurrency(monthly)} per month.</p>}
        {error && <div className="bg-red-50 border border-red-200 rounded-lg p-3 text-sm text-red-700">{error}</div>}
        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>{mutation.isPending ? 'Creating…' : 'Create schedule'}</button>
        </div>
      </form>
    </Modal>
  );
}
