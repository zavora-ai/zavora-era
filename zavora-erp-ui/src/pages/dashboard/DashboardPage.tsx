import { useQuery } from '@tanstack/react-query';
import { getDashboard } from '../../api/client';
import type { DashboardSummary } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import StatCard from '../../components/shared/StatCard';
import PageHeader from '../../components/shared/PageHeader';
import { SkeletonCard } from '../../components/shared/Skeleton';
import ErrorRetry from '../../components/shared/ErrorRetry';
import WidgetErrorBoundary from '../../components/shared/WidgetErrorBoundary';
import DashboardOnboarding, { isNewTenant } from './DashboardOnboarding';
import { TrendingUp, TrendingDown, Wallet, AlertCircle, FileText, Receipt } from 'lucide-react';
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend } from 'recharts';

export default function DashboardPage() {
  const { data, isLoading, isError, refetch } = useQuery<DashboardSummary>({
    queryKey: ['dashboard'],
    queryFn: () => getDashboard().then(r => r.data),
  });

  if (isLoading) {
    return (
      <div>
        <PageHeader title="Dashboard" subtitle="Financial overview" />
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
          {Array.from({ length: 4 }).map((_, i) => <SkeletonCard key={i} />)}
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <div className="card p-6 lg:col-span-2 animate-pulse"><div className="h-[280px] bg-gray-100 rounded" /></div>
          <div className="card p-6 animate-pulse"><div className="h-[280px] bg-gray-100 rounded" /></div>
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div>
        <PageHeader title="Dashboard" subtitle="Financial overview" />
        <ErrorRetry message="Couldn't load your dashboard." onRetry={() => refetch()} />
      </div>
    );
  }

  // Brand-new tenant with no activity yet: show the guided onboarding checklist
  // instead of empty charts / demo numbers.
  if (isNewTenant(data)) {
    return (
      <div>
        <PageHeader title="Dashboard" subtitle="Financial overview" />
        <DashboardOnboarding />
      </div>
    );
  }

  // Demo data fallback
  const summary: DashboardSummary = data || {
    as_at: new Date().toISOString(),
    total_receivable: 2450000,
    overdue_receivable: 680000,
    overdue_invoice_count: 5,
    total_payable: 1230000,
    overdue_payable: 320000,
    overdue_bill_count: 3,
    cash_and_bank: 4850000,
    net_income_mtd: 890000,
    net_income_prior: 750000,
    revenue_6m: [
      { year: 2026, month: 1, amount: 1200000 },
      { year: 2026, month: 2, amount: 1350000 },
      { year: 2026, month: 3, amount: 980000 },
      { year: 2026, month: 4, amount: 1500000 },
      { year: 2026, month: 5, amount: 1680000 },
      { year: 2026, month: 6, amount: 1420000 },
    ],
    expenses_6m: [
      { year: 2026, month: 1, amount: 850000 },
      { year: 2026, month: 2, amount: 920000 },
      { year: 2026, month: 3, amount: 780000 },
      { year: 2026, month: 4, amount: 1050000 },
      { year: 2026, month: 5, amount: 1100000 },
      { year: 2026, month: 6, amount: 950000 },
    ],
    recent_transactions: [],
    outstanding_invoices: [
      { id: '1', number: 'INV-2026-042', customer_name: 'Safaricom PLC', amount: 580000, balance_due: 580000, due_date: '2026-06-01', is_overdue: true },
      { id: '2', number: 'INV-2026-043', customer_name: 'Kenya Power', amount: 320000, balance_due: 160000, due_date: '2026-06-15', is_overdue: false },
      { id: '3', number: 'INV-2026-044', customer_name: 'Equity Bank', amount: 450000, balance_due: 450000, due_date: '2026-06-20', is_overdue: false },
    ],
    pending_approvals: 4,
    uncategorised_txns: 12,
  };

  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const chartData = summary.revenue_6m.map((r, i) => ({
    month: months[r.month - 1],
    Revenue: r.amount,
    Expenses: summary.expenses_6m[i]?.amount || 0,
  }));

  return (
    <div>
      <PageHeader title="Dashboard" subtitle="Financial overview" />

      {/* KPI Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
        <StatCard
          title="Cash & Bank"
          value={formatCurrency(summary.cash_and_bank)}
          icon={<Wallet className="w-6 h-6" />}
        />
        <StatCard
          title="Accounts Receivable"
          value={formatCurrency(summary.total_receivable)}
          subtitle={`${summary.overdue_invoice_count} overdue`}
          icon={<TrendingUp className="w-6 h-6" />}
        />
        <StatCard
          title="Accounts Payable"
          value={formatCurrency(summary.total_payable)}
          subtitle={`${summary.overdue_bill_count} overdue`}
          icon={<TrendingDown className="w-6 h-6" />}
        />
        <StatCard
          title="Net Income (MTD)"
          value={formatCurrency(summary.net_income_mtd)}
          trend={{
            value: `${Math.round(((summary.net_income_mtd - summary.net_income_prior) / (summary.net_income_prior || 1)) * 100)}% vs prior`,
            positive: summary.net_income_mtd >= summary.net_income_prior,
          }}
          icon={<TrendingUp className="w-6 h-6" />}
        />
      </div>

      {/* Charts + Actions */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 mb-6">
        {/* Revenue vs Expenses Chart */}
        <WidgetErrorBoundary label="The chart" >
        <div className="card p-6 lg:col-span-2">
          <h3 className="text-sm font-medium text-gray-500 mb-4">Revenue vs Expenses (6 months)</h3>
          <ResponsiveContainer width="100%" height={280}>
            <BarChart data={chartData}>
              <CartesianGrid strokeDasharray="3 3" stroke="#f0f0f0" />
              <XAxis dataKey="month" fontSize={12} />
              <YAxis fontSize={12} tickFormatter={(v) => `${(v / 1000000).toFixed(1)}M`} />
              <Tooltip formatter={(v) => formatCurrency(Number(v))} />
              <Legend />
              <Bar dataKey="Revenue" fill="#1a56db" radius={[4, 4, 0, 0]} />
              <Bar dataKey="Expenses" fill="#f59e0b" radius={[4, 4, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
        </WidgetErrorBoundary>

        {/* Quick Actions */}
        <WidgetErrorBoundary label="Needs Attention">
        <div className="card p-6">
          <h3 className="text-sm font-medium text-gray-500 mb-4">Needs Attention</h3>
          <div className="space-y-3">
            {summary.pending_approvals > 0 && (
              <div className="flex items-center gap-3 p-3 bg-yellow-50 rounded-lg">
                <AlertCircle className="w-5 h-5 text-yellow-600 shrink-0" />
                <div>
                  <p className="text-sm font-medium text-yellow-900">{summary.pending_approvals} pending approvals</p>
                  <p className="text-xs text-yellow-700">Bills awaiting review</p>
                </div>
              </div>
            )}
            {summary.uncategorised_txns > 0 && (
              <div className="flex items-center gap-3 p-3 bg-blue-50 rounded-lg">
                <Receipt className="w-5 h-5 text-blue-600 shrink-0" />
                <div>
                  <p className="text-sm font-medium text-blue-900">{summary.uncategorised_txns} uncategorised</p>
                  <p className="text-xs text-blue-700">Bank transactions need review</p>
                </div>
              </div>
            )}
            {summary.overdue_invoice_count > 0 && (
              <div className="flex items-center gap-3 p-3 bg-red-50 rounded-lg">
                <FileText className="w-5 h-5 text-red-600 shrink-0" />
                <div>
                  <p className="text-sm font-medium text-red-900">{formatCurrency(summary.overdue_receivable)} overdue</p>
                  <p className="text-xs text-red-700">{summary.overdue_invoice_count} invoices past due</p>
                </div>
              </div>
            )}
          </div>
        </div>
        </WidgetErrorBoundary>
      </div>

      {/* Outstanding Invoices */}
      <div className="card">
        <div className="px-6 py-4 border-b border-gray-200">
          <h3 className="text-sm font-medium text-gray-900">Outstanding Invoices</h3>
        </div>
        <div className="divide-y divide-gray-200">
          {summary.outstanding_invoices.map((inv) => (
            <div key={inv.id} className="px-6 py-4 flex items-center justify-between">
              <div className="flex items-center gap-4">
                <div>
                  <p className="text-sm font-medium text-gray-900">{inv.number}</p>
                  <p className="text-sm text-gray-500">{inv.customer_name}</p>
                </div>
              </div>
              <div className="text-right">
                <p className="text-sm font-medium text-gray-900">{formatCurrency(inv.balance_due)}</p>
                <p className={`text-xs ${inv.is_overdue ? 'text-red-600 font-medium' : 'text-gray-500'}`}>
                  {inv.is_overdue ? 'OVERDUE' : `Due ${formatDate(inv.due_date)}`}
                </p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
