import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import PageHeader from '../../components/shared/PageHeader';
import StatCard from '../../components/shared/StatCard';
import { Landmark, ArrowLeftRight, CheckCircle2, AlertTriangle, Plus, Trash2, Wifi, WifiOff, X } from 'lucide-react';
import { getBankAccounts, createBankAccount, deleteBankAccount, importStatement, getTransactions, generateReport } from '../../api/client';
import { formatCurrency, formatDate } from '../../utils/format';
import type { BankAccount } from '../../types';

const TODAY = new Date().toISOString().split('T')[0];

export default function BankingPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  const { data: bankAccounts = [], isLoading } = useQuery<BankAccount[]>({
    queryKey: ['bank-accounts'],
    queryFn: () => getBankAccounts().then(r => Array.isArray(r.data) ? r.data : []),
  });

  // Trial Balance gives every account's closing balance; we map each bank
  // account's gl_account to its balance (closing_debit − closing_credit).
  const { data: tbLines = [] } = useQuery<any[]>({
    queryKey: ['trial-balance', 'banking'],
    queryFn: () =>
      generateReport({ entity_id: '00000000-0000-0000-0000-000000000000', report_type: 'TrialBalance', parameters: { as_at: TODAY } })
        .then(r => {
          const tb = r.data?.TrialBalance ?? r.data?.content?.TrialBalance ?? r.data;
          return Array.isArray(tb?.lines) ? tb.lines : [];
        }),
  });
  const balanceFor = (glCode: string): number => {
    const row = tbLines.find((l) => l.account_code === glCode);
    if (!row) return 0;
    return Number(row.closing_debit || 0) - Number(row.closing_credit || 0);
  };
  const totalCash = bankAccounts.reduce((sum, ba) => sum + balanceFor(ba.gl_account), 0);

  // Recent bank transactions: GL detail for each bank account's gl_account,
  // merged and sorted by date (most recent first).
  const { data: recentTxns = [] } = useQuery<any[]>({
    queryKey: ['bank-gl-detail', bankAccounts.map(b => b.gl_account).join(',')],
    enabled: bankAccounts.length > 0,
    queryFn: async () => {
      const all: any[] = [];
      for (const ba of bankAccounts) {
        try {
          const r = await generateReport({
            entity_id: '00000000-0000-0000-0000-000000000000',
            report_type: 'GlDetail',
            parameters: { as_at: TODAY, account_code: ba.gl_account },
          });
          const g = r.data?.GlDetail ?? r.data?.content?.GlDetail ?? r.data;
          for (const ln of (Array.isArray(g?.lines) ? g.lines : [])) {
            all.push({ ...ln, account_name: ba.name, gl_account: ba.gl_account });
          }
        } catch { /* skip accounts with no ledger */ }
      }
      all.sort((a, b) => (a.date < b.date ? 1 : a.date > b.date ? -1 : 0));
      return all.slice(0, 25);
    },
  });

  // Categorisation queue — drives the reconciliation summary cards.
  const { data: txns = [] } = useQuery<any[]>({
    queryKey: ['transactions', 'all'],
    queryFn: () => getTransactions({ limit: 500 }).then(r => {
      const d = r.data;
      return Array.isArray(d) ? d : (Array.isArray(d?.data) ? d.data : []);
    }),
  });
  const matchedCount = txns.filter(t => t.status === 'categorised').length;
  const pendingCount = txns.filter(t => t.status === 'uncategorised').length;
  const excludedCount = txns.filter(t => t.status === 'excluded').length;

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteBankAccount(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bank-accounts'] });
    },
  });

  const handleDelete = (id: string, name: string) => {
    if (window.confirm(`Are you sure you want to delete "${name}"?`)) {
      deleteMutation.mutate(id);
    }
  };

  const formatLastSync = (lastSync: string | null | undefined) => {
    if (!lastSync) return 'Never synced';
    const date = new Date(lastSync);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    if (diffMin < 1) return 'Just now';
    if (diffMin < 60) return `${diffMin} min ago`;
    const diffHrs = Math.floor(diffMin / 60);
    if (diffHrs < 24) return `${diffHrs} hour${diffHrs > 1 ? 's' : ''} ago`;
    const diffDays = Math.floor(diffHrs / 24);
    return `${diffDays} day${diffDays > 1 ? 's' : ''} ago`;
  };

  return (
    <div>
      <PageHeader title="Banking" subtitle="Bank accounts, feeds, and reconciliation" />

      {/* Total cash across all accounts */}
      <div className="flex items-center justify-between mb-4">
        <div className="text-sm text-gray-500">
          Total balance across {bankAccounts.length} account{bankAccounts.length === 1 ? '' : 's'}:{' '}
          <span className="font-semibold text-gray-900">{formatCurrency(totalCash)}</span>
        </div>
        <button className="btn-primary flex items-center gap-2" onClick={() => setShowCreate(true)}>
          <Plus className="w-4 h-4" /> Add Bank Account
        </button>
      </div>

      {/* Bank accounts grid */}
      {isLoading ? (
        <div className="text-center py-8 text-gray-500">Loading bank accounts…</div>
      ) : bankAccounts.length === 0 ? (
        <div className="card p-8 text-center mb-6">
          <Landmark className="w-10 h-10 text-gray-300 mx-auto mb-3" />
          <p className="text-gray-500 mb-2">No bank accounts connected yet</p>
          <button className="btn-primary" onClick={() => setShowCreate(true)}>Add your first account</button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          {bankAccounts.map((ba) => (
            <div key={ba.id} className="card p-5 hover:border-blue-300 transition-colors relative group">
              <div className="flex items-start justify-between mb-3">
                <div className="p-2 bg-blue-50 rounded-lg">
                  <Landmark className="w-5 h-5 text-blue-600" />
                </div>
                <div className="flex items-center gap-2">
                  {/* Feed status indicator */}
                  <span className="flex items-center gap-1" title={ba.feed_enabled ? 'Feed connected' : 'Feed not connected'}>
                    {ba.feed_enabled ? (
                      <Wifi className="w-3.5 h-3.5 text-green-500" />
                    ) : (
                      <WifiOff className="w-3.5 h-3.5 text-gray-400" />
                    )}
                  </span>
                  <span className="text-xs text-gray-400">
                    {formatLastSync(ba.last_sync)}
                  </span>
                </div>
              </div>
              <p className="font-medium text-gray-900">{ba.name}</p>
              <p className="text-xs text-gray-500 mb-1">{ba.bank_name}</p>
              <p className="text-xs text-gray-400 mb-2">
                ••••{ba.account_number.slice(-4)} · {ba.currency}
              </p>
              {/* Account balance from the GL */}
              <p className="text-xl font-bold text-gray-900 mb-2">{formatCurrency(balanceFor(ba.gl_account), ba.currency)}</p>
              {/* Feed status badge */}
              <div className="flex items-center justify-between">
                <span className={`text-xs px-2 py-0.5 rounded-full ${ba.feed_enabled ? 'bg-green-50 text-green-700' : 'bg-gray-100 text-gray-500'}`}>
                  {ba.feed_enabled ? 'Feed active' : 'Manual'}
                </span>
                <button
                  className="opacity-0 group-hover:opacity-100 transition-opacity p-1 text-red-400 hover:text-red-600 rounded"
                  onClick={() => handleDelete(ba.id, ba.name)}
                  title="Delete account"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Recent bank transactions (across all accounts) */}
      {recentTxns.length > 0 && (
        <div className="card overflow-hidden mb-6">
          <div className="px-5 py-3 border-b bg-gray-50">
            <h3 className="text-sm font-medium text-gray-700">Recent Bank Transactions</h3>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b text-xs font-medium text-gray-500 uppercase">
                  <th className="px-5 py-3 text-left">Date</th>
                  <th className="px-5 py-3 text-left">Account</th>
                  <th className="px-5 py-3 text-left">Description</th>
                  <th className="px-5 py-3 text-left">Reference</th>
                  <th className="px-5 py-3 text-right">Money In</th>
                  <th className="px-5 py-3 text-right">Money Out</th>
                  <th className="px-5 py-3 text-right">Balance</th>
                </tr>
              </thead>
              <tbody className="divide-y">
                {recentTxns.map((t, i) => {
                  const dr = Number(t.debit || 0), cr = Number(t.credit || 0);
                  return (
                    <tr key={t.entry_id ? `${t.entry_id}-${i}` : i}>
                      <td className="px-5 py-3 text-sm text-gray-600">{formatDate(t.date)}</td>
                      <td className="px-5 py-3 text-sm text-gray-600">{t.account_name}</td>
                      <td className="px-5 py-3 text-sm text-gray-900">{t.description || t.reference || '—'}</td>
                      <td className="px-5 py-3 text-sm text-gray-400">{t.journal_number || t.reference || ''}</td>
                      <td className="px-5 py-3 text-sm text-right text-green-600">{dr > 0 ? formatCurrency(dr) : ''}</td>
                      <td className="px-5 py-3 text-sm text-right text-red-600">{cr > 0 ? formatCurrency(cr) : ''}</td>
                      <td className="px-5 py-3 text-sm text-right font-medium">{formatCurrency(Number(t.balance || 0))}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Reconciliation summary */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
        <StatCard
          title="Matched Transactions"
          value={String(matchedCount)}
          subtitle="Categorised"
          icon={<CheckCircle2 className="w-5 h-5" />}
          onClick={() => navigate('/transactions')}
        />
        <StatCard
          title="Pending Categorisation"
          value={String(pendingCount)}
          subtitle={pendingCount > 0 ? 'Needs review' : 'All caught up'}
          icon={<ArrowLeftRight className="w-5 h-5" />}
          onClick={() => navigate('/transactions')}
        />
        <StatCard
          title="Excluded"
          value={String(excludedCount)}
          subtitle="Not for the books"
          icon={<AlertTriangle className="w-5 h-5" />}
          onClick={() => navigate('/transactions')}
        />
      </div>

      {/* Reconciliation features */}
      <div className="card p-6">
        <h3 className="font-medium mb-4">Bank Reconciliation</h3>
        <p className="text-sm text-gray-500 mb-4">
          Three-pass matching algorithm: Exact match → Near match (2-day window) → AI suggestion.
          Import statements in MT940, OFX, or CSV format.
        </p>
        <div className="flex gap-3">
          <button className="btn-primary" onClick={() => setShowImport(true)} disabled={bankAccounts.length === 0}>
            Import Statement
          </button>
          <button className="btn-secondary" onClick={() => navigate('/reconciliation')}>
            Run Auto-Match
          </button>
        </div>
        {bankAccounts.length === 0 && (
          <p className="text-xs text-gray-400 mt-2">Add a bank account first to import statements.</p>
        )}
      </div>

      {/* Create modal */}
      {showCreate && <CreateBankAccountModal onClose={() => setShowCreate(false)} />}
      {showImport && <ImportStatementModal bankAccounts={bankAccounts} onClose={() => setShowImport(false)} />}
    </div>
  );
}

function CreateBankAccountModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    name: '',
    bank_name: '',
    account_number: '',
    currency: 'KES',
  });
  const [error, setError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: (data: typeof form) => createBankAccount(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bank-accounts'] });
      onClose();
    },
    onError: (err: any) => {
      setError(err.response?.data?.error || 'Failed to create bank account');
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!form.name.trim() || !form.bank_name.trim() || !form.account_number.trim()) {
      setError('All fields are required');
      return;
    }
    mutation.mutate(form);
  };

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
      <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">Add Bank Account</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600">
            <X className="w-5 h-5" />
          </button>
        </div>

        {error && (
          <div className="bg-red-50 text-red-700 text-sm p-3 rounded-lg mb-4">{error}</div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Account Name</label>
            <input
              type="text"
              className="input w-full"
              placeholder="e.g. KCB Business Account"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Bank / Institution</label>
            <input
              type="text"
              className="input w-full"
              placeholder="e.g. KCB, Equity, Safaricom"
              value={form.bank_name}
              onChange={(e) => setForm({ ...form, bank_name: e.target.value })}
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Account Number</label>
            <input
              type="text"
              className="input w-full"
              placeholder="e.g. 1234567890"
              value={form.account_number}
              onChange={(e) => setForm({ ...form, account_number: e.target.value })}
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Currency</label>
            <select
              className="input w-full"
              value={form.currency}
              onChange={(e) => setForm({ ...form, currency: e.target.value })}
            >
              <option value="KES">KES - Kenya Shilling</option>
              <option value="USD">USD - US Dollar</option>
              <option value="EUR">EUR - Euro</option>
              <option value="GBP">GBP - British Pound</option>
              <option value="TZS">TZS - Tanzania Shilling</option>
              <option value="UGX">UGX - Uganda Shilling</option>
            </select>
          </div>

          <div className="flex justify-end gap-3 pt-2">
            <button type="button" className="btn-secondary" onClick={onClose}>Cancel</button>
            <button type="submit" className="btn-primary" disabled={mutation.isPending}>
              {mutation.isPending ? 'Creating…' : 'Create Account'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function ImportStatementModal({
  bankAccounts,
  onClose,
}: {
  bankAccounts: BankAccount[];
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const [bankAccountId, setBankAccountId] = useState(bankAccounts[0]?.id ?? '');
  const [filename, setFilename] = useState('');
  const [content, setContent] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{ line_count: number; format: string } | null>(null);

  const mutation = useMutation({
    mutationFn: () =>
      importStatement({ bank_account_id: bankAccountId, filename: filename || 'statement.csv', content }),
    onSuccess: (res: any) => {
      setError(null);
      setResult({ line_count: res?.data?.line_count ?? 0, format: res?.data?.format ?? 'CSV' });
      queryClient.invalidateQueries({ queryKey: ['transactions'] });
      queryClient.invalidateQueries({ queryKey: ['bank-accounts'] });
    },
    onError: (err: any) => {
      setResult(null);
      setError(err.response?.data?.error || 'Failed to import statement');
    },
  });

  const onFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setFilename(file.name);
    const reader = new FileReader();
    reader.onload = () => setContent(String(reader.result ?? ''));
    reader.readAsText(file);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!bankAccountId) {
      setError('Select a bank account');
      return;
    }
    if (!content.trim()) {
      setError('Upload a file or paste statement content');
      return;
    }
    mutation.mutate();
  };

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
      <div className="bg-white rounded-xl shadow-xl w-full max-w-lg p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">Import Bank Statement</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600">
            <X className="w-5 h-5" />
          </button>
        </div>

        {error && <div className="bg-red-50 text-red-700 text-sm p-3 rounded-lg mb-4">{error}</div>}
        {result && (
          <div className="bg-green-50 text-green-700 text-sm p-3 rounded-lg mb-4">
            Imported {result.line_count} transaction{result.line_count === 1 ? '' : 's'} ({result.format}) into the
            categorisation queue. <button className="underline" onClick={onClose}>Done</button>
          </div>
        )}

        {!result && (
          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Bank Account</label>
              <select
                className="input w-full"
                value={bankAccountId}
                onChange={(e) => setBankAccountId(e.target.value)}
              >
                {bankAccounts.map((ba) => (
                  <option key={ba.id} value={ba.id}>
                    {ba.name} — {ba.bank_name} ({ba.currency})
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">Statement file (CSV / MT940 / OFX)</label>
              <input type="file" accept=".csv,.mt940,.sta,.940,.ofx,.qfx,text/csv" className="input w-full" onChange={onFile} />
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">…or paste content</label>
              <textarea
                className="input w-full font-mono text-xs"
                rows={6}
                placeholder={'Date,Description,Debit,Credit,Balance\n2026-06-01,Customer deposit,,1000.00,1000.00\n2026-06-02,Bank charge,50.00,,950.00'}
                value={content}
                onChange={(e) => setContent(e.target.value)}
              />
              <p className="text-xs text-gray-400 mt-1">
                CSV columns are positional: <code>date, description, amount[, balance]</code> or{' '}
                <code>date, description, debit, credit, balance</code>. Re-importing the same file is blocked.
              </p>
            </div>

            <div className="flex justify-end gap-3 pt-2">
              <button type="button" className="btn-secondary" onClick={onClose}>Cancel</button>
              <button type="submit" className="btn-primary" disabled={mutation.isPending}>
                {mutation.isPending ? 'Importing…' : 'Import'}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
