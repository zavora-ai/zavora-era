import { useEffect, useState } from 'react';
import { clearSession, logout as apiLogout, getIdentity } from '../../api/client';
import { ShieldAlert } from 'lucide-react';

interface SupportMeta {
  organization_name?: string;
  entity_id?: string;
  target_email?: string;
  suspended?: boolean;
  read_only?: boolean;
  reason?: string;
}

const KEY = 'era_support_session';

function readMeta(): SupportMeta | null {
  try {
    const raw = sessionStorage.getItem(KEY);
    if (!raw) return null;
    return JSON.parse(raw) as SupportMeta;
  } catch {
    return null;
  }
}

export function clearSupportSessionMeta() {
  try {
    sessionStorage.removeItem(KEY);
  } catch {
    /* ignore */
  }
}

/** Amber strip shown when a platform operator is inside a tenant via impersonate. */
export default function SupportSessionBanner() {
  const [meta, setMeta] = useState<SupportMeta | null>(null);

  useEffect(() => {
    const fromStorage = readMeta();
    const identity = getIdentity() as {
      support_session?: boolean;
      email?: string;
      read_only?: boolean;
    } | null;
    if (fromStorage || identity?.support_session) {
      setMeta(
        fromStorage ?? {
          target_email: identity?.email,
          organization_name: 'this tenant',
          read_only: identity?.read_only,
        },
      );
    }
  }, []);

  if (!meta) return null;

  const exit = async () => {
    try {
      await apiLogout();
    } catch {
      /* ignore */
    }
    clearSession();
    clearSupportSessionMeta();
    window.location.href = '/platform';
  };

  return (
    <div
      className={`sticky top-0 z-50 flex items-center justify-between gap-4 border-b px-4 py-2 text-sm print:hidden ${
        meta.read_only
          ? 'border-sky-700 bg-sky-400 text-sky-950'
          : 'border-amber-700 bg-amber-500 text-amber-950'
      }`}
    >
      <div className="flex items-center gap-2 font-medium min-w-0">
        <ShieldAlert className="h-4 w-4 shrink-0" />
        <span className="truncate">
          {meta.read_only ? 'Read-only support session' : 'Support session'}
          {meta.organization_name ? (
            <>
              {' '}
              in <strong>{meta.organization_name}</strong>
            </>
          ) : null}
          {meta.target_email ? <> as {meta.target_email}</> : null}
          {meta.suspended ? ' · tenant is suspended' : ''}
          {meta.reason ? ` · ${meta.reason}` : ''}
          . Audited · short-lived.
        </span>
      </div>
      <button
        type="button"
        onClick={exit}
        className={`shrink-0 rounded-md px-3 py-1 text-xs font-semibold ${
          meta.read_only
            ? 'bg-sky-950/90 text-sky-50 hover:bg-sky-950'
            : 'bg-amber-950/90 text-amber-50 hover:bg-amber-950'
        }`}
      >
        Exit to platform
      </button>
    </div>
  );
}
