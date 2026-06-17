import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import PageHeader from '../../components/shared/PageHeader';
import StatCard from '../../components/shared/StatCard';
import { Landmark, ArrowLeftRight, CheckCircle2, AlertTriangle, Plus, Trash2, Wifi, WifiOff, X } from 'lucide-react';
import { getBankAccounts, createBankAccount, deleteBankAccount } from '../../api/client';
import type { BankAccount } from '../../types';

export default function BankingPage() {
  const [showCreate, setShowCreate] = useState(false);
  const queryClient = useQueryClient();

  const { data: bankAccounts = [], isLoading } = useQuery<BankAccount[]>({
    queryKey: ['bank-accounts'],
    queryFn: () => getBankAccounts().then(r => r.data),
  });

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

      {/* Actions */}
      <div className="flex justify-end mb-4">
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

      {/* Reconciliation summary */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
        <StatCard title="Matched Transactions" value="—" icon={<CheckCircle2 className="w-5 h-5" />} />
        <StatCard title="Pending Categorisation" value="—" icon={<ArrowLeftRight className="w-5 h-5" />} />
        <StatCard title="Discrepancies" value="—" icon={<AlertTriangle className="w-5 h-5" />} />
      </div>

      {/* Reconciliation features */}
      <div className="card p-6">
        <h3 className="font-medium mb-4">Bank Reconciliation</h3>
        <p className="text-sm text-gray-500 mb-4">
          Three-pass matching algorithm: Exact match → Near match (2-day window) → AI suggestion.
          Import statements in MT940, OFX, or CSV format.
        </p>
        <div className="flex gap-3">
          <button className="btn-primary">Import Statement</button>
          <button className="btn-secondary">Run Auto-Match</button>
        </div>
      </div>

      {/* Create modal */}
      {showCreate && <CreateBankAccountModal onClose={() => setShowCreate(false)} />}
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
