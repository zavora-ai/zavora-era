import { useQuery } from '@tanstack/react-query';
import { getPortalBids } from '../../api/portalClient';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { FileText } from 'lucide-react';

interface Bid {
  id: string; tender_id: string; currency: string; total_amount: string;
  notes?: string; status: string; submitted_at: string;
}

export default function PortalBidsPage() {
  const { data: bids = [], isLoading } = useQuery<Bid[]>({
    queryKey: ['portal-bids'],
    queryFn: () => getPortalBids().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900">My Bids</h1>
        <p className="mt-1 text-sm text-gray-500">Bids you've submitted and their status. Awarded bids become purchase orders.</p>
      </div>

      {isLoading ? (
        <p className="text-sm text-gray-500 py-12 text-center">Loading…</p>
      ) : bids.length === 0 ? (
        <div className="bg-white rounded-xl border border-gray-200 p-12 text-center">
          <FileText className="w-10 h-10 text-gray-300 mx-auto mb-3" />
          <p className="text-gray-500">You haven't submitted any bids yet.</p>
        </div>
      ) : (
        <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
                <th className="text-left px-4 py-3">Status</th>
                <th className="text-left px-4 py-3">Submitted</th>
                <th className="text-right px-4 py-3">Amount</th>
                <th className="text-left px-4 py-3">Notes</th>
              </tr>
            </thead>
            <tbody>
              {bids.map((b) => (
                <tr key={b.id} className="border-b last:border-b-0">
                  <td className="px-4 py-3"><span className={statusColor(b.status)}>{b.status}</span></td>
                  <td className="px-4 py-3 text-gray-600">{formatDate(b.submitted_at)}</td>
                  <td className="px-4 py-3 text-right font-medium">{formatCurrency(b.total_amount, b.currency)}</td>
                  <td className="px-4 py-3 text-gray-500">{b.notes || '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
