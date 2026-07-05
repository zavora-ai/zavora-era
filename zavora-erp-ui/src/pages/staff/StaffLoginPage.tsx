import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { staffLogin, storeStaffSession, staffForgotPassword } from '../../api/staffClient';
import { UserCircle } from 'lucide-react';

/** Employee self-service login — a separate principal from the back-office ERP. */
export default function StaffLoginPage() {
  const navigate = useNavigate();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [err, setErr] = useState('');
  const [busy, setBusy] = useState(false);
  const [forgot, setForgot] = useState(false);
  const [msg, setMsg] = useState('');

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true); setErr('');
    try {
      const resp = await staffLogin(email, password);
      storeStaffSession(resp.data);
      navigate('/staff', { replace: true });
    } catch (e: any) {
      setErr(e?.response?.data?.error ?? 'Login failed');
    } finally {
      setBusy(false);
    }
  };

  const sendReset = async () => {
    setBusy(true); setErr(''); setMsg('');
    try {
      const r = await staffForgotPassword(email);
      setMsg(r.data?.message ?? 'If that account exists, a reset link has been sent.');
    } catch {
      setMsg('If that account exists, a reset link has been sent.');
    } finally { setBusy(false); }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-indigo-50 to-purple-50 px-4">
      <div className="w-full max-w-sm">
        <div className="text-center mb-6">
          <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center mx-auto mb-3 shadow-lg shadow-indigo-500/20">
            <UserCircle className="w-7 h-7 text-white" />
          </div>
          <h1 className="text-xl font-bold text-gray-900">Employee Self-Service</h1>
          <p className="text-sm text-gray-500">Sign in to view payslips and request leave</p>
        </div>
        <form onSubmit={submit} className="card p-6 space-y-4">
          {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
          {msg && <div className="bg-green-50 text-green-700 text-sm px-3 py-2 rounded">{msg}</div>}
          <div>
            <label className="label">Work Email</label>
            <input type="email" className="input" value={email} onChange={e => setEmail(e.target.value)} required autoFocus />
          </div>
          {!forgot && (
            <div>
              <label className="label">Password</label>
              <input type="password" className="input" value={password} onChange={e => setPassword(e.target.value)} required />
            </div>
          )}
          {!forgot ? (
            <button type="submit" className="btn-primary w-full justify-center" disabled={busy}>
              {busy ? 'Signing in…' : 'Sign in'}
            </button>
          ) : (
            <button type="button" onClick={sendReset} className="btn-primary w-full justify-center" disabled={busy || !email}>
              {busy ? 'Sending…' : 'Send reset link'}
            </button>
          )}
          <div className="text-center">
            <button type="button" onClick={() => { setForgot(f => !f); setErr(''); setMsg(''); }} className="text-xs text-indigo-600 hover:underline">
              {forgot ? '← Back to sign in' : 'Forgot password?'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
