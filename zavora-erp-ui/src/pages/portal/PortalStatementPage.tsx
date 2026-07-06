import { useQuery } from '@tanstack/react-query';
import { getPortalStatement } from '../../api/portalClient';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { Receipt } from 'lucide-react';

interface Bill {
  id: string; number: string; issue_date: string; due_date?: string;
  currency: string; gross_total: string; balance_due: string; status: string;
}

export default function PortalStatementPage() {
  const { data, isLoading } = useQuery({
    queryKey: ['portal-statement'],
    queryFn: () => getPortalStatement().then((r) => r.data),
  });
  const bills: Bill[] = data?.bills ?? [];
  const totalBilled = data?.total_billed ?? 0;
  const totalOutstanding = data?.total_outstanding ?? 0;

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900">Statement</h1>
        <p className="mt-1 text-sm text-gray-500">Every invoice you've lodged and its outstanding balance.</p>
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-2 gap-4 mb-6 sm:max-w-md">
        <div className="bg-white rounded-xl border border-gray-200 p-5">
          <p className="text-xs text-gray-500 uppercase tracking-wide">Total Billed</p>
          <p className="text-2xl font-bold text-gray-900 mt-1">{formatCurrency(totalBilled)}</p>
        </div>
        <div className="bg-white rounded-xl border border-gray-200 p-5">
          <p className="text-xs text-gray-500 uppercase tracking-wide">Outstanding</p>
          <p className="text-2xl font-bold text-emerald-600 mt-1">{formatCurrency(totalOutstanding)}</p>
        </div>
      </div>

      {isLoading ? (
        <p className="text-sm text-gray-500 py-12 text-center">Loading…</p>
      ) : bills.length === 0 ? (
        <div className="bg-white rounded-xl border border-gray-200 p-12 text-center">
          <Receipt className="w-10 h-10 text-gray-300 mx-auto mb-3" />
          <p className="text-gray-500">No invoices lodged yet.</p>
        </div>
      ) : (
        <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
                <th className="text-left px-4 py-3">Invoice #</th>
                <th className="text-left px-4 py-3">Status</th>
                <th className="text-left px-4 py-3">Date</th>
                <th className="text-left px-4 py-3">Due</th>
                <th className="text-right px-4 py-3">Amount</th>
                <th className="text-right px-4 py-3">Balance</th>
              </tr>
            </thead>
            <tbody>
              {bills.map((b) => (
                <tr key={b.id} className="border-b last:border-b-0">
                  <td className="px-4 py-3 font-medium text-blue-600">{b.number}</td>
                  <td className="px-4 py-3"><span className={statusColor(b.status)}>{b.status.replace('_', ' ')}</span></td>
                  <td className="px-4 py-3 text-gray-600">{formatDate(b.issue_date)}</td>
                  <td className="px-4 py-3 text-gray-600">{b.due_date ? formatDate(b.due_date) : '—'}</td>
                  <td className="px-4 py-3 text-right">{formatCurrency(b.gross_total, b.currency)}</td>
                  <td className="px-4 py-3 text-right font-medium">{formatCurrency(b.balance_due, b.currency)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
