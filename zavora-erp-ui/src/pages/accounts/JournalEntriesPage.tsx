import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getJournalEntries, createJournalEntry, getAccounts, reverseJournalEntry } from '../../api/client';
import type { JournalEntry, Account } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { hasRole, ROLES_POST } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import PaginationControls from '../../components/shared/PaginationControls';
import { usePagination } from '../../hooks/usePagination';
import Modal from '../../components/shared/Modal';
import { Plus, BookOpen, AlertCircle, RotateCcw } from 'lucide-react';

export default function JournalEntriesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [reverseTarget, setReverseTarget] = useState<JournalEntry | null>(null);
  const [notice, setNotice] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  const { page, limit, offset, setPage } = usePagination();
  const { data: resp, isLoading } = useQuery({
    queryKey: ['journal-entries', offset, limit],
    queryFn: () => getJournalEntries({ limit, offset }).then(r => r.data),
  });
  const entries: JournalEntry[] = resp?.data ?? [];
  const total: number = resp?.total_count ?? 0;

  const columns: Column<JournalEntry>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status}</span> },
    { key: 'number', header: 'Entry #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'date', header: 'Date', render: (r) => formatDate(r.date) },
    { key: 'reference', header: 'Reference', render: (r) => r.reference || '—' },
    { key: 'description', header: 'Description', render: (r) => <span className="text-gray-900">{r.description}</span> },
    { key: 'source', header: 'Source', render: (r) => <span className="badge-info text-xs">{r.source}</span> },
    { key: 'posted_at', header: 'Posted', render: (r) => r.posted_at ? formatDate(r.posted_at) : '—' },
    {
      key: 'actions', header: '',
      render: (r) => (
        <div className="flex items-center justify-end gap-1">
          {r.status === 'posted' && hasRole(ROLES_POST) && (
            <button
              onClick={() => { setNotice(null); setReverseTarget(r); }}
              className="btn-secondary text-xs py-1 px-2"
              title="Reverse this entry"
            >
              <RotateCcw className="w-3 h-3" /> Reverse
            </button>
          )}
        </div>
      ),
    },
  ];

  return (
    <div>
      <PageHeader
        title="Journal Entries"
        subtitle="Manual journal entries — double-entry bookkeeping"
        actions={
          hasRole(ROLES_POST) ? (
            <button onClick={() => setShowCreate(true)} className="btn-primary">
              <Plus className="w-4 h-4" /> New Journal Entry
            </button>
          ) : undefined
        }
      />
      {notice && (
        <div className={`mb-4 flex items-center gap-2 p-3 rounded-lg text-sm ${notice.type === 'success' ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'}`}>
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span>{notice.message}</span>
        </div>
      )}
      <DataTable columns={columns} data={entries} loading={isLoading} emptyMessage="No journal entries yet. Create a manual entry to record adjustments." />
      <PaginationControls page={page} limit={limit} total={total} onPage={setPage} />
      {showCreate && <CreateJournalEntryModal onClose={() => setShowCreate(false)} />}
      {reverseTarget && (
        <ReverseJournalEntryModal
          entry={reverseTarget}
          onClose={() => setReverseTarget(null)}
          onResult={setNotice}
        />
      )}
    </div>
  );
}

function ReverseJournalEntryModal({
  entry,
  onClose,
  onResult,
}: {
  entry: JournalEntry;
  onClose: () => void;
  onResult: (notice: { type: 'success' | 'error'; message: string }) => void;
}) {
  const queryClient = useQueryClient();
  const [reason, setReason] = useState('');

  const mutation = useMutation({
    mutationFn: () => reverseJournalEntry(entry.id, { reason: reason.trim() || undefined }),
    onSuccess: (resp) => {
      queryClient.invalidateQueries({ queryKey: ['journal-entries'] });
      const number = resp?.data?.reversing_number;
      onResult({
        type: 'success',
        message: number
          ? `Entry ${entry.number} reversed — created reversing entry ${number}.`
          : `Entry ${entry.number} reversed.`,
      });
      onClose();
    },
    onError: (e: any) => {
      onResult({
        type: 'error',
        message: e?.response?.data?.error || e?.response?.data?.message || 'Failed to reverse entry.',
      });
      onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate();
  };

  return (
    <Modal open={true} onClose={onClose} title={`Reverse ${entry.number}`} subtitle="Creates an offsetting reversing entry" size="sm">
      <form onSubmit={handleSubmit} className="space-y-5">
        <div className="flex items-start gap-2 p-3 rounded-lg bg-amber-50 text-amber-700 text-sm">
          <RotateCcw className="w-4 h-4 shrink-0 mt-0.5" />
          <span>This will post a new entry that reverses the debits and credits of {entry.number}. The original entry stays on record.</span>
        </div>
        <div>
          <label className="label">Reason <span className="text-gray-400 font-normal">(optional)</span></label>
          <textarea
            className="input"
            rows={3}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            placeholder="Why is this entry being reversed?"
          />
        </div>
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Reversing...' : 'Reverse Entry'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

interface JournalLine {
  account_code: string;
  description: string;
  debit: number;
  credit: number;
}

function CreateJournalEntryModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data: accounts = [] } = useQuery<Account[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then(r => r.data) });

  const today = new Date().toISOString().split('T')[0];

  const [form, setForm] = useState({
    date: today,
    reference: '',
    description: '',
    lines: [emptyLine(), emptyLine()] as JournalLine[],
  });

  function emptyLine(): JournalLine {
    return { account_code: '', description: '', debit: 0, credit: 0 };
  }

  const mutation = useMutation({
    mutationFn: (data: any) => createJournalEntry(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['journal-entries'] });
      onClose();
    },
  });

  const addLine = () => setForm({ ...form, lines: [...form.lines, emptyLine()] });

  const updateLine = (i: number, field: keyof JournalLine, value: any) => {
    const lines = [...form.lines];
    (lines[i] as any)[field] = value;
    setForm({ ...form, lines });
  };

  const removeLine = (i: number) => {
    if (form.lines.length <= 2) return;
    setForm({ ...form, lines: form.lines.filter((_, idx) => idx !== i) });
  };

  // Balance check
  const totalDebits = form.lines.reduce((sum, l) => sum + (l.debit || 0), 0);
  const totalCredits = form.lines.reduce((sum, l) => sum + (l.credit || 0), 0);
  const isBalanced = Math.abs(totalDebits - totalCredits) < 0.01 && totalDebits > 0;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!isBalanced) return;

    mutation.mutate({
      date: form.date,
      reference: form.reference || undefined,
      description: form.description,
      lines: form.lines
        .filter(l => l.account_code && (l.debit > 0 || l.credit > 0))
        .map(l => ({
          account_code: l.account_code,
          description: l.description || undefined,
          debit: l.debit || 0,
          credit: l.credit || 0,
        })),
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="New Journal Entry" subtitle="Debits must equal credits" size="xl">
      <form onSubmit={handleSubmit} className="space-y-6">
        {/* Header */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div>
            <label className="label">Date *</label>
            <input type="date" className="input" value={form.date} onChange={(e) => setForm({ ...form, date: e.target.value })} required />
          </div>
          <div>
            <label className="label">Reference</label>
            <input className="input" value={form.reference} onChange={(e) => setForm({ ...form, reference: e.target.value })} placeholder="e.g. ADJ-001" />
          </div>
          <div>
            <label className="label">Description *</label>
            <input className="input" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="Reason for this entry" required />
          </div>
        </div>

        {/* Lines */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="label mb-0">Entry Lines</label>
          </div>
          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-3">Account</div>
              <div className="col-span-4">Description</div>
              <div className="col-span-2 text-right">Debit (KES)</div>
              <div className="col-span-2 text-right">Credit (KES)</div>
              <div className="col-span-1"></div>
            </div>
            {form.lines.map((line, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-3">
                  <select className="input text-sm py-1.5" value={line.account_code} onChange={(e) => updateLine(i, 'account_code', e.target.value)}>
                    <option value="">Select account...</option>
                    {accounts.map(a => <option key={a.id} value={a.code}>{a.code} — {a.name}</option>)}
                  </select>
                </div>
                <div className="col-span-4">
                  <input className="input text-sm py-1.5" placeholder="Line description" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} />
                </div>
                <div className="col-span-2">
                  <input
                    className="input text-sm py-1.5 text-right"
                    type="number"
                    min="0"
                    step="0.01"
                    value={line.debit || ''}
                    onChange={(e) => updateLine(i, 'debit', +e.target.value)}
                    placeholder="0.00"
                    disabled={line.credit > 0}
                  />
                </div>
                <div className="col-span-2">
                  <input
                    className="input text-sm py-1.5 text-right"
                    type="number"
                    min="0"
                    step="0.01"
                    value={line.credit || ''}
                    onChange={(e) => updateLine(i, 'credit', +e.target.value)}
                    placeholder="0.00"
                    disabled={line.debit > 0}
                  />
                </div>
                <div className="col-span-1 text-center">
                  <button type="button" onClick={() => removeLine(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={form.lines.length <= 2}>×</button>
                </div>
              </div>
            ))}
            {/* Totals row */}
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-t font-medium text-sm">
              <div className="col-span-3"></div>
              <div className="col-span-4 text-right text-gray-600">Totals:</div>
              <div className="col-span-2 text-right">{formatCurrency(totalDebits)}</div>
              <div className="col-span-2 text-right">{formatCurrency(totalCredits)}</div>
              <div className="col-span-1"></div>
            </div>
          </div>
          <button type="button" onClick={addLine} className="mt-2 text-sm font-medium text-blue-600 hover:text-blue-800">
            + Add Line
          </button>
        </div>

        {/* Balance indicator */}
        {totalDebits > 0 && (
          <div className={`flex items-center gap-2 p-3 rounded-lg text-sm ${isBalanced ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'}`}>
            {isBalanced ? (
              <>
                <BookOpen className="w-4 h-4" />
                <span>Entry is balanced — debits equal credits ({formatCurrency(totalDebits)})</span>
              </>
            ) : (
              <>
                <AlertCircle className="w-4 h-4" />
                <span>Entry is not balanced — difference of {formatCurrency(Math.abs(totalDebits - totalCredits))}</span>
              </>
            )}
          </div>
        )}

        {/* Footer actions */}
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button
            type="submit"
            className="btn-primary"
            disabled={mutation.isPending || !isBalanced || !form.description}
          >
            {mutation.isPending ? 'Saving...' : 'Post Entry'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
