import { useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Settings, Users, FileText, CreditCard, Building2, ArrowRight, Sparkles, Check } from 'lucide-react';
import { getCustomers, getVendors, getSettings } from '../../api/client';

interface Summary {
  invoice_count?: number;
  bill_count?: number;
  payment_count?: number;
}

// A tenant is "new" until it has any invoice, bill, or payment activity.
export function isNewTenant(summary?: Summary | null): boolean {
  if (!summary) return false;
  return (summary.invoice_count ?? 0) === 0
    && (summary.bill_count ?? 0) === 0
    && (summary.payment_count ?? 0) === 0;
}

export default function DashboardOnboarding({ summary, hideWhenComplete = false }: { summary?: Summary | null; hideWhenComplete?: boolean }) {
  const navigate = useNavigate();

  // Cheap reads to detect which setup steps are already done. The onboarding
  // panel only renders for low-activity tenants, so these lists are tiny.
  const { data: customers = [] } = useQuery<any[]>({
    queryKey: ['customers'],
    queryFn: () => getCustomers().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });
  const { data: vendors = [] } = useQuery<any[]>({
    queryKey: ['vendors'],
    queryFn: () => getVendors().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });
  const { data: settings } = useQuery<any>({
    queryKey: ['settings'],
    queryFn: () => getSettings().then((r) => r.data),
  });

  // A company is "set up" once branding carries more than the default — i.e. the
  // owner has set a company name (≠ the placeholder) or a KRA PIN/VAT number.
  const branding = settings?.branding ?? {};
  const companyConfigured = Boolean(
    (branding.company_name && branding.company_name !== 'My Company') ||
    branding.kra_pin ||
    branding.vat_number,
  );

  const steps = [
    { icon: Settings, title: 'Set up your company', desc: 'Add your KRA PIN, branding, and tax settings.', to: '/settings', done: companyConfigured },
    { icon: Users, title: 'Create your first customer', desc: 'Add the businesses you invoice.', to: '/customers', done: customers.length > 0 },
    { icon: FileText, title: 'Send your first invoice', desc: 'Bill a customer and post it to the ledger.', to: '/invoices', done: (summary?.invoice_count ?? 0) > 0 },
    { icon: CreditCard, title: 'Record a payment', desc: 'Match a receipt against an open invoice.', to: '/payments', done: (summary?.payment_count ?? 0) > 0 },
    { icon: Building2, title: 'Add a vendor', desc: 'Track suppliers and bills you owe.', to: '/vendors', done: vendors.length > 0 },
  ];

  const completed = steps.filter((s) => s.done).length;
  const pct = Math.round((completed / steps.length) * 100);
  // The first not-yet-done step is the one we nudge the user toward.
  const nextIndex = steps.findIndex((s) => !s.done);

  // When mounted above the full dashboard, collapse once every step is done so
  // it stops taking space; the new-tenant view keeps it (hideWhenComplete=false).
  if (hideWhenComplete && completed === steps.length) {
    return null;
  }

  return (
    <div className="card p-8 mb-6">
      <div className="flex items-center gap-3 mb-2">
        <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-indigo-500 to-blue-600 flex items-center justify-center">
          <Sparkles className="w-5 h-5 text-white" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-gray-900">Welcome to Zavora ERP</h2>
          <p className="text-sm text-gray-500">Let's get your books set up — follow these steps to start.</p>
        </div>
      </div>

      {/* Progress bar */}
      <div className="mt-5">
        <div className="flex items-center justify-between mb-1.5">
          <span className="text-xs font-medium text-gray-500">{completed} of {steps.length} done</span>
          <span className="text-xs font-semibold text-indigo-600">{pct}%</span>
        </div>
        <div className="h-2 rounded-full bg-gray-100 overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-indigo-500 to-blue-600 transition-all duration-500"
            style={{ width: `${pct}%` }}
          />
        </div>
      </div>

      <div className="mt-6 grid grid-cols-1 md:grid-cols-2 gap-3">
        {steps.map((s, i) => {
          const isNext = i === nextIndex;
          return (
            <button
              key={s.to}
              onClick={() => navigate(s.to)}
              className={`group flex items-center gap-4 text-left rounded-xl border p-4 transition-colors ${
                s.done
                  ? 'border-green-100 bg-green-50/40'
                  : isNext
                    ? 'border-indigo-300 bg-indigo-50/50 ring-1 ring-indigo-100'
                    : 'border-gray-100 hover:border-indigo-200 hover:bg-indigo-50/40'
              }`}
            >
              <div
                className={`w-9 h-9 rounded-lg flex items-center justify-center shrink-0 ${
                  s.done ? 'bg-green-100' : 'bg-gray-50 group-hover:bg-white'
                }`}
              >
                {s.done ? (
                  <Check className="w-[18px] h-[18px] text-green-600" />
                ) : (
                  <s.icon className="w-[18px] h-[18px] text-indigo-600" />
                )}
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className={`text-[11px] font-semibold ${s.done ? 'text-green-600' : 'text-gray-400'}`}>
                    {s.done ? 'DONE' : `STEP ${i + 1}`}
                  </span>
                  {isNext && <span className="text-[10px] font-semibold text-indigo-600 bg-indigo-100 px-1.5 py-0.5 rounded">NEXT</span>}
                </div>
                <p className={`text-sm font-medium ${s.done ? 'text-gray-500 line-through' : 'text-gray-900'}`}>{s.title}</p>
                <p className="text-xs text-gray-500 truncate">{s.desc}</p>
              </div>
              <ArrowRight className="w-4 h-4 text-gray-300 group-hover:text-indigo-500 transition-colors shrink-0" />
            </button>
          );
        })}
      </div>

      <p className="mt-6 text-xs text-gray-400">
        {completed === steps.length
          ? 'All set! Your dashboard charts and KPIs are ready.'
          : 'Your dashboard charts and KPIs will appear here once you have some activity.'}
      </p>
    </div>
  );
}
