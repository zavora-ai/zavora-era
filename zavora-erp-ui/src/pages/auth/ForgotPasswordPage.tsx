import { useState } from 'react';
import { forgotPassword } from '../../api/client';

/** Internal-user password recovery — always returns a neutral message (no enumeration). */
export default function ForgotPasswordPage() {
  const [email, setEmail] = useState('');
  const [msg, setMsg] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      const r = await forgotPassword(email.trim());
      setMsg(r.data?.message ?? 'If that account exists, a reset link has been sent.');
    } catch {
      setMsg('If that account exists, a reset link has been sent.');
    } finally { setBusy(false); }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 p-6">
      <div className="card p-8 w-full max-w-md">
        <div className="mb-6 text-center">
          <h1 className="text-2xl font-semibold text-gray-900">Reset password</h1>
          <p className="text-sm text-gray-500 mt-1">We'll email you a link to set a new password</p>
        </div>
        <form onSubmit={submit} className="space-y-4">
          {msg && <div className="rounded-lg bg-green-50 border border-green-200 px-3 py-2 text-sm text-green-700">{msg}</div>}
          <div><label className="label">Email</label>
            <input className="input" type="email" autoFocus required value={email} onChange={(e) => setEmail(e.target.value)} placeholder="you@company.co.ke" /></div>
          <button type="submit" className="btn-primary w-full justify-center" disabled={busy || !email}>
            {busy ? 'Sending…' : 'Send reset link'}
          </button>
          <p className="text-center text-sm"><a href="/login" className="text-indigo-600 hover:underline">← Back to sign in</a></p>
        </form>
      </div>
    </div>
  );
}
