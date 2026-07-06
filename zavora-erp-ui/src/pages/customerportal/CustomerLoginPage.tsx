import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { customerLogin, customerRegister, customerForgotPassword, storeCustomerSession } from '../../api/customerClient';
import { Building2 } from 'lucide-react';

type Mode = 'login' | 'register' | 'forgot';

/** Customer portal login / self-onboarding — a separate principal from the ERP. */
export default function CustomerLoginPage() {
  const navigate = useNavigate();
  const [mode, setMode] = useState<Mode>('login');
  const [form, setForm] = useState({ email: '', password: '', display_name: '', company: '', phone: '' });
  const [err, setErr] = useState('');
  const [msg, setMsg] = useState('');
  const [busy, setBusy] = useState(false);
  const set = (k: string, v: string) => setForm({ ...form, [k]: v });

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true); setErr(''); setMsg('');
    try {
      if (mode === 'login') {
        const resp = await customerLogin(form.email, form.password);
        storeCustomerSession(resp.data);
        navigate('/customerportal', { replace: true });
      } else if (mode === 'register') {
        const resp = await customerRegister({
          display_name: form.display_name, email: form.email, password: form.password,
          company: form.company || undefined, phone: form.phone || undefined,
        });
        storeCustomerSession(resp.data);
        navigate('/customerportal', { replace: true });
      } else {
        const r = await customerForgotPassword(form.email);
        setMsg(r.data?.message ?? 'If that account exists, a reset link has been sent.');
      }
    } catch (e: any) {
      setErr(e?.response?.data?.error ?? (mode === 'register' ? 'Could not create your account.' : 'Login failed'));
    } finally {
      setBusy(false);
    }
  };

  const title = mode === 'register' ? 'Create your account' : mode === 'forgot' ? 'Reset password' : 'Customer Portal';
  const subtitle = mode === 'register' ? 'Sign up to manage your invoices and support requests'
    : mode === 'forgot' ? 'We\u2019ll email you a reset link'
    : 'Sign in to view invoices, statements and support tickets';

  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-indigo-50 to-purple-50 px-4">
      <div className="w-full max-w-sm">
        <div className="text-center mb-6">
          <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center mx-auto mb-3 shadow-lg shadow-indigo-500/20">
            <Building2 className="w-7 h-7 text-white" />
          </div>
          <h1 className="text-xl font-bold text-gray-900">{title}</h1>
          <p className="text-sm text-gray-500">{subtitle}</p>
        </div>
        <form onSubmit={submit} className="card p-6 space-y-4">
          {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
          {msg && <div className="bg-green-50 text-green-700 text-sm px-3 py-2 rounded">{msg}</div>}
          {mode === 'register' && (
            <>
              <div><label className="label">Your name</label><input className="input" value={form.display_name} onChange={e => set('display_name', e.target.value)} required autoFocus /></div>
              <div><label className="label">Company</label><input className="input" value={form.company} onChange={e => set('company', e.target.value)} /></div>
              <div><label className="label">Phone</label><input className="input" value={form.phone} onChange={e => set('phone', e.target.value)} /></div>
            </>
          )}
          <div><label className="label">Email</label><input type="email" className="input" value={form.email} onChange={e => set('email', e.target.value)} required autoFocus={mode !== 'register'} /></div>
          {mode !== 'forgot' && (
            <div><label className="label">Password</label><input type="password" className="input" value={form.password} onChange={e => set('password', e.target.value)} required minLength={8} /></div>
          )}
          <button type="submit" className="btn-primary w-full justify-center" disabled={busy || (mode === 'forgot' && !form.email)}>
            {busy ? 'Please wait…' : mode === 'register' ? 'Create account' : mode === 'forgot' ? 'Send reset link' : 'Sign in'}
          </button>
          <div className="flex items-center justify-between text-xs pt-1">
            {mode === 'login' ? (
              <>
                <button type="button" onClick={() => { setMode('register'); setErr(''); setMsg(''); }} className="text-indigo-600 hover:underline">Create an account</button>
                <button type="button" onClick={() => { setMode('forgot'); setErr(''); setMsg(''); }} className="text-gray-500 hover:underline">Forgot password?</button>
              </>
            ) : (
              <button type="button" onClick={() => { setMode('login'); setErr(''); setMsg(''); }} className="text-indigo-600 hover:underline">← Back to sign in</button>
            )}
          </div>
        </form>
      </div>
    </div>
  );
}
