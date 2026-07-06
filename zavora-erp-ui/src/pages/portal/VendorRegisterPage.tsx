import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Building2, CheckCircle2 } from 'lucide-react';
import { portalRegister } from '../../api/portalClient';

export default function VendorRegisterPage() {
  const navigate = useNavigate();
  const [form, setForm] = useState({ company_name: '', display_name: '', email: '', password: '', kra_pin: '', phone: '' });
  const [error, setError] = useState('');
  const [done, setDone] = useState(false);
  const [loading, setLoading] = useState(false);

  const set = (k: string, v: string) => setForm((f) => ({ ...f, [k]: v }));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    if (form.password.length < 8) { setError('Password must be at least 8 characters.'); return; }
    setLoading(true);
    try {
      await portalRegister({
        company_name: form.company_name,
        display_name: form.display_name,
        email: form.email,
        password: form.password,
        kra_pin: form.kra_pin || undefined,
        phone: form.phone || undefined,
      });
      setDone(true);
    } catch (err: any) {
      setError(err?.response?.data?.error ?? 'Registration failed. Please try again.');
    } finally {
      setLoading(false);
    }
  };

  if (done) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-emerald-50 to-teal-50 flex items-center justify-center px-4">
        <div className="w-full max-w-md bg-white rounded-2xl shadow-xl p-8 text-center">
          <CheckCircle2 className="w-12 h-12 text-emerald-500 mx-auto mb-4" />
          <h1 className="text-xl font-bold text-gray-900 mb-2">Registration received</h1>
          <p className="text-sm text-gray-500 mb-6">
            An account manager will review your details and approve your access. You'll be able to sign in once approved.
          </p>
          <button onClick={() => navigate('/vendorportal/login')} className="btn-primary w-full justify-center bg-emerald-600 hover:bg-emerald-700">
            Back to sign in
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-emerald-50 to-teal-50 flex items-center justify-center px-4 py-10">
      <div className="w-full max-w-md">
        <div className="flex items-center justify-center gap-2.5 mb-6">
          <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-emerald-500 to-teal-600 flex items-center justify-center shadow-lg shadow-emerald-500/20">
            <Building2 className="w-5 h-5 text-white" />
          </div>
          <div>
            <span className="text-lg font-bold text-gray-900">Zavora</span>
            <span className="text-lg font-medium text-emerald-600 ml-1">Vendor Portal</span>
          </div>
        </div>

        <div className="bg-white rounded-2xl shadow-xl p-8">
          <h1 className="text-xl font-bold text-gray-900 mb-1">Register your company</h1>
          <p className="text-sm text-gray-500 mb-6">Become a supplier to bid on tenders and receive purchase orders.</p>

          {error && <div className="mb-4 p-3 rounded-lg bg-red-50 text-red-700 text-sm">{error}</div>}

          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="label">Company name *</label>
              <input className="input" value={form.company_name} onChange={(e) => set('company_name', e.target.value)} required autoFocus />
            </div>
            <div>
              <label className="label">Contact name *</label>
              <input className="input" value={form.display_name} onChange={(e) => set('display_name', e.target.value)} required />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="label">KRA PIN</label>
                <input className="input" value={form.kra_pin} onChange={(e) => set('kra_pin', e.target.value)} placeholder="Optional" />
              </div>
              <div>
                <label className="label">Phone</label>
                <input className="input" value={form.phone} onChange={(e) => set('phone', e.target.value)} placeholder="Optional" />
              </div>
            </div>
            <div>
              <label className="label">Email *</label>
              <input type="email" className="input" value={form.email} onChange={(e) => set('email', e.target.value)} required />
            </div>
            <div>
              <label className="label">Password *</label>
              <input type="password" className="input" value={form.password} onChange={(e) => set('password', e.target.value)} required minLength={8} placeholder="At least 8 characters" />
            </div>
            <button type="submit" className="btn-primary w-full justify-center bg-emerald-600 hover:bg-emerald-700" disabled={loading}>
              {loading ? 'Submitting…' : 'Register'}
            </button>
          </form>

          <p className="text-sm text-gray-500 mt-6 text-center">
            Already registered? <Link to="/vendorportal/login" className="font-medium text-emerald-600 hover:text-emerald-700">Sign in</Link>
          </p>
        </div>
      </div>
    </div>
  );
}
