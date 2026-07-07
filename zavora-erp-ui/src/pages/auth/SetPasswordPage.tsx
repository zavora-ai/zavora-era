import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { setPassword } from '../../api/client';

/** Internal-user activation / password reset — consumes a single-use token. */
export default function SetPasswordPage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const token = params.get('token') ?? '';
  const [password, setPw] = useState('');
  const [confirm, setConfirm] = useState('');
  const [err, setErr] = useState('');
  const [done, setDone] = useState(false);
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (password.length < 8) { setErr('Password must be at least 8 characters'); return; }
    if (password !== confirm) { setErr('Passwords do not match'); return; }
    setBusy(true); setErr('');
    try {
      await setPassword(token, password);
      setDone(true);
      setTimeout(() => navigate('/login', { replace: true }), 1500);
    } catch (e: any) {
      setErr(e?.response?.data?.error ?? 'Could not set password');
    } finally { setBusy(false); }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 p-6">
      <div className="card p-8 w-full max-w-md">
        <div className="mb-6 text-center">
          <h1 className="text-2xl font-semibold text-gray-900">Set your password</h1>
          <p className="text-sm text-gray-500 mt-1">Choose a password to activate your Zavora ERP account</p>
        </div>
        <form onSubmit={submit} className="space-y-4">
          {!token && <div className="rounded-lg bg-amber-50 border border-amber-200 px-3 py-2 text-sm text-amber-700">Missing token. Use the link from your email.</div>}
          {err && <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">{err}</div>}
          {done && <div className="rounded-lg bg-green-50 border border-green-200 px-3 py-2 text-sm text-green-700">Password set. Redirecting to sign in…</div>}
          <div><label className="label">New password</label>
            <input className="input" type="password" value={password} onChange={(e) => setPw(e.target.value)} required minLength={8} /></div>
          <div><label className="label">Confirm password</label>
            <input className="input" type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} required /></div>
          <button type="submit" className="btn-primary w-full justify-center" disabled={busy || !token || done}>
            {busy ? 'Saving…' : 'Set password'}
          </button>
        </form>
      </div>
    </div>
  );
}
