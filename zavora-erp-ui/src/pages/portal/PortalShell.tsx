import { useEffect, useState } from 'react';
import { NavLink, Outlet, useNavigate } from 'react-router-dom';
import { Building2, Gavel, FileText, ShoppingCart, Receipt, LogOut } from 'lucide-react';
import clsx from 'clsx';
import {
  getVendorToken, getVendorIdentity, bootstrapVendorAuth, clearVendorSession, portalLogout,
} from '../../api/portalClient';

const nav = [
  { name: 'Tenders', href: '/vendorportal', icon: Gavel, end: true },
  { name: 'My Bids', href: '/vendorportal/bids', icon: FileText },
  { name: 'Purchase Orders', href: '/vendorportal/purchase-orders', icon: ShoppingCart },
  { name: 'Statement', href: '/vendorportal/statement', icon: Receipt },
];

/**
 * Vendor-portal layout + auth gate. Boots the vendor session from the portal
 * refresh cookie independently of the staff app, then renders a slim supplier
 * shell (no ERP navigation). Redirects to /portal/login when unauthenticated.
 */
export default function PortalShell() {
  const navigate = useNavigate();
  const [booting, setBooting] = useState(true);
  const [authed, setAuthed] = useState(false);

  useEffect(() => {
    (async () => {
      const ok = getVendorToken() != null || (await bootstrapVendorAuth());
      setAuthed(ok);
      setBooting(false);
      if (!ok) navigate('/vendorportal/login', { replace: true });
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const vendor = getVendorIdentity() as { company_name?: string; display_name?: string } | null;

  const handleLogout = async () => {
    try { await portalLogout(); } catch { /* ignore */ }
    clearVendorSession();
    navigate('/vendorportal/login', { replace: true });
  };

  if (booting) {
    return <div className="min-h-screen flex items-center justify-center text-gray-500">Loading…</div>;
  }
  if (!authed) return null;

  return (
    <div className="min-h-screen bg-gray-50">
      {/* Top bar */}
      <header className="bg-white border-b border-gray-200 sticky top-0 z-40">
        <div className="max-w-6xl mx-auto px-6 h-16 flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-emerald-500 to-teal-600 flex items-center justify-center shadow-lg shadow-emerald-500/20">
              <Building2 className="w-4.5 h-4.5 text-white" />
            </div>
            <div>
              <span className="text-[15px] font-bold text-gray-900 tracking-tight">Zavora</span>
              <span className="text-[15px] font-medium text-emerald-600 ml-1">Vendor Portal</span>
            </div>
          </div>
          <div className="flex items-center gap-4">
            {vendor?.company_name && (
              <div className="text-right hidden sm:block">
                <p className="text-sm font-medium text-gray-900 leading-tight">{vendor.company_name}</p>
                <p className="text-xs text-gray-400 leading-tight">{vendor.display_name}</p>
              </div>
            )}
            <button onClick={handleLogout} className="btn-secondary text-sm" title="Sign out">
              <LogOut className="w-4 h-4" /> Sign out
            </button>
          </div>
        </div>
        {/* Nav */}
        <nav className="max-w-6xl mx-auto px-6 flex gap-1 -mb-px">
          {nav.map((item) => (
            <NavLink
              key={item.name}
              to={item.href}
              end={item.end}
              className={({ isActive }) =>
                clsx(
                  'flex items-center gap-2 px-4 py-3 text-sm font-medium border-b-2 transition-colors',
                  isActive
                    ? 'border-emerald-600 text-emerald-700'
                    : 'border-transparent text-gray-500 hover:text-gray-800'
                )
              }
            >
              <item.icon className="w-4 h-4" />
              {item.name}
            </NavLink>
          ))}
        </nav>
      </header>

      <main className="max-w-6xl mx-auto px-6 py-8">
        <Outlet />
      </main>
    </div>
  );
}
