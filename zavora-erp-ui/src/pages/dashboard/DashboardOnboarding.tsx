import { useNavigate } from 'react-router-dom';
import { Settings, Users, FileText, CreditCard, Building2, ArrowRight, Sparkles } from 'lucide-react';

// A tenant is "new" until it has any invoice, bill, or payment activity.
export function isNewTenant(summary?: { invoice_count?: number; bill_count?: number; payment_count?: number } | null): boolean {
  if (!summary) return false;
  return (summary.invoice_count ?? 0) === 0
    && (summary.bill_count ?? 0) === 0
    && (summary.payment_count ?? 0) === 0;
}

const steps = [
  { icon: Settings, title: 'Set up your company', desc: 'Add your KRA PIN, branding, and tax settings.', to: '/settings' },
  { icon: Users, title: 'Create your first customer', desc: 'Add the businesses you invoice.', to: '/customers' },
  { icon: FileText, title: 'Send your first invoice', desc: 'Bill a customer and post it to the ledger.', to: '/invoices' },
  { icon: CreditCard, title: 'Record a payment', desc: 'Match a receipt against an open invoice.', to: '/payments' },
  { icon: Building2, title: 'Add a vendor', desc: 'Track suppliers and bills you owe.', to: '/vendors' },
];

export default function DashboardOnboarding() {
  const navigate = useNavigate();

  return (
    <div className="card p-8">
      <div className="flex items-center gap-3 mb-2">
        <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-indigo-500 to-blue-600 flex items-center justify-center">
          <Sparkles className="w-5 h-5 text-white" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-gray-900">Welcome to Zavora ERP</h2>
          <p className="text-sm text-gray-500">Let's get your books set up — follow these steps to start.</p>
        </div>
      </div>

      <div className="mt-6 grid grid-cols-1 md:grid-cols-2 gap-3">
        {steps.map((s, i) => (
          <button
            key={s.to}
            onClick={() => navigate(s.to)}
            className="group flex items-center gap-4 text-left rounded-xl border border-gray-100 p-4 hover:border-indigo-200 hover:bg-indigo-50/40 transition-colors"
          >
            <div className="w-9 h-9 rounded-lg bg-gray-50 group-hover:bg-white flex items-center justify-center shrink-0">
              <s.icon className="w-[18px] h-[18px] text-indigo-600" />
            </div>
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-[11px] font-semibold text-gray-400">STEP {i + 1}</span>
              </div>
              <p className="text-sm font-medium text-gray-900">{s.title}</p>
              <p className="text-xs text-gray-500 truncate">{s.desc}</p>
            </div>
            <ArrowRight className="w-4 h-4 text-gray-300 group-hover:text-indigo-500 transition-colors shrink-0" />
          </button>
        ))}
      </div>

      <p className="mt-6 text-xs text-gray-400">
        Your dashboard charts and KPIs will appear here once you have some activity.
      </p>
    </div>
  );
}
