import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard, FileText, Receipt, CreditCard, Users, Building2,
  Package, Landmark, Wallet, BarChart3, Settings, BookOpen, Calculator,
  ArrowLeftRight, ClipboardList, UserCheck, BookMarked, Boxes, Building,
  RefreshCw, History, Camera, UserCog, CalendarClock, FileMinus, Target, Layers, Network, Percent, Scale, Upload, CheckCircle, FileCheck, BellRing, Sparkles
} from 'lucide-react';
import clsx from 'clsx';

const navigation = [
  { name: 'Dashboard', href: '/', icon: LayoutDashboard },

  { divider: true, label: 'SALES' },
  { name: 'Invoices', href: '/invoices', icon: FileText },
  { name: 'Estimates', href: '/estimates', icon: ClipboardList },
  { name: 'Recurring Invoices', href: '/recurring-invoices', icon: RefreshCw },
  { name: 'Customers', href: '/customers', icon: Users },

  { divider: true, label: 'PURCHASES' },
  { name: 'Bills', href: '/bills', icon: Receipt },
  { name: 'Supplier Credits', href: '/supplier-credit-notes', icon: FileMinus },
  { name: 'Capture Receipt', href: '/receipts/capture', icon: Camera },
  { name: 'Vendors', href: '/vendors', icon: Building2 },

  { divider: true, label: 'BANKING' },
  { name: 'Payments', href: '/payments', icon: CreditCard },
  { name: 'Banking', href: '/banking', icon: Landmark },
  { name: 'Reconciliation', href: '/reconciliation', icon: CheckCircle },
  { name: 'Transactions', href: '/transactions', icon: ArrowLeftRight },

  { divider: true, label: 'PRODUCTS & INVENTORY' },
  { name: 'Products', href: '/products', icon: Package },
  { name: 'Inventory', href: '/inventory', icon: Boxes },

  { divider: true, label: 'PAYROLL & HR' },
  { name: 'Employees', href: '/employees', icon: UserCheck },
  { name: 'Payroll', href: '/payroll', icon: Wallet },

  { divider: true, label: 'ACCOUNTING' },
  { name: 'Chart of Accounts', href: '/accounts', icon: BookOpen },
  { name: 'Journal Entries', href: '/journal-entries', icon: BookMarked },
  { name: 'Recurring Journals', href: '/recurring-journals', icon: RefreshCw },
  { name: 'Fixed Assets', href: '/assets', icon: Building },
  { name: 'Opening Balances', href: '/opening-balances', icon: Scale },
  { name: 'Periods', href: '/periods', icon: CalendarClock },

  { divider: true, label: 'REPORTS & ANALYSIS' },
  { name: 'Reports', href: '/reports', icon: BarChart3 },
  { name: 'Budgets', href: '/budgets', icon: Target },
  { name: 'Dimensions', href: '/dimensions', icon: Layers },
  { name: 'Consolidation', href: '/consolidation', icon: Network },

  { divider: true, label: 'TAX & COMPLIANCE' },
  { name: 'Tax Filing', href: '/tax-filings', icon: FileCheck },
  { name: 'WHT Rates', href: '/wht-rates', icon: Percent },

  { divider: true, label: 'ADMIN' },
  { name: 'Settings', href: '/settings', icon: Settings },
  { name: 'Users & Roles', href: '/users', icon: UserCog },
  { name: 'FX Rates', href: '/fx-rates', icon: RefreshCw },
  { name: 'Import Data', href: '/import', icon: Upload },
  { name: 'Audit Trail', href: '/audit', icon: History },
  { name: 'Notifications', href: '/notifications', icon: BellRing },
];

export default function Sidebar() {
  return (
    <aside className="fixed inset-y-0 left-0 z-50 w-[260px] bg-[#0f0f1a] flex flex-col">
      {/* Logo */}
      <div className="flex h-16 items-center px-5 border-b border-white/5">
        <div className="flex items-center gap-2.5">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center shadow-lg shadow-indigo-500/20">
            <Calculator className="w-4.5 h-4.5 text-white" />
          </div>
          <div>
            <span className="text-[15px] font-bold text-white tracking-tight">Zavora ERP</span>
          </div>
        </div>
      </div>

      {/* Amos — AI Accountant */}
      <div className="px-3 pt-3">
        <NavLink
          to="/amos"
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
            return (
              <div key={idx} className="pt-4 pb-1 px-3">
                {item.label && (
                  <span className="text-[10px] font-semibold tracking-widest text-gray-500 uppercase">{item.label}</span>
                )}
              </div>
            );
          }
          const navItem = item as { name: string; href: string; icon: any };
          return (
            <NavLink
              key={navItem.name}
              to={navItem.href}
              end={navItem.href === '/'}
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
  );
}
