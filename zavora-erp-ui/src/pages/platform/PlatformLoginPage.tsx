import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { platformLogin, storePlatformSession } from '../../api/platformClient';

/** Zavora ops plane — not customer tenant signup. */
export default function PlatformLoginPage() {
  const navigate = useNavigate();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const { data } = await platformLogin(email.trim(), password);
      storePlatformSession(data);
      navigate('/platform', { replace: true });
    } catch (err: any) {
      setError(err?.response?.data?.error ?? 'Sign-in failed');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-slate-950 p-6">
      <div className="w-full max-w-md rounded-2xl border border-slate-800 bg-slate-900 p-8 shadow-xl">
        <div className="mb-6 text-center">
          <p className="text-xs font-semibold uppercase tracking-widest text-indigo-400">Zavora Platform</p>
          <h1 className="mt-2 text-2xl font-semibold text-white">Super Admin</h1>
          <p className="mt-1 text-sm text-slate-400">Operator access to the multi-tenant console</p>
        </div>
        <form onSubmit={submit} className="space-y-4">
          {error && (
            <div className="rounded-lg border border-red-900/50 bg-red-950/50 px-3 py-2 text-sm text-red-300">
              {error}
            </div>
          )}
          <div>
            <label className="mb-1 block text-xs font-medium text-slate-400">Email</label>
            <input
              className="w-full rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white outline-none focus:border-indigo-500"
              type="email"
              name="email"
              autoComplete="username"
              autoFocus
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="ops@zavora.ai"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs font-medium text-slate-400">Password</label>
            <input
              className="w-full rounded-lg border border-slate-700 bg-slate-950 px-3 py-2 text-sm text-white outline-none focus:border-indigo-500"
              type="password"
              name="password"
              autoComplete="current-password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>
          <button
            type="submit"
            disabled={busy}
            className="w-full rounded-lg bg-indigo-600 py-2.5 text-sm font-semibold text-white hover:bg-indigo-500 disabled:opacity-50"
          >
            {busy ? 'Signing in…' : 'Sign in'}
          </button>
        </form>
        <p className="mt-6 text-center text-xs text-slate-500">
          Tenant ERP login is at{' '}
          <a href="/login" className="text-indigo-400 hover:underline">
            /login
          </a>
        </p>
      </div>
    </div>
  );
}
