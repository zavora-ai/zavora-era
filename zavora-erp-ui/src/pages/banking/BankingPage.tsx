import PageHeader from '../../components/shared/PageHeader';
import StatCard from '../../components/shared/StatCard';
import { Landmark, ArrowLeftRight, CheckCircle2, AlertTriangle } from 'lucide-react';
import { formatCurrency } from '../../utils/format';

export default function BankingPage() {
  // Demo data — in production fetched from API
  const bankAccounts = [
    { id: '1', name: 'KCB Business Account', bank: 'KCB', balance: 3250000, currency: 'KES', lastSync: '2 hours ago' },
    { id: '2', name: 'Equity Savings', bank: 'Equity', balance: 1200000, currency: 'KES', lastSync: '1 day ago' },
    { id: '3', name: 'M-Pesa Paybill', bank: 'Safaricom', balance: 450000, currency: 'KES', lastSync: '5 min ago' },
  ];

  return (
    <div>
      <PageHeader title="Banking" subtitle="Bank accounts, feeds, and reconciliation" />

      {/* Bank accounts grid */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
        {bankAccounts.map((ba) => (
          <div key={ba.id} className="card p-5 hover:border-blue-300 cursor-pointer transition-colors">
            <div className="flex items-start justify-between mb-3">
              <div className="p-2 bg-blue-50 rounded-lg"><Landmark className="w-5 h-5 text-blue-600" /></div>
              <span className="text-xs text-gray-400">Synced {ba.lastSync}</span>
            </div>
            <p className="font-medium text-gray-900">{ba.name}</p>
            <p className="text-xs text-gray-500 mb-2">{ba.bank}</p>
            <p className="text-xl font-bold">{formatCurrency(ba.balance, ba.currency)}</p>
          </div>
        ))}
      </div>

      {/* Reconciliation summary */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
        <StatCard title="Matched Transactions" value="156" icon={<CheckCircle2 className="w-5 h-5" />} />
        <StatCard title="Pending Categorisation" value="12" icon={<ArrowLeftRight className="w-5 h-5" />} />
        <StatCard title="Discrepancies" value="2" icon={<AlertTriangle className="w-5 h-5" />} />
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
    </div>
  );
}
