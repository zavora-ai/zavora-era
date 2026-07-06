import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { customerSetPassword } from '../../api/customerClient';
import { KeyRound } from 'lucide-react';

/** Accept-invite / password-reset for the customer portal (single-use token). */
export default function CustomerSetPasswordPage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const token = params.get('token') ?? '';
  const [password, setPassword] = useState('');
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
      await customerSetPassword(token, password);
      setDone(true);
      setTimeout(() => navigate('/customerportal/login', { replace: true }), 1500);
    } catch (e: any) {
      setErr(e?.response?.data?.error ?? 'Could not set password');
    } finally { setBusy(false); }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-indigo-50 to-purple-50 px-4">
      <div className="w-full max-w-sm">
        <div className="text-center mb-6">
          <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center mx-auto mb-3 shadow-lg shadow-indigo-500/20">
            <KeyRound className="w-7 h-7 text-white" />
          </div>
          <h1 className="text-xl font-bold text-gray-900">Set your password</h1>
          <p className="text-sm text-gray-500">Choose a password for your customer portal account</p>
        </div>
        <form onSubmit={submit} className="card p-6 space-y-4">
          {!token && <div className="bg-amber-50 text-amber-700 text-sm px-3 py-2 rounded">Missing token. Use the link from your email.</div>}
          {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
          {done && <div className="bg-green-50 text-green-700 text-sm px-3 py-2 rounded">Password set. Redirecting to sign in…</div>}
          <div><label className="label">New password</label><input type="password" className="input" value={password} onChange={e => setPassword(e.target.value)} required /></div>
          <div><label className="label">Confirm password</label><input type="password" className="input" value={confirm} onChange={e => setConfirm(e.target.value)} required /></div>
          <button type="submit" className="btn-primary w-full justify-center" disabled={busy || !token || done}>{busy ? 'Saving…' : 'Set password'}</button>
        </form>
      </div>
    </div>
  );
}
