import { useState } from 'react';
import { useToast } from '../../components/toast/ToastProvider';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getExpenseClaims, getExpenseClaim, createExpenseClaim, submitExpenseClaim,
  approveExpenseClaim, rejectExpenseClaim,
} from '../../api/client';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { usePermissions } from '../../hooks/usePermissions';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, Send, Check, X } from 'lucide-react';

interface Claim { id: string; number: string; title: string; currency: string; total: string; status: string; rejection_reason?: string; created_at: string; }
interface ClaimLine { id: string; expense_date?: string; description: string; account_code?: string; amount: string; }

export default function ExpenseClaimsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [detailId, setDetailId] = useState<string | null>(null);
  const { data: claims = [], isLoading } = useQuery<Claim[]>({
    queryKey: ['expense-claims'], queryFn: () => getExpenseClaims().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });

  const { can } = usePermissions();

  const columns: Column<Claim>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status}</span> },
    { key: 'number', header: 'Claim #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'title', header: 'Title' },
    { key: 'created_at', header: 'Date', render: (r) => formatDate(r.created_at) },
    { key: 'total', header: 'Total', className: 'text-right', render: (r) => <span className="font-medium">{formatCurrency(r.total, r.currency)}</span> },
  ];

  return (
    <div>
      <PageHeader title="Expense Claims" subtitle="Submit out-of-pocket expenses for approval and reimbursement."
        actions={can('expense_claim.create') ? <button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> New Claim</button> : undefined} />
      <DataTable columns={columns} data={claims} loading={isLoading} onRowClick={(r) => setDetailId(r.id)} emptyMessage="No expense claims yet." />
      {showCreate && <CreateClaimModal onClose={() => setShowCreate(false)} />}
      {detailId && <ClaimDetailModal id={detailId} onClose={() => setDetailId(null)} />}
    </div>
  );
}

function CreateClaimModal({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const [title, setTitle] = useState('');
  const [lines, setLines] = useState([{ expense_date: '', description: '', account_code: '', amount: 0 }]);
  const [error, setError] = useState<string | null>(null);
  const total = lines.reduce((s, l) => s + (Number(l.amount) || 0), 0);
  const addLine = () => setLines([...lines, { expense_date: '', description: '', account_code: '', amount: 0 }]);
  const upd = (i: number, f: string, v: any) => { const n = [...lines]; (n[i] as any)[f] = v; setLines(n); };
  const rm = (i: number) => { if (lines.length === 1) return; setLines(lines.filter((_, idx) => idx !== i)); };

  const mut = useMutation({
    mutationFn: () => createExpenseClaim({ title, lines: lines.filter((l) => l.description.trim()).map((l) => ({
      expense_date: l.expense_date || undefined, description: l.description, account_code: l.account_code || undefined, amount: Number(l.amount),
    })) }),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['expense-claims'] }); onClose(); },
    onError: (e: any) => setError(e?.response?.data?.error || 'Could not create the claim.'),
  });
  const submit = () => { setError(null); if (!title.trim()) return setError('Enter a title.'); if (!lines.some((l) => l.description.trim() && Number(l.amount) > 0)) return setError('Add at least one line with an amount.'); mut.mutate(); };

  return (
    <Modal open={true} onClose={onClose} title="New Expense Claim" size="lg">
      <form onSubmit={(e) => { e.preventDefault(); submit(); }} className="space-y-5">
        <div><label className="label">Title *</label><input className="input" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="e.g. Client meeting expenses" required /></div>
        <div>
          <label className="label">Expenses</label>
          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-3">Date</div><div className="col-span-5">Description</div><div className="col-span-2">Account</div><div className="col-span-2 text-right">Amount</div>
            </div>
            {lines.map((l, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-3"><input type="date" className="input text-sm py-1.5" value={l.expense_date} onChange={(e) => upd(i, 'expense_date', e.target.value)} /></div>
                <div className="col-span-5"><input className="input text-sm py-1.5" placeholder="What was it for?" value={l.description} onChange={(e) => upd(i, 'description', e.target.value)} /></div>
                <div className="col-span-2"><input className="input text-sm py-1.5" placeholder="acct" value={l.account_code} onChange={(e) => upd(i, 'account_code', e.target.value)} /></div>
                <div className="col-span-1"><input type="number" min="0" step="0.01" className="input text-sm py-1.5 text-right" value={l.amount} onChange={(e) => upd(i, 'amount', +e.target.value)} /></div>
                <div className="col-span-1 text-center"><button type="button" onClick={() => rm(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={lines.length === 1}>×</button></div>
              </div>
            ))}
          </div>
          <div className="flex justify-between items-center mt-2">
            <button type="button" onClick={addLine} className="text-sm font-medium text-blue-600 hover:text-blue-800">+ Add a Line</button>
            <span className="font-bold">{formatCurrency(total, 'KES')}</span>
          </div>
        </div>
        {error && <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">{error}</div>}
        <div className="flex justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mut.isPending}>{mut.isPending ? 'Saving…' : 'Create Claim'}</button>
        </div>
      </form>
    </Modal>
  );
}

function ClaimDetailModal({ id, onClose }: { id: string; onClose: () => void }) {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ['expense-claim', id], queryFn: () => getExpenseClaim(id).then((r) => r.data) });
  const claim: Claim | undefined = data?.claim;
  const lines: ClaimLine[] = data?.lines ?? [];
  const inv = () => { qc.invalidateQueries({ queryKey: ['expense-claims'] }); qc.invalidateQueries({ queryKey: ['expense-claim', id] }); };
  const toast = useToast();
  const { can } = usePermissions();
  const act = useMutation({ mutationFn: (fn: () => Promise<any>) => fn(), onSuccess: inv, onError: (e: any) => toast.fromError(e, 'Action failed.') });

  if (!claim) return <Modal open={true} onClose={onClose} title="Expense Claim"><p className="text-sm text-gray-500 py-8 text-center">Loading…</p></Modal>;
  return (
    <Modal open={true} onClose={onClose} title={`${claim.number} — ${claim.title}`} size="lg">
      <div className="space-y-4">
        <div className="flex items-center gap-3 text-sm"><span className="text-gray-500">Status</span><span className={statusColor(claim.status)}>{claim.status}</span></div>
        {claim.status === 'rejected' && claim.rejection_reason && <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">Rejected: {claim.rejection_reason}</div>}
        <div className="border rounded-lg overflow-hidden">
          <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase"><div className="col-span-3">Date</div><div className="col-span-7">Description</div><div className="col-span-2 text-right">Amount</div></div>
          {lines.map((l) => (
            <div key={l.id} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 text-sm">
              <div className="col-span-3 text-gray-600">{l.expense_date ? formatDate(l.expense_date) : '—'}</div>
              <div className="col-span-7 text-gray-900">{l.description}</div>
              <div className="col-span-2 text-right font-medium">{formatCurrency(l.amount, claim.currency)}</div>
            </div>
          ))}
        </div>
        <div className="flex justify-end"><span className="text-lg font-bold">{formatCurrency(claim.total, claim.currency)}</span></div>
        <div className="flex items-center justify-between pt-3 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Close</button>
          <div className="flex gap-2">
            {claim.status === 'draft' && can('expense_claim.create') && <button className="btn-primary" disabled={act.isPending} onClick={() => act.mutate(() => submitExpenseClaim(id))}><Send className="w-4 h-4" /> Submit</button>}
            {claim.status === 'submitted' && can('expense_claim.approve') && (<>
              <button className="btn-secondary text-red-600" disabled={act.isPending} onClick={() => { const reason = window.prompt('Reason for rejection?') ?? undefined; act.mutate(() => rejectExpenseClaim(id, reason)); }}><X className="w-4 h-4" /> Reject</button>
              <button className="btn-primary bg-emerald-600 hover:bg-emerald-700" disabled={act.isPending} onClick={() => act.mutate(() => approveExpenseClaim(id))}><Check className="w-4 h-4" /> Approve</button>
            </>)}
          </div>
        </div>
      </div>
    </Modal>
  );
}
