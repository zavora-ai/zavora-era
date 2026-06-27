import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { getDashboard } from '../../api/client';
import type { DashboardSummary } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import StatCard from '../../components/shared/StatCard';
import PageHeader from '../../components/shared/PageHeader';
import { SkeletonCard } from '../../components/shared/Skeleton';
import ErrorRetry from '../../components/shared/ErrorRetry';
import WidgetErrorBoundary from '../../components/shared/WidgetErrorBoundary';
import DashboardOnboarding, { isNewTenant } from './DashboardOnboarding';
import { TrendingUp, TrendingDown, Wallet, AlertCircle, FileText, Receipt, Landmark, ArrowRight } from 'lucide-react';
import {
  BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend,
  PieChart, Pie, Cell,
} from 'recharts';

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const EXPENSE_COLORS = ['#1a56db', '#7e3af2', '#f59e0b', '#0694a2', '#e74694', '#16a34a', '#6366f1', '#9ca3af'];

export default function DashboardPage() {
  const navigate = useNavigate();
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

  // After the loading/error/new-tenant guards above, data is present.
  const s = data!;

  const chartData = s.revenue_6m.map((r, i) => ({
    month: MONTHS[r.month - 1],
    Revenue: r.amount,
    Expenses: s.expenses_6m[i]?.amount || 0,
  }));

  const priorPct = s.net_income_prior
    ? Math.round(((s.net_income_mtd - s.net_income_prior) / Math.abs(s.net_income_prior)) * 100)
    : null;

  return (
    <div>
      <PageHeader title="Dashboard" subtitle="Financial overview" />

      {/* KPI Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
        <StatCard title="Cash & Bank" value={formatCurrency(s.cash_and_bank)} icon={<Wallet className="w-6 h-6" />} onClick={() => navigate('/banking')} />
        <StatCard
          title="Accounts Receivable"
          value={formatCurrency(s.total_receivable)}
          subtitle={`${s.overdue_invoice_count} overdue`}
          icon={<TrendingUp className="w-6 h-6" />}
          onClick={() => navigate('/invoices')}
        />
        <StatCard
          title="Accounts Payable"
          value={formatCurrency(s.total_payable)}
          subtitle={`${s.overdue_bill_count} overdue`}
          icon={<TrendingDown className="w-6 h-6" />}
          onClick={() => navigate('/bills')}
        />
        <StatCard
          title="Net Income (this month)"
          value={formatCurrency(s.net_income_mtd)}
          trend={priorPct !== null ? { value: `${priorPct >= 0 ? '+' : ''}${priorPct}% vs last month`, positive: s.net_income_mtd >= s.net_income_prior } : undefined}
          icon={<TrendingUp className="w-6 h-6" />}
          onClick={() => navigate('/reports')}
        />
      </div>

      {/* Invoices money-bar — QBO signature widget */}
      <WidgetErrorBoundary label="Invoices">
        <InvoicesBarCard summary={s} onOpen={() => navigate('/invoices')} />
      </WidgetErrorBoundary>

      {/* Profit & Loss + Expenses */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 mb-6">
        <WidgetErrorBoundary label="Profit & Loss">
          <div className="card p-6 lg:col-span-2">
            <div className="flex items-baseline justify-between mb-1">
              <h3 className="text-sm font-medium text-gray-500">Profit &amp; Loss</h3>
              <button onClick={() => navigate('/reports')} className="text-xs text-blue-600 hover:underline flex items-center gap-1">
                View report <ArrowRight className="w-3 h-3" />
              </button>
            </div>
            <div className="flex flex-wrap gap-x-8 gap-y-1 mb-4">
              <Metric label="Income (MTD)" value={formatCurrency(s.pnl_mtd.income)} className="text-gray-900" />
              <Metric label="Expenses (MTD)" value={formatCurrency(s.pnl_mtd.expenses)} className="text-gray-900" />
              <Metric label="Net income (MTD)" value={formatCurrency(s.pnl_mtd.net_income)} className={s.pnl_mtd.net_income >= 0 ? 'text-green-600' : 'text-red-600'} />
            </div>
            <ResponsiveContainer width="100%" height={240}>
              <BarChart data={chartData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#f0f0f0" />
                <XAxis dataKey="month" fontSize={12} />
                <YAxis fontSize={12} tickFormatter={(v) => Math.abs(v) >= 1000 ? `${(v / 1000).toFixed(0)}k` : `${v}`} />
                <Tooltip formatter={(v) => formatCurrency(Number(v))} />
                <Legend />
                <Bar dataKey="Revenue" fill="#1a56db" radius={[4, 4, 0, 0]} />
                <Bar dataKey="Expenses" fill="#f59e0b" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </WidgetErrorBoundary>

        <WidgetErrorBoundary label="Expenses">
          <ExpensesCard summary={s} />
        </WidgetErrorBoundary>
      </div>

      {/* Bank accounts + Needs Attention + Outstanding invoices */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <WidgetErrorBoundary label="Bank accounts">
          <div className="card p-6">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-sm font-medium text-gray-500">Bank Accounts</h3>
              <button onClick={() => navigate('/banking')} className="text-xs text-blue-600 hover:underline">Manage</button>
            </div>
            {s.bank_accounts.length === 0 ? (
              <p className="text-sm text-gray-400">No bank accounts yet.</p>
            ) : (
              <div className="space-y-3">
                {s.bank_accounts.map((b) => (
                  <div key={b.id} className="flex items-center justify-between">
                    <div className="flex items-center gap-2 min-w-0">
                      <Landmark className="w-4 h-4 text-gray-400 shrink-0" />
                      <div className="min-w-0">
                        <p className="text-sm font-medium text-gray-900 truncate">{b.name}</p>
                        <p className="text-xs text-gray-500 truncate">{b.bank_name}</p>
                      </div>
                    </div>
                    <p className={`text-sm font-medium ${b.balance < 0 ? 'text-red-600' : 'text-gray-900'}`}>{formatCurrency(b.balance)}</p>
                  </div>
                ))}
                <div className="flex items-center justify-between border-t pt-3 mt-1">
                  <span className="text-xs font-medium text-gray-500">Cash &amp; bank total</span>
                  <span className="text-sm font-semibold text-gray-900">{formatCurrency(s.cash_and_bank)}</span>
                </div>
              </div>
            )}
          </div>
        </WidgetErrorBoundary>

        <WidgetErrorBoundary label="Needs Attention">
          <div className="card p-6">
            <h3 className="text-sm font-medium text-gray-500 mb-4">Needs Attention</h3>
            <div className="space-y-3">
              {s.pending_approvals > 0 && (
                <button onClick={() => navigate('/bills')} className="w-full flex items-center gap-3 p-3 bg-yellow-50 rounded-lg hover:bg-yellow-100 transition-colors text-left">
                  <AlertCircle className="w-5 h-5 text-yellow-600 shrink-0" />
                  <div>
                    <p className="text-sm font-medium text-yellow-900">{s.pending_approvals} pending approvals</p>
                    <p className="text-xs text-yellow-700">Bills awaiting review</p>
                  </div>
                </button>
              )}
              {s.uncategorised_txns > 0 && (
                <button onClick={() => navigate('/transactions')} className="w-full flex items-center gap-3 p-3 bg-blue-50 rounded-lg hover:bg-blue-100 transition-colors text-left">
                  <Receipt className="w-5 h-5 text-blue-600 shrink-0" />
                  <div>
                    <p className="text-sm font-medium text-blue-900">{s.uncategorised_txns} uncategorised</p>
                    <p className="text-xs text-blue-700">Bank transactions need review</p>
                  </div>
                </button>
              )}
              {s.overdue_invoice_count > 0 && (
                <button onClick={() => navigate('/invoices')} className="w-full flex items-center gap-3 p-3 bg-red-50 rounded-lg hover:bg-red-100 transition-colors text-left">
                  <FileText className="w-5 h-5 text-red-600 shrink-0" />
                  <div>
                    <p className="text-sm font-medium text-red-900">{formatCurrency(s.overdue_receivable)} overdue</p>
                    <p className="text-xs text-red-700">{s.overdue_invoice_count} invoices past due</p>
                  </div>
                </button>
              )}
              {s.pending_approvals === 0 && s.uncategorised_txns === 0 && s.overdue_invoice_count === 0 && (
                <p className="text-sm text-gray-400">You're all caught up. 🎉</p>
              )}
            </div>
          </div>
        </WidgetErrorBoundary>

        <WidgetErrorBoundary label="Outstanding invoices">
          <div className="card overflow-hidden">
            <div className="px-6 py-4 border-b border-gray-200 flex items-center justify-between">
              <h3 className="text-sm font-medium text-gray-900">Outstanding Invoices</h3>
              <button onClick={() => navigate('/invoices')} className="text-xs text-blue-600 hover:underline">View all</button>
            </div>
            {s.outstanding_invoices.length === 0 ? (
              <p className="px-6 py-6 text-sm text-gray-400">No outstanding invoices.</p>
            ) : (
              <div className="divide-y divide-gray-200 max-h-[260px] overflow-y-auto">
                {s.outstanding_invoices.map((inv) => (
                  <button
                    key={inv.id}
                    onClick={() => navigate(`/invoices/${inv.id}`)}
                    className="w-full px-6 py-3 flex items-center justify-between hover:bg-gray-50 transition-colors text-left"
                  >
                    <div className="min-w-0">
                      <p className="text-sm font-medium text-gray-900 truncate">{inv.number}</p>
                      <p className="text-sm text-gray-500 truncate">{inv.customer_name || '—'}</p>
                    </div>
                    <div className="text-right shrink-0 ml-3">
                      <p className="text-sm font-medium text-gray-900">{formatCurrency(inv.balance_due)}</p>
                      <p className={`text-xs ${inv.is_overdue ? 'text-red-600 font-medium' : 'text-gray-500'}`}>
                        {inv.is_overdue ? 'OVERDUE' : `Due ${formatDate(inv.due_date)}`}
                      </p>
                    </div>
                  </button>
                ))}
              </div>
            )}
          </div>
        </WidgetErrorBoundary>
      </div>
    </div>
  );
}

function Metric({ label, value, className = '' }: { label: string; value: string; className?: string }) {
  return (
    <div>
      <p className="text-xs text-gray-500">{label}</p>
      <p className={`text-lg font-semibold ${className}`}>{value}</p>
    </div>
  );
}

function InvoicesBarCard({ summary: s, onOpen }: { summary: DashboardSummary; onOpen: () => void }) {
  const bar = s.invoices_bar;
  const total = bar.overdue + bar.due_soon + bar.open;
  const seg = (v: number) => (total > 0 ? `${(v / total) * 100}%` : '0%');

  return (
    <div className="card p-6 mb-6">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-medium text-gray-500">Invoices</h3>
        <button onClick={onOpen} className="text-xs text-blue-600 hover:underline flex items-center gap-1">
          Manage invoices <ArrowRight className="w-3 h-3" />
        </button>
      </div>

      <div className="flex flex-col lg:flex-row lg:items-center gap-6">
        <div className="flex-1">
          {/* Segmented bar */}
          <div className="flex h-3 rounded-full overflow-hidden bg-gray-100 mb-3">
            <div style={{ width: seg(bar.overdue) }} className="bg-red-500" title="Overdue" />
            <div style={{ width: seg(bar.due_soon) }} className="bg-amber-400" title="Due within 30 days" />
            <div style={{ width: seg(bar.open) }} className="bg-blue-500" title="Open" />
          </div>
          <div className="grid grid-cols-3 gap-4">
            <BarLegend color="bg-red-500" label="Overdue" amount={formatCurrency(bar.overdue)} sub={`${bar.overdue_count} invoice${bar.overdue_count === 1 ? '' : 's'}`} />
            <BarLegend color="bg-amber-400" label="Due within 30 days" amount={formatCurrency(bar.due_soon)} sub={`${bar.due_soon_count} invoice${bar.due_soon_count === 1 ? '' : 's'}`} />
            <BarLegend color="bg-blue-500" label="Open (later)" amount={formatCurrency(bar.open)} sub={`${bar.open_count} invoice${bar.open_count === 1 ? '' : 's'}`} />
          </div>
        </div>
        <div className="lg:w-48 lg:border-l lg:pl-6">
          <p className="text-xs text-gray-500">Paid (last 30 days)</p>
          <p className="text-2xl font-bold text-green-600">{formatCurrency(bar.paid_last_30)}</p>
        </div>
      </div>
    </div>
  );
}

function BarLegend({ color, label, amount, sub }: { color: string; label: string; amount: string; sub: string }) {
  return (
    <div>
      <div className="flex items-center gap-1.5 mb-0.5">
        <span className={`w-2.5 h-2.5 rounded-full ${color}`} />
        <span className="text-xs text-gray-500">{label}</span>
      </div>
      <p className="text-base font-semibold text-gray-900">{amount}</p>
      <p className="text-xs text-gray-400">{sub}</p>
    </div>
  );
}

function ExpensesCard({ summary: s }: { summary: DashboardSummary }) {
  const data = s.expense_breakdown.map((e) => ({ name: e.name, value: Number(e.amount) }));
  const total = data.reduce((acc, d) => acc + d.value, 0);

  return (
    <div className="card p-6">
      <h3 className="text-sm font-medium text-gray-500 mb-1">Expenses (this month)</h3>
      {data.length === 0 ? (
        <p className="text-sm text-gray-400 mt-8 text-center">No expenses recorded this month.</p>
      ) : (
        <>
          <div className="relative">
            <ResponsiveContainer width="100%" height={180}>
              <PieChart>
                <Pie data={data} dataKey="value" nameKey="name" innerRadius={55} outerRadius={80} paddingAngle={2}>
                  {data.map((_, i) => <Cell key={i} fill={EXPENSE_COLORS[i % EXPENSE_COLORS.length]} />)}
                </Pie>
                <Tooltip formatter={(v) => formatCurrency(Number(v))} />
              </PieChart>
            </ResponsiveContainer>
            <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none">
              <span className="text-xs text-gray-400">Total</span>
              <span className="text-lg font-semibold text-gray-900">{formatCurrency(total)}</span>
            </div>
          </div>
          <div className="space-y-1.5 mt-3">
            {s.expense_breakdown.slice(0, 5).map((e, i) => (
              <div key={e.code} className="flex items-center justify-between text-sm">
                <span className="flex items-center gap-2 min-w-0">
                  <span className="w-2.5 h-2.5 rounded-full shrink-0" style={{ background: EXPENSE_COLORS[i % EXPENSE_COLORS.length] }} />
                  <span className="text-gray-600 truncate">{e.name}</span>
                </span>
                <span className="font-medium text-gray-900 shrink-0 ml-2">{formatCurrency(e.amount)}</span>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
