import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ChevronDown, LogOut, CalendarClock } from 'lucide-react';
import { getIdentity, logout, clearSession } from '../../api/client';
import { getWorkDate, setWorkDate, realToday } from '../../utils/workDate';

export default function UserMenu() {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [workDate, setWorkDateState] = useState<string>(getWorkDate() ?? '');
  const ref = useRef<HTMLDivElement>(null);

  const identity = getIdentity() as
    | { display_name?: string; role?: string; email?: string }
    | null;
  const displayName = identity?.display_name ?? 'Signed in';
  const role = identity?.role ?? '';
  const email = identity?.email ?? '';
  const initials =
    displayName
      .split(' ')
      .map((p) => p[0])
      .filter(Boolean)
      .slice(0, 2)
      .join('')
      .toUpperCase() || 'U';

  // Close the menu when clicking outside of it.
  useEffect(() => {
    function onClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener('mousedown', onClick);
    return () => document.removeEventListener('mousedown', onClick);
  }, []);

  const handleLogout = async () => {
    setBusy(true);
    try {
      // Clears the httpOnly refresh cookie server-side; ignore network errors.
      await logout();
    } catch {
      // no-op
    } finally {
      clearSession();
      navigate('/login', { replace: true });
    }
  };

  const applyWorkDate = (v: string) => {
    setWorkDateState(v);
    setWorkDate(v || null);
  };
  const workDateActive = !!workDate && workDate !== realToday();

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 rounded-lg py-1 pl-1 pr-2 hover:bg-gray-50 transition-colors"
      >
        <div className="w-8 h-8 rounded-full bg-gradient-to-br from-indigo-400 to-purple-500 flex items-center justify-center ring-2 ring-white shadow-sm">
          <span className="text-xs font-bold text-white">{initials}</span>
        </div>
        <div className="hidden sm:block text-left leading-tight">
          <p className="text-[13px] font-medium text-gray-700 truncate max-w-[140px]">{displayName}</p>
          {workDateActive
            ? <p className="text-[11px] text-amber-600 truncate" title="New documents default to this date">📅 {workDate}</p>
            : (role && <p className="text-[11px] text-gray-400 truncate">{role}</p>)}
        </div>
        <ChevronDown className="w-4 h-4 text-gray-400" />
      </button>

      {open && (
        <div className="absolute right-0 mt-2 w-56 rounded-xl border border-gray-100 bg-white shadow-lg shadow-gray-200/60 py-1 z-50">
          <div className="px-4 py-3 border-b border-gray-100">
            <p className="text-sm font-medium text-gray-800 truncate">{displayName}</p>
            {email && <p className="text-xs text-gray-400 truncate">{email}</p>}
          </div>
          {/* Work-as-of date: per-user default date for new documents. */}
          <div className="px-4 py-3 border-b border-gray-100">
            <label className="flex items-center gap-1.5 text-xs font-medium text-gray-600 mb-1">
              <CalendarClock className="w-3.5 h-3.5" /> Working as of
            </label>
            <input
              type="date"
              value={workDate}
              onChange={(e) => applyWorkDate(e.target.value)}
              className="w-full text-sm border border-gray-200 rounded-md px-2 py-1.5 focus:ring-1 focus:ring-indigo-400 focus:outline-none"
            />
            <div className="flex items-center justify-between mt-1">
              <p className="text-[11px] text-gray-400">
                {workDateActive ? 'New documents default to this date.' : 'New documents use today.'}
              </p>
              {workDateActive && (
                <button onClick={() => applyWorkDate('')} className="text-[11px] text-indigo-600 hover:underline">Reset</button>
              )}
            </div>
          </div>
          <button
            onClick={handleLogout}
            disabled={busy}
            className="w-full flex items-center gap-2 px-4 py-2.5 text-sm text-red-600 hover:bg-red-50 transition-colors disabled:opacity-50"
          >
            <LogOut className="w-4 h-4" />
            {busy ? 'Signing out…' : 'Sign out'}
          </button>
        </div>
      )}
    </div>
  );
}
