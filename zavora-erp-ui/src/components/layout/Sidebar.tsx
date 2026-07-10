import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard, FileText, Receipt, CreditCard, Users, Building2,
  Package, Landmark, Wallet, BarChart3, Settings, BookOpen, Calculator,
  ArrowLeftRight, ClipboardList, UserCheck, BookMarked, Boxes, Building,
  RefreshCw, History, Camera, UserCog, CalendarClock, FileMinus, Target, Layers, Network, Percent, Scale, Upload, CheckCircle, FileCheck, BellRing, Sparkles,
  Gavel, ShoppingCart, UserPlus, Shield, X
} from 'lucide-react';
import clsx from 'clsx';
import { usePermissions } from '../../hooks/usePermissions';

// Primary read-permission for a nav destination, keyed by href. Items absent
// here are always shown (safe default — the backend still enforces on every
// request; this only trims what a role can't use so people stop clicking into
// 403s). Keys match the RBAC catalog in core `rbac/mod.rs`.
const PERM_BY_HREF: Record<string, string> = {
  '/invoices': 'invoice.read',
  '/estimates': 'estimate.read',
  '/customers': 'customer.read',
  '/bills': 'bill.read',
  '/debit-notes': 'debit_note.read',
  '/expense-claims': 'expense_claim.read',
  '/purchase-orders': 'purchase_order.read',
  '/crm': 'crm.read',
  '/payments': 'payment.read',
  '/banking': 'bank_account.read',
  '/reconciliation': 'reconciliation.read',
  '/transactions': 'bank_transaction.read',
  '/products': 'product.read',
  '/inventory': 'inventory.read',
  '/pos/sessions': 'pos_session.read',
  '/employees': 'employee.read',
  '/onboarding': 'onboarding.read',
  '/leave': 'leave.read',
  '/payroll': 'pay_run.read',
  '/payroll-settings': 'payroll_config.read',
  '/payroll-reports': 'pay_run.read',
  '/accounts': 'account.read',
  '/journal-entries': 'journal.read',
  '/recurring-journals': 'journal.read',
  '/assets': 'asset.read',
  '/amortization': 'journal.read',
  '/periods': 'period.read',
  '/budgets': 'budget.read',
  '/dimensions': 'dimension.read',
  '/etims': 'etims.read',
  '/tax-filings': 'tax_filing.read',
  '/approval-limits': 'approval_limit.read',
  '/users': 'user.manage',
  '/roles-admin': 'role.read',
  '/fx-rates': 'fx_rate.read',
  '/audit': 'audit.read',
};

export const navigation = [
  { name: 'Dashboard', href: '/', icon: LayoutDashboard },

  { divider: true, label: 'SALES' },
  { name: 'Invoices', href: '/invoices', icon: FileText },
  { name: 'Estimates', href: '/estimates', icon: ClipboardList },
  { name: 'Recurring Invoices', href: '/recurring-invoices', icon: RefreshCw },
  { name: 'Customers', href: '/customers', icon: Users },

  { divider: true, label: 'PURCHASES' },
  { name: 'Bills', href: '/bills', icon: Receipt },
  { name: 'Supplier Credits', href: '/supplier-credit-notes', icon: FileMinus },
  { name: 'Debit Notes', href: '/debit-notes', icon: FileMinus },
  { name: 'Expense Claims', href: '/expense-claims', icon: Wallet },
  { name: 'Capture Receipt', href: '/receipts/capture', icon: Camera },
  { name: 'Vendors', href: '/vendors', icon: Building2 },

  { divider: true, label: 'PROCUREMENT' },
  { name: 'Requisitions', href: '/requisitions', icon: ClipboardList },
  { name: 'Tenders', href: '/tenders', icon: Gavel },
  { name: 'Purchase Orders', href: '/purchase-orders', icon: ShoppingCart },
  { name: 'Analytics', href: '/procurement-analytics', icon: BarChart3 },
  { name: 'Vendor Applications', href: '/vendor-applications', icon: UserPlus },

  { divider: true, label: 'CRM' },
  { name: 'CRM', href: '/crm', icon: Target },

  { divider: true, label: 'BANKING' },
  { name: 'Payments', href: '/payments', icon: CreditCard },
  { name: 'Banking', href: '/banking', icon: Landmark },
  { name: 'Reconciliation', href: '/reconciliation', icon: CheckCircle },
  { name: 'Cash Forecast', href: '/cash-forecast', icon: BarChart3 },
  { name: 'Transactions', href: '/transactions', icon: ArrowLeftRight },

  { divider: true, label: 'PRODUCTS & INVENTORY' },
  { name: 'Products', href: '/products', icon: Package },
  { name: 'Inventory', href: '/inventory', icon: Boxes },
  { name: 'Point of Sale', href: '/pos', icon: ShoppingCart },
  { name: 'Till Sessions', href: '/pos/sessions', icon: Landmark },
  { name: 'Stock (Mobile)', href: '/pos/stock', icon: Camera },

  { divider: true, label: 'PAYROLL & HR' },
  { name: 'Employees', href: '/employees', icon: UserCheck },
  { name: 'Onboarding', href: '/onboarding', icon: UserPlus },
  { name: 'Leave', href: '/leave', icon: CalendarClock },
  { name: 'Payroll', href: '/payroll', icon: Wallet },
  { name: 'Payroll Settings', href: '/payroll-settings', icon: Settings },
  { name: 'Payroll Reports', href: '/payroll-reports', icon: BarChart3 },

  { divider: true, label: 'ACCOUNTING' },
  { name: 'Chart of Accounts', href: '/accounts', icon: BookOpen },
  { name: 'Journal Entries', href: '/journal-entries', icon: BookMarked },
  { name: 'Recurring Journals', href: '/recurring-journals', icon: RefreshCw },
  { name: 'Fixed Assets', href: '/assets', icon: Building },
  { name: 'Amortisation', href: '/amortization', icon: RefreshCw },
  { name: 'Opening Balances', href: '/opening-balances', icon: Scale },
  { name: 'Periods', href: '/periods', icon: CalendarClock },

  { divider: true, label: 'REPORTS & ANALYSIS' },
  { name: 'Reports', href: '/reports', icon: BarChart3 },
  { name: 'Budgets', href: '/budgets', icon: Target },
  { name: 'Dimensions', href: '/dimensions', icon: Layers },
  { name: 'Consolidation', href: '/consolidation', icon: Network },

  { divider: true, label: 'TAX & COMPLIANCE' },
  { name: 'KRA eTIMS', href: '/etims', icon: Receipt },
  { name: 'Tax Filing', href: '/tax-filings', icon: FileCheck },
  { name: 'WHT Rates', href: '/wht-rates', icon: Percent },

  { divider: true, label: 'ADMIN' },
  { name: 'Approval Limits', href: '/approval-limits', icon: Scale },
  { name: 'Settings', href: '/settings', icon: Settings },
  { name: 'Users & Roles', href: '/users', icon: UserCog },
  { name: 'Roles', href: '/roles-admin', icon: Shield },
  { name: 'FX Rates', href: '/fx-rates', icon: RefreshCw },
  { name: 'Import Data', href: '/import', icon: Upload },
  { name: 'Audit Trail', href: '/audit', icon: History },
  { name: 'Notifications', href: '/notifications', icon: BellRing },
];

export default function Sidebar({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { can, loaded } = usePermissions();

  // Hide a nav item when the user demonstrably lacks its permission. Until
  // permissions load (or when an item has no mapped perm) everything shows, so
  // the nav never flickers empty and unmapped items are never wrongly hidden.
  const visible = (href: string) => {
    const perm = PERM_BY_HREF[href];
    if (!perm || !loaded) return true;
    return can(perm);
  };

  // A section divider shows only when at least one item beneath it (up to the
  // next divider) is visible — no empty section headers.
  const dividerHasVisibleItems = (startIdx: number) => {
    for (let i = startIdx + 1; i < navigation.length; i++) {
      const it = navigation[i] as any;
      if (it.divider) break;
      if (visible(it.href)) return true;
    }
    return false;
  };

  return (
    <>
      {/* Mobile backdrop — tap to dismiss the drawer */}
      {open && (
        <div
          className="fixed inset-0 z-40 bg-black/50 lg:hidden"
          onClick={onClose}
          aria-hidden="true"
        />
      )}

      <aside
        className={clsx(
          'fixed inset-y-0 left-0 z-50 w-[260px] bg-[#0f0f1a] flex flex-col transition-transform duration-200 ease-out lg:translate-x-0',
          open ? 'translate-x-0' : '-translate-x-full'
        )}
        aria-label="Primary navigation"
      >
        {/* Logo */}
        <div className="flex h-16 items-center justify-between px-5 border-b border-white/5">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center shadow-lg shadow-indigo-500/20">
              <Calculator className="w-4.5 h-4.5 text-white" />
            </div>
            <div>
              <span className="text-[15px] font-bold text-white tracking-tight">Zavora ERP</span>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="lg:hidden -mr-1 p-1 rounded-lg text-gray-400 hover:text-white hover:bg-white/10 transition-colors"
            aria-label="Close navigation menu"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Amos — AI Accountant */}
        <div className="px-3 pt-3">
          <NavLink
            to="/amos"
            onClick={onClose}
            className={({ isActive }) =>
              clsx(
                'flex items-center gap-2.5 px-3 py-[9px] rounded-lg text-[13px] font-semibold text-white bg-gradient-to-r shadow-lg shadow-indigo-900/30 transition-all duration-150',
                isActive
                  ? 'from-indigo-500 to-purple-500'
                  : 'from-indigo-600/80 to-purple-600/70 hover:from-indigo-500/90 hover:to-purple-500/80'
              )
            }
          >
            <Sparkles className="w-[18px] h-[18px] shrink-0" />
            <span className="flex-1">Amos — AI Accountant</span>
          </NavLink>
        </div>

        {/* Navigation */}
        <nav className="flex-1 overflow-y-auto px-3 py-4 space-y-0.5">
          {navigation.map((item, idx) => {
            if ('divider' in item && item.divider) {
              if (!dividerHasVisibleItems(idx)) return null;
              return (
                <div key={idx} className="pt-4 pb-1 px-3">
                  {item.label && (
                    <span className="text-[10px] font-semibold tracking-widest text-gray-500 uppercase">{item.label}</span>
                  )}
                </div>
              );
            }
            const navItem = item as { name: string; href: string; icon: any };
            if (!visible(navItem.href)) return null;
            return (
              <NavLink
                key={navItem.name}
                to={navItem.href}
                end={navItem.href === '/'}
                onClick={onClose}
                className={({ isActive }) =>
                  clsx(
                    'flex items-center gap-2.5 px-3 py-[7px] rounded-lg text-[13px] font-medium transition-all duration-150',
                    isActive
                      ? 'bg-white/[0.08] text-white shadow-sm shadow-white/5'
                      : 'text-gray-400 hover:text-gray-200 hover:bg-white/[0.04]'
                  )
                }
              >
                <navItem.icon className="w-[18px] h-[18px] shrink-0" />
                {navItem.name}
              </NavLink>
            );
          })}
        </nav>
      </aside>
    </>
  );
}
