import PageHeader from '../../components/shared/PageHeader';
import { ArrowLeftRight, Sparkles, Split, Merge } from 'lucide-react';

export default function TransactionsPage() {
  // Demo uncategorised transactions
  const transactions = [
    { id: '1', date: '2026-06-05', description: 'MPESA - SAFARICOM PLC', amount: -35000, reference: 'REC45892', suggestion: { account: '7200', name: 'Utilities', confidence: 0.92 } },
    { id: '2', date: '2026-06-04', description: 'BANK TRANSFER - EQUITY', amount: 580000, reference: 'FT89234', suggestion: null },
    { id: '3', date: '2026-06-03', description: 'MPESA - JOHN KAMAU', amount: -50000, reference: 'REC45701', suggestion: { account: '7100', name: 'Rent Expense', confidence: 0.87 } },
    { id: '4', date: '2026-06-02', description: 'CARD - NAIVAS SUPERMARKET', amount: -12500, reference: 'CARD0082', suggestion: { account: '7300', name: 'Office Supplies', confidence: 0.78 } },
  ];

  return (
    <div>
      <PageHeader title="Transaction Queue" subtitle="Categorise, split, or merge bank transactions" />

      <div className="card">
        <div className="px-6 py-3 border-b bg-gray-50 flex items-center justify-between">
          <span className="text-sm font-medium text-gray-700">Uncategorised ({transactions.length})</span>
          <div className="flex gap-2">
            <button className="btn-secondary text-xs"><Sparkles className="w-3 h-3" /> Auto-Categorise</button>
          </div>
        </div>
        <div className="divide-y">
          {transactions.map((txn) => (
            <div key={txn.id} className="px-6 py-4 flex items-center justify-between hover:bg-gray-50">
              <div className="flex items-center gap-4">
                <div className={`w-2 h-2 rounded-full ${txn.amount > 0 ? 'bg-green-500' : 'bg-red-400'}`} />
                <div>
                  <p className="text-sm font-medium text-gray-900">{txn.description}</p>
                  <p className="text-xs text-gray-500">{txn.date} · {txn.reference}</p>
                </div>
              </div>
              <div className="flex items-center gap-4">
                {txn.suggestion && (
                  <div className="text-right mr-4">
                    <p className="text-xs text-gray-500">AI Suggestion</p>
                    <p className="text-sm font-medium text-blue-600">{txn.suggestion.name} ({Math.round(txn.suggestion.confidence * 100)}%)</p>
                  </div>
                )}
                <span className={`text-sm font-medium ${txn.amount > 0 ? 'text-green-600' : 'text-gray-900'}`}>
                  {txn.amount > 0 ? '+' : ''}{new Intl.NumberFormat('en-KE').format(txn.amount)} KES
                </span>
                <div className="flex gap-1">
                  {txn.suggestion && <button className="btn-primary text-xs py-1 px-2">Accept</button>}
                  <button className="btn-secondary text-xs py-1 px-2"><Split className="w-3 h-3" /></button>
                  <button className="btn-secondary text-xs py-1 px-2"><Merge className="w-3 h-3" /></button>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
