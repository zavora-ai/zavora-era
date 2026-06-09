import { NavLink } from 'react-router-dom';
import {
  LayoutDashboard, FileText, Receipt, CreditCard, Users, Building2,
  Package, Landmark, Wallet, BarChart3, Settings, BookOpen, Calculator,
  ArrowLeftRight, ClipboardList
} from 'lucide-react';
import clsx from 'clsx';

const navigation = [
  { name: 'Dashboard', href: '/', icon: LayoutDashboard },
  { name: 'Invoices', href: '/invoices', icon: FileText },
  { name: 'Estimates', href: '/estimates', icon: ClipboardList },
  { name: 'Bills', href: '/bills', icon: Receipt },
  { name: 'Payments', href: '/payments', icon: CreditCard },
  { name: 'Customers', href: '/customers', icon: Users },
  { name: 'Vendors', href: '/vendors', icon: Building2 },
  { name: 'Products', href: '/products', icon: Package },
  { name: 'Banking', href: '/banking', icon: Landmark },
  { name: 'Transactions', href: '/transactions', icon: ArrowLeftRight },
  { name: 'Payroll', href: '/payroll', icon: Wallet },
  { name: 'Accounts', href: '/accounts', icon: BookOpen },
  { name: 'Reports', href: '/reports', icon: BarChart3 },
  { name: 'Settings', href: '/settings', icon: Settings },
];

export default function Sidebar() {
  return (
    <aside className="fixed inset-y-0 left-0 z-50 w-64 bg-gray-900 flex flex-col">
      {/* Logo */}
      <div className="flex h-16 items-center px-6 border-b border-gray-800">
        <div className="flex items-center gap-2">
          <div className="w-8 h-8 rounded-lg bg-blue-600 flex items-center justify-center">
            <Calculator className="w-5 h-5 text-white" />
          </div>
          <span className="text-lg font-bold text-white">Zavora ERA</span>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto px-3 py-4 space-y-1">
        {navigation.map((item) => (
          <NavLink
            key={item.name}
            to={item.href}
            end={item.href === '/'}
            className={({ isActive }) =>
              clsx(
                'flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-colors',
                isActive
                  ? 'bg-gray-800 text-white'
                  : 'text-gray-400 hover:text-white hover:bg-gray-800/50'
              )
            }
          >
            <item.icon className="w-5 h-5 shrink-0" />
            {item.name}
          </NavLink>
        ))}
      </nav>

      {/* User */}
      <div className="p-4 border-t border-gray-800">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-full bg-gray-700 flex items-center justify-center">
            <span className="text-sm font-medium text-gray-300">JK</span>
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-white truncate">James Karanja</p>
            <p className="text-xs text-gray-400 truncate">Owner</p>
          </div>
        </div>
      </div>
    </aside>
  );
}
