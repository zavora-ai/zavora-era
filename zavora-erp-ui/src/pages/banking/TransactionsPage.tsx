import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getTransactions,
  categoriseTransaction,
  splitTransaction,
  mergeTransactions,
  excludeTransaction,
  getAccounts,
} from '../../api/client';
import type { CategorisationTransaction, Account } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import { ArrowLeftRight, Sparkles, Split, Merge, Ban, Check, Search } from 'lucide-react';

type FilterStatus = 'all' | 'uncategorised' | 'categorised' | 'posted' | 'excluded';

export default function TransactionsPage() {
  const [filter, setFilter] = useState<FilterStatus>('uncategorised');
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [splitTxn, setSplitTxn] = useState<CategorisationTransaction | null>(null);
  const [mergeTxns, setMergeTxns] = useState<CategorisationTransaction[] | null>(null);
  const [manualAssignTxn, setManualAssignTxn] = useState<CategorisationTransaction | null>(null);

  const queryClient = useQueryClient();

  const { data: transactions = [], isLoading } = useQuery<CategorisationTransaction[]>({
    queryKey: ['transactions', filter],
    queryFn: () =>
      getTransactions(filter !== 'all' ? { status: filter } : undefined).then((r) => r.data),
  });

  // Accept AI suggestion mutation
  const acceptMutation = useMutation({
    mutationFn: ({ id, account_code }: { id: string; account_code: string }) =>
      categoriseTransaction(id, { account_code, method: 'ai_suggestion' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['transactions'] });
    },
  });

  // Exclude transaction mutation
  const excludeMutation = useMutation({
    mutationFn: (id: string) => excludeTransaction(id, { reason: 'User excluded' }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['transactions'] });
    },
  });

  const handleAcceptSuggestion = (txn: CategorisationTransaction) => {
    if (txn.suggestion) {
      acceptMutation.mutate({ id: txn.id, account_code: txn.suggestion.account_code });
    }
  };

  // Bulk-accept every AI suggestion currently shown (those with a suggested
  // account that aren't yet categorised). Uses the same per-transaction endpoint.
  const handleAutoCategorise = async () => {
    const withSuggestions = transactions.filter(
      (t) => t.suggestion?.account_code && t.status === 'uncategorised',
    );
    if (withSuggestions.length === 0) {
      alert('No AI suggestions to apply on the current list.');
      return;
    }
    if (!window.confirm(`Apply AI suggestions to ${withSuggestions.length} transaction(s)?`)) return;
    for (const t of withSuggestions) {
      try {
        await acceptMutation.mutateAsync({ id: t.id, account_code: t.suggestion!.account_code });
      } catch {
        /* keep going; failures stay uncategorised for manual handling */
      }
    }
    queryClient.invalidateQueries({ queryKey: ['transactions'] });
  };

  const handleExclude = (txn: CategorisationTransaction) => {
    excludeMutation.mutate(txn.id);
  };

  const handleToggleSelect = (id: string) => {
    setSelectedIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]
    );
  };

  const handleMergeSelected = () => {
    const selected = transactions.filter((t) => selectedIds.includes(t.id));
    if (selected.length >= 2) {
      setMergeTxns(selected);
    }
  };

  const uncategorisedCount = transactions.filter((t) => t.status === 'uncategorised').length;

  return (
    <div>
      <PageHeader
        title="Transaction Queue"
        subtitle="Categorise, split, or merge bank transactions"
      />

      {/* Filter tabs */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex gap-1 border-b border-gray-200">
          {(['uncategorised', 'posted', 'excluded', 'all'] as FilterStatus[]).map((f) => (
            <button
              key={f}
              onClick={() => { setFilter(f); setSelectedIds([]); }}
              className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors capitalize ${
                filter === f
                  ? 'border-blue-600 text-blue-600'
                  : 'border-transparent text-gray-500 hover:text-gray-700'
              }`}
            >
              {f}
            </button>
          ))}
        </div>
        {selectedIds.length >= 2 && (
          <button onClick={handleMergeSelected} className="btn-secondary text-xs inline-flex items-center gap-1">
            <Merge className="w-3 h-3" /> Merge Selected ({selectedIds.length})
          </button>
        )}
      </div>

      {/* Transactions list */}
      <div className="card">
        <div className="px-6 py-3 border-b bg-gray-50 flex items-center justify-between">
          <span className="text-sm font-medium text-gray-700">
            {filter === 'uncategorised'
              ? `Uncategorised (${uncategorisedCount})`
              : `Transactions (${transactions.length})`}
          </span>
          <div className="flex gap-2">
            <button
              className="btn-secondary text-xs inline-flex items-center gap-1"
              onClick={handleAutoCategorise}
              disabled={acceptMutation.isPending}
            >
              <Sparkles className="w-3 h-3" /> Auto-Categorise
            </button>
          </div>
        </div>

        {isLoading ? (
          <div className="p-8 text-center text-gray-400 text-sm">Loading transactions...</div>
        ) : transactions.length === 0 ? (
          <div className="p-8 text-center text-gray-400 text-sm">
            No transactions in this view.
          </div>
        ) : (
          <div className="divide-y">
            {transactions.map((txn) => (
              <div
                key={txn.id}
                className="px-6 py-4 flex items-center justify-between hover:bg-gray-50"
              >
                <div className="flex items-center gap-4">
                  {/* Multi-select checkbox for merge */}
                  {filter === 'uncategorised' && (
                    <input
                      type="checkbox"
                      className="h-4 w-4 rounded border-gray-300 text-blue-600"
                      checked={selectedIds.includes(txn.id)}
                      onChange={() => handleToggleSelect(txn.id)}
                    />
                  )}
                  <div
                    className={`w-2 h-2 rounded-full ${
                      txn.amount > 0 ? 'bg-green-500' : 'bg-red-400'
                    }`}
                  />
                  <div>
                    <p className="text-sm font-medium text-gray-900">{txn.description}</p>
                    <p className="text-xs text-gray-500">
                      {formatDate(txn.date)} · {txn.reference}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-4">
                  {/* AI Suggestion display */}
                  {txn.suggestion && txn.status === 'uncategorised' && (
                    <div className="text-right mr-4">
                      <p className="text-xs text-gray-500">AI Suggestion</p>
                      <p className="text-sm font-medium text-blue-600">
                        {txn.suggestion.account_code} – {txn.suggestion.account_name}{' '}
                        <span
                          className={`inline-block ml-1 text-xs px-1.5 py-0.5 rounded-full font-medium ${
                            txn.suggestion.confidence >= 0.9
                              ? 'bg-green-100 text-green-700'
                              : txn.suggestion.confidence >= 0.7
                                ? 'bg-yellow-100 text-yellow-700'
                                : 'bg-red-100 text-red-700'
                          }`}
                        >
                          {Math.round(txn.suggestion.confidence * 100)}%
                        </span>
                      </p>
                    </div>
                  )}

                  {/* Assigned account for categorised */}
                  {txn.status === 'categorised' && txn.assigned_account_code && (
                    <div className="text-right mr-4">
                      <p className="text-xs text-gray-500">Assigned</p>
                      <p className="text-sm font-medium text-green-600">
                        {txn.assigned_account_code} – {txn.assigned_account_name}
                      </p>
                    </div>
                  )}

                  {/* Amount */}
                  <span
                    className={`text-sm font-medium ${
                      txn.amount > 0 ? 'text-green-600' : 'text-gray-900'
                    }`}
                  >
                    {txn.amount > 0 ? '+' : ''}
                    {formatCurrency(txn.amount, txn.currency || 'KES')}
                  </span>

                  {/* Action buttons for uncategorised */}
                  {txn.status === 'uncategorised' && (
                    <div className="flex gap-1">
                      {txn.suggestion && (
                        <button
                          onClick={() => handleAcceptSuggestion(txn)}
                          disabled={acceptMutation.isPending}
                          className="btn-primary text-xs py-1 px-2 inline-flex items-center gap-1"
                          title="Accept AI suggestion"
                        >
                          <Check className="w-3 h-3" /> Accept
                        </button>
                      )}
                      <button
                        onClick={() => setManualAssignTxn(txn)}
                        className="btn-secondary text-xs py-1 px-2"
                        title="Manually assign account"
                      >
                        <ArrowLeftRight className="w-3 h-3" />
                      </button>
                      <button
                        onClick={() => setSplitTxn(txn)}
                        className="btn-secondary text-xs py-1 px-2"
                        title="Split transaction"
                      >
                        <Split className="w-3 h-3" />
                      </button>
                      <button
                        onClick={() => handleExclude(txn)}
                        disabled={excludeMutation.isPending}
                        className="btn-secondary text-xs py-1 px-2 hover:text-red-600"
                        title="Exclude transaction"
                      >
                        <Ban className="w-3 h-3" />
                      </button>
                    </div>
                  )}

                  {/* Status badge for non-uncategorised */}
                  {txn.status !== 'uncategorised' && (
                    <span
                      className={`text-xs px-2 py-0.5 rounded-full font-medium ${
                        txn.status === 'categorised'
                          ? 'bg-green-100 text-green-700'
                          : txn.status === 'excluded'
                            ? 'bg-gray-100 text-gray-500'
                            : txn.status === 'split'
                              ? 'bg-purple-100 text-purple-700'
                              : 'bg-blue-100 text-blue-700'
                      }`}
                    >
                      {txn.status}
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Modals */}
      {splitTxn && <SplitModal txn={splitTxn} onClose={() => setSplitTxn(null)} />}
      {mergeTxns && <MergeModal txns={mergeTxns} onClose={() => { setMergeTxns(null); setSelectedIds([]); }} />}
      {manualAssignTxn && <ManualAssignModal txn={manualAssignTxn} onClose={() => setManualAssignTxn(null)} />}
    </div>
  );
}


// ─── Split Transaction Modal ───────────────────────────────────────────────────

function SplitModal({ txn, onClose }: { txn: CategorisationTransaction; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [lines, setLines] = useState([
    { amount: Math.abs(txn.amount) / 2, account_code: '', description: '' },
    { amount: Math.abs(txn.amount) / 2, account_code: '', description: '' },
  ]);
  const [error, setError] = useState('');

  const { data: accounts = [] } = useQuery<Account[]>({
    queryKey: ['accounts'],
    queryFn: () => getAccounts().then((r) => r.data),
  });

  const mutation = useMutation({
    mutationFn: (data: any) => splitTransaction(txn.id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['transactions'] });
      onClose();
    },
    onError: (err: any) => {
      setError(err.response?.data?.message || 'Failed to split transaction.');
    },
  });

  const addLine = () => {
    setLines([...lines, { amount: 0, account_code: '', description: '' }]);
  };

  const updateLine = (idx: number, field: string, value: string | number) => {
    const updated = [...lines];
    updated[idx] = { ...updated[idx], [field]: value };
    setLines(updated);
  };

  const removeLine = (idx: number) => {
    if (lines.length <= 2) return;
    setLines(lines.filter((_, i) => i !== idx));
  };

  const totalSplit = lines.reduce((sum, l) => sum + Number(l.amount), 0);
  const isBalanced = Math.abs(totalSplit - Math.abs(txn.amount)) < 0.01;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!isBalanced) {
      setError(`Split amounts must total ${formatCurrency(Math.abs(txn.amount))}. Current total: ${formatCurrency(totalSplit)}.`);
      return;
    }

    const invalidLine = lines.find((l) => !l.account_code || l.amount <= 0);
    if (invalidLine) {
      setError('Each line must have an account and positive amount.');
      return;
    }

    mutation.mutate({ lines });
  };

  return (
    <Modal open={true} onClose={onClose} title="Split Transaction" subtitle={txn.description} size="lg">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="bg-gray-50 rounded-lg p-3 flex justify-between text-sm">
          <span className="text-gray-500">Original Amount</span>
          <span className="font-semibold">{formatCurrency(Math.abs(txn.amount), txn.currency || 'KES')}</span>
        </div>

        <div className="space-y-3">
          {lines.map((line, idx) => (
            <div key={idx} className="grid grid-cols-12 gap-2 items-end">
              <div className="col-span-4">
                <label className="label text-xs">Account</label>
                <select
                  className="input text-sm"
                  value={line.account_code}
                  onChange={(e) => updateLine(idx, 'account_code', e.target.value)}
                  required
                >
                  <option value="">Select account...</option>
                  {accounts.map((a) => (
                    <option key={a.code} value={a.code}>
                      {a.code} – {a.name}
                    </option>
                  ))}
                </select>
              </div>
              <div className="col-span-3">
                <label className="label text-xs">Amount</label>
                <input
                  type="number"
                  step="0.01"
                  min="0.01"
                  className="input text-sm"
                  value={line.amount}
                  onChange={(e) => updateLine(idx, 'amount', +e.target.value)}
                  required
                />
              </div>
              <div className="col-span-4">
                <label className="label text-xs">Description</label>
                <input
                  className="input text-sm"
                  value={line.description}
                  onChange={(e) => updateLine(idx, 'description', e.target.value)}
                  placeholder="Optional"
                />
              </div>
              <div className="col-span-1">
                {lines.length > 2 && (
                  <button
                    type="button"
                    onClick={() => removeLine(idx)}
                    className="text-red-400 hover:text-red-600 text-sm"
                  >
                    ✕
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>

        <div className="flex items-center justify-between">
          <button type="button" onClick={addLine} className="btn-secondary text-xs">
            + Add Line
          </button>
          <span className={`text-sm font-medium ${isBalanced ? 'text-green-600' : 'text-red-600'}`}>
            Total: {formatCurrency(totalSplit)} / {formatCurrency(Math.abs(txn.amount))}
          </span>
        </div>

        {error && (
          <div className="bg-red-50 border border-red-200 rounded-lg p-3 text-sm text-red-700">
            {error}
          </div>
        )}

        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !isBalanced}>
            {mutation.isPending ? 'Splitting...' : 'Split Transaction'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

// ─── Merge Transactions Modal ──────────────────────────────────────────────────

function MergeModal({ txns, onClose }: { txns: CategorisationTransaction[]; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [accountCode, setAccountCode] = useState('');
  const [description, setDescription] = useState('');
  const [error, setError] = useState('');

  const { data: accounts = [] } = useQuery<Account[]>({
    queryKey: ['accounts'],
    queryFn: () => getAccounts().then((r) => r.data),
  });

  const mutation = useMutation({
    mutationFn: (data: any) => mergeTransactions(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['transactions'] });
      onClose();
    },
    onError: (err: any) => {
      setError(err.response?.data?.message || 'Failed to merge transactions.');
    },
  });

  const totalAmount = txns.reduce((sum, t) => sum + t.amount, 0);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!accountCode) {
      setError('Please select an account for the merged transaction.');
      return;
    }

    mutation.mutate({
      transaction_ids: txns.map((t) => t.id),
      account_code: accountCode,
      description: description || txns.map((t) => t.description).join(' + '),
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Merge Transactions" subtitle={`Combining ${txns.length} transactions`}>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* Summary */}
        <div className="bg-gray-50 rounded-lg p-4 space-y-2">
          {txns.map((t) => (
            <div key={t.id} className="flex justify-between text-sm">
              <span className="text-gray-600 truncate max-w-[60%]">{t.description}</span>
              <span className="font-medium">{formatCurrency(t.amount)}</span>
            </div>
          ))}
          <div className="border-t pt-2 flex justify-between text-sm font-semibold">
            <span>Merged Total</span>
            <span>{formatCurrency(totalAmount)}</span>
          </div>
        </div>

        <div>
          <label className="label">Target Account *</label>
          <select
            className="input"
            value={accountCode}
            onChange={(e) => setAccountCode(e.target.value)}
            required
          >
            <option value="">Select account...</option>
            {accounts.map((a) => (
              <option key={a.code} value={a.code}>
                {a.code} – {a.name}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="label">Description</label>
          <input
            className="input"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder={txns.map((t) => t.description).join(' + ')}
          />
        </div>

        {error && (
          <div className="bg-red-50 border border-red-200 rounded-lg p-3 text-sm text-red-700">
            {error}
          </div>
        )}

        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Merging...' : 'Merge Transactions'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

// ─── Manual Assign Modal ───────────────────────────────────────────────────────

function ManualAssignModal({ txn, onClose }: { txn: CategorisationTransaction; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [accountCode, setAccountCode] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [error, setError] = useState('');

  const { data: accounts = [] } = useQuery<Account[]>({
    queryKey: ['accounts'],
    queryFn: () => getAccounts().then((r) => r.data),
  });

  const filteredAccounts = accounts.filter(
    (a) =>
      a.code.toLowerCase().includes(searchQuery.toLowerCase()) ||
      a.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const mutation = useMutation({
    mutationFn: (data: any) => categoriseTransaction(txn.id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['transactions'] });
      onClose();
    },
    onError: (err: any) => {
      setError(err.response?.data?.message || 'Failed to categorise transaction.');
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!accountCode) {
      setError('Please select an account.');
      return;
    }

    mutation.mutate({ account_code: accountCode, method: 'manual' });
  };

  return (
    <Modal open={true} onClose={onClose} title="Assign Account" subtitle={txn.description}>
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="bg-gray-50 rounded-lg p-3 flex justify-between text-sm">
          <span className="text-gray-500">Amount</span>
          <span className="font-semibold">{formatCurrency(txn.amount, txn.currency || 'KES')}</span>
        </div>

        {/* AI suggestion hint */}
        {txn.suggestion && (
          <div className="bg-blue-50 border border-blue-200 rounded-lg p-3 text-sm">
            <p className="text-blue-700">
              <Sparkles className="w-3 h-3 inline mr-1" />
              AI suggests: <strong>{txn.suggestion.account_code} – {txn.suggestion.account_name}</strong>{' '}
              ({Math.round(txn.suggestion.confidence * 100)}% confidence)
            </p>
          </div>
        )}

        <div>
          <label className="label">Search Accounts</label>
          <div className="relative">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
            <input
              className="input pl-9"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search by code or name..."
            />
          </div>
        </div>

        <div>
          <label className="label">Account *</label>
          <select
            className="input"
            value={accountCode}
            onChange={(e) => setAccountCode(e.target.value)}
            required
            size={6}
          >
            {filteredAccounts.map((a) => (
              <option key={a.code} value={a.code}>
                {a.code} – {a.name} ({a.account_type})
              </option>
            ))}
          </select>
        </div>

        {error && (
          <div className="bg-red-50 border border-red-200 rounded-lg p-3 text-sm text-red-700">
            {error}
          </div>
        )}

        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Assigning...' : 'Assign Account'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
