import { createContext, useCallback, useContext, useState, type ReactNode } from 'react';
import { CheckCircle, AlertCircle, Info, X } from 'lucide-react';
import clsx from 'clsx';

export type ToastKind = 'success' | 'error' | 'info';

interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

interface ToastApi {
  success: (message: string) => void;
  error: (message: string) => void;
  info: (message: string) => void;
  /** Convenience for mutation onError: pulls the server error, falls back. */
  fromError: (e: any, fallback?: string) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

const AUTO_DISMISS_MS: Record<ToastKind, number> = {
  success: 4000,
  info: 4000,
  error: 6000,
};

let nextId = 1;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const dismiss = useCallback((id: number) => {
    setToasts((cur) => cur.filter((t) => t.id !== id));
  }, []);

  const push = useCallback(
    (kind: ToastKind, message: string) => {
      const id = nextId++;
      setToasts((cur) => [...cur, { id, kind, message }]);
      window.setTimeout(() => dismiss(id), AUTO_DISMISS_MS[kind]);
    },
    [dismiss],
  );

  const api: ToastApi = {
    success: (m) => push('success', m),
    error: (m) => push('error', m),
    info: (m) => push('info', m),
    fromError: (e, fallback = 'Something went wrong.') =>
      push('error', e?.response?.data?.error || e?.response?.data?.message || fallback),
  };

  return (
    <ToastContext.Provider value={api}>
      {children}
      {/* Stack — top-right, above modals */}
      <div className="fixed top-4 right-4 z-[100] flex flex-col gap-2 w-[calc(100vw-2rem)] max-w-sm pointer-events-none">
        {toasts.map((t) => (
          <div
            key={t.id}
            role="status"
            aria-live="polite"
            className={clsx(
              'pointer-events-auto flex items-start gap-2 p-3 rounded-lg text-sm shadow-lg border animate-in fade-in slide-in-from-top-2',
              t.kind === 'success' && 'bg-green-50 text-green-800 border-green-200',
              t.kind === 'error' && 'bg-red-50 text-red-800 border-red-200',
              t.kind === 'info' && 'bg-indigo-50 text-indigo-800 border-indigo-200',
            )}
          >
            {t.kind === 'success' && <CheckCircle className="w-4 h-4 shrink-0 mt-0.5" />}
            {t.kind === 'error' && <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />}
            {t.kind === 'info' && <Info className="w-4 h-4 shrink-0 mt-0.5" />}
            <span className="flex-1 break-words">{t.message}</span>
            <button
              type="button"
              onClick={() => dismiss(t.id)}
              className="shrink-0 opacity-60 hover:opacity-100 transition-opacity"
              aria-label="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

/**
 * Access the global toast API. Safe to call anywhere under <ToastProvider>.
 * Falls back to window.alert if (defensively) used outside the provider.
 */
export function useToast(): ToastApi {
  const ctx = useContext(ToastContext);
  if (ctx) return ctx;
  return {
    success: (m) => window.alert(m),
    error: (m) => window.alert(m),
    info: (m) => window.alert(m),
    fromError: (e, fallback = 'Something went wrong.') =>
      window.alert(e?.response?.data?.error || e?.response?.data?.message || fallback),
  };
}
