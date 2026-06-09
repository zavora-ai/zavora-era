import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard, FileText, Receipt, CreditCard, Users, Building2,
  Package, Landmark, Wallet, BarChart3, Settings, BookOpen, Calculator,
  ArrowLeftRight, ClipboardList, UserCheck, BookMarked, Boxes, Building,
  RefreshCw, History
} from 'lucide-react';
import clsx from 'clsx';

const navigation = [
  { name: 'Dashboard', href: '/', icon: LayoutDashboard },
  { divider: true, label: 'SALES' },
  { name: 'Invoices', href: '/invoices', icon: FileText },
  { name: 'Estimates', href: '/estimates', icon: ClipboardList },
  { name: 'Recurring', href: '/recurring-invoices', icon: RefreshCw },
  { name: 'Customers', href: '/customers', icon: Users },
  { divider: true, label: 'PURCHASES' },
  { name: 'Bills', href: '/bills', icon: Receipt },
  { name: 'Vendors', href: '/vendors', icon: Building2 },
  { divider: true, label: 'MONEY' },
  { name: 'Payments', href: '/payments', icon: CreditCard },
  { name: 'Banking', href: '/banking', icon: Landmark },
  { name: 'Transactions', href: '/transactions', icon: ArrowLeftRight },
  { divider: true, label: 'ACCOUNTING' },
  { name: 'Products', href: '/products', icon: Package },
  { name: 'Inventory', href: '/inventory', icon: Boxes },
  { name: 'Assets', href: '/assets', icon: Building },
  { name: 'Employees', href: '/employees', icon: UserCheck },
  { name: 'Payroll', href: '/payroll', icon: Wallet },
  { name: 'Accounts', href: '/accounts', icon: BookOpen },
  { name: 'Journal Entries', href: '/journal-entries', icon: BookMarked },
  { name: 'Reports', href: '/reports', icon: BarChart3 },
  { divider: true, label: '' },
  { name: 'Settings', href: '/settings', icon: Settings },
  { name: 'FX Rates', href: '/fx-rates', icon: RefreshCw },
  { name: 'Audit Trail', href: '/audit', icon: History },
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

      {/* User */}
      <div className="p-4 border-t border-white/5">
        <div className="flex items-center gap-3 px-2">
          <div className="w-8 h-8 rounded-full bg-gradient-to-br from-indigo-400 to-purple-500 flex items-center justify-center ring-2 ring-white/10">
            <span className="text-xs font-bold text-white">JK</span>
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-[13px] font-medium text-gray-200 truncate">James Karanja</p>
            <p className="text-[11px] text-gray-500 truncate">Owner</p>
          </div>
        </div>
      </div>
    </aside>
  );
}
