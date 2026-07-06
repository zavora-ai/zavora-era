import { useQuery } from '@tanstack/react-query';
import { getProcurementAnalytics, getBudgetControl } from '../../api/client';
import { formatCurrency, formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';

interface SpendRow { vendor: string; ordered: string; billed: string; }
interface CommitmentRow { number: string; vendor: string; currency: string; gross_total: string; issue_date: string; status: string; }
interface BudgetRow { account_code: string; account_name: string; budget: string; actual: string; committed: string; available: string; over_budget: boolean; }

function CountPills({ title, counts }: { title: string; counts: Record<string, number> }) {
  const entries = Object.entries(counts || {});
  return (
    <div className="bg-white rounded-xl border border-gray-200 p-4">
      <p className="text-xs font-medium text-gray-500 uppercase mb-2">{title}</p>
      {entries.length === 0 ? <p className="text-sm text-gray-400">None yet</p> : (
        <div className="flex flex-wrap gap-2">
          {entries.map(([s, c]) => (
            <span key={s} className="inline-flex items-center gap-1.5 rounded-full bg-gray-100 px-2.5 py-1 text-xs">
              <span className="text-gray-600">{s.replace('_', ' ')}</span>
              <span className="font-semibold text-gray-900">{c}</span>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

export default function ProcurementAnalyticsPage() {
  const { data, isLoading } = useQuery({ queryKey: ['procurement-analytics'], queryFn: () => getProcurementAnalytics().then((r) => r.data) });
  const { data: budgetData } = useQuery({ queryKey: ['budget-control'], queryFn: () => getBudgetControl().then((r) => r.data) });
  const spend: SpendRow[] = data?.spend_by_vendor ?? [];
  const commitments: CommitmentRow[] = data?.open_commitments ?? [];
  const counts = data?.counts ?? {};
  const budgets: BudgetRow[] = budgetData?.accounts ?? [];

  return (
    <div>
      <PageHeader title="Procurement Analytics" subtitle="Spend by vendor, open commitments (ordered but not yet invoiced), and the procurement pipeline." />

      {isLoading ? (
        <p className="text-sm text-gray-500 py-12 text-center">Loading…</p>
      ) : (
        <div className="space-y-6">
          {/* KPI: committed total */}
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <div className="bg-white rounded-xl border border-gray-200 p-4">
              <p className="text-xs font-medium text-gray-500 uppercase">Open commitments</p>
              <p className="text-2xl font-bold text-gray-900 mt-1">{formatCurrency(data?.committed_total ?? 0, 'KES')}</p>
              <p className="text-xs text-gray-500 mt-1">{commitments.length} PO(s) issued, not yet invoiced</p>
            </div>
            <CountPills title="Requisitions" counts={counts.requisitions} />
            <CountPills title="Purchase Orders" counts={counts.purchase_orders} />
          </div>

          {/* Spend by vendor */}
          <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
            <div className="px-4 py-3 border-b"><h3 className="font-semibold text-gray-900">Spend by vendor</h3></div>
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
                  <th className="text-left px-4 py-2">Vendor</th>
                  <th className="text-right px-4 py-2">Ordered (POs)</th>
                  <th className="text-right px-4 py-2">Billed</th>
                  <th className="text-right px-4 py-2">Uninvoiced</th>
                </tr>
              </thead>
              <tbody>
                {spend.length === 0 ? (
                  <tr><td colSpan={4} className="px-4 py-8 text-center text-gray-400">No procurement spend yet.</td></tr>
                ) : spend.map((r) => {
                  const uninv = Number(r.ordered) - Number(r.billed);
                  return (
                    <tr key={r.vendor} className="border-b last:border-b-0">
                      <td className="px-4 py-2 text-gray-900">{r.vendor}</td>
                      <td className="px-4 py-2 text-right">{formatCurrency(r.ordered, 'KES')}</td>
                      <td className="px-4 py-2 text-right">{formatCurrency(r.billed, 'KES')}</td>
                      <td className={`px-4 py-2 text-right ${uninv > 0 ? 'text-amber-600' : 'text-gray-400'}`}>{formatCurrency(uninv, 'KES')}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {/* Open commitments register */}
          <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
            <div className="px-4 py-3 border-b"><h3 className="font-semibold text-gray-900">Open commitment register</h3></div>
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
                  <th className="text-left px-4 py-2">LPO #</th>
                  <th className="text-left px-4 py-2">Vendor</th>
                  <th className="text-left px-4 py-2">Issued</th>
                  <th className="text-left px-4 py-2">Status</th>
                  <th className="text-right px-4 py-2">Committed</th>
                </tr>
              </thead>
              <tbody>
                {commitments.length === 0 ? (
                  <tr><td colSpan={5} className="px-4 py-8 text-center text-gray-400">No open commitments.</td></tr>
                ) : commitments.map((c) => (
                  <tr key={c.number} className="border-b last:border-b-0">
                    <td className="px-4 py-2 font-medium text-blue-600">{c.number}</td>
                    <td className="px-4 py-2 text-gray-900">{c.vendor}</td>
                    <td className="px-4 py-2 text-gray-600">{formatDate(c.issue_date)}</td>
                    <td className="px-4 py-2 text-gray-600">{c.status.replace('_', ' ')}</td>
                    <td className="px-4 py-2 text-right font-medium">{formatCurrency(c.gross_total, c.currency)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Budget control (encumbrance): budget vs committed vs actual */}
          <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
            <div className="px-4 py-3 border-b">
              <h3 className="font-semibold text-gray-900">Budget control</h3>
              <p className="text-xs text-gray-500 mt-0.5">Committed = open POs charged to the account. Available = budget − actual − committed.</p>
            </div>
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
                  <th className="text-left px-4 py-2">Account</th>
                  <th className="text-right px-4 py-2">Budget</th>
                  <th className="text-right px-4 py-2">Actual</th>
                  <th className="text-right px-4 py-2">Committed</th>
                  <th className="text-right px-4 py-2">Available</th>
                </tr>
              </thead>
              <tbody>
                {budgets.length === 0 ? (
                  <tr><td colSpan={5} className="px-4 py-8 text-center text-gray-400">No budgets or open commitments. Set budgets under Reports → Budgets.</td></tr>
                ) : budgets.map((b) => (
                  <tr key={b.account_code} className={`border-b last:border-b-0 ${b.over_budget ? 'bg-red-50' : ''}`}>
                    <td className="px-4 py-2 text-gray-900">{b.account_code}{b.account_name ? ` · ${b.account_name}` : ''}</td>
                    <td className="px-4 py-2 text-right">{formatCurrency(b.budget, 'KES')}</td>
                    <td className="px-4 py-2 text-right">{formatCurrency(b.actual, 'KES')}</td>
                    <td className="px-4 py-2 text-right text-amber-600">{formatCurrency(b.committed, 'KES')}</td>
                    <td className={`px-4 py-2 text-right font-medium ${b.over_budget ? 'text-red-600' : 'text-emerald-600'}`}>{formatCurrency(b.available, 'KES')}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
