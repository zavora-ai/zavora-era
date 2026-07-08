import { useState } from 'react';
import { useNavigate, useSearchParams, Link } from 'react-router-dom';
import { Check } from 'lucide-react';
import { login, signup, storeSession } from '../../api/client';
import Logo from '../../components/brand/Logo';

export default function LoginPage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [orgName, setOrgName] = useState('');
  const [orgType, setOrgType] = useState('limited_company');
  const [kraPin, setKraPin] = useState('');
  const [withSampleData, setWithSampleData] = useState(true);
  // "Start free" CTAs deep-link here with ?signup=1 to open create-organization.
  const [mode, setMode] = useState<'signin' | 'signup'>(
    params.get('signup') === '1' || params.get('mode') === 'signup' ? 'signup' : 'signin',
  );
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const storeAndGo = (session: any) => {
    storeSession(session);
    navigate('/', { replace: true });
  };

  const handleSignIn = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const { data } = await login(email.trim(), password);
      storeAndGo(data);
    } catch (err: any) {
      if (err?.response?.status === 401) {
        setError('Invalid email or password.');
      } else {
        setError(err?.response?.data?.error ?? 'Sign in failed. Please try again.');
      }
    } finally {
      setBusy(false);
    }
  };

  const handleSignup = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      // Create a brand-new organization (tenant) with its own Owner; returns a
      // token pair directly and isolates this org's data from all others.
      const { data } = await signup({
        organization_name: orgName.trim(),
        organization_type: orgType,
        kra_pin: kraPin.trim() || undefined,
        email: email.trim(),
        display_name: displayName.trim(),
        password,
        with_sample_data: withSampleData,
      });
      storeAndGo(data);
    } catch (err: any) {
      setError(err?.response?.data?.error ?? 'Could not create the organization.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="min-h-screen flex">
      {/* Brand panel (hidden on small screens) */}
      <aside className="hidden lg:flex flex-col justify-between w-[44%] bg-slate-950 text-white p-12 relative overflow-hidden">
        <div className="pointer-events-none absolute -top-32 -left-24 h-[420px] w-[420px] rounded-full bg-gradient-to-r from-indigo-600/40 to-fuchsia-600/30 blur-[110px]" />
        <Link to="/" className="relative"><Logo variant="light" /></Link>
        <div className="relative">
          <h2 className="text-3xl font-bold tracking-tight leading-tight">Your books, on autopilot.</h2>
          <p className="mt-4 text-slate-300">One platform for sales, stock, payroll and tax — with Amos, your AI accountant, doing the heavy lifting.</p>
          <ul className="mt-8 space-y-3">
            {['Amos posts, reconciles & prepares taxes', 'KRA eTIMS, M-Pesa & Kenyan payroll built in', 'Books that are always closed'].map((b) => (
              <li key={b} className="flex items-start gap-3 text-slate-200">
                <span className="mt-0.5 flex h-5 w-5 items-center justify-center rounded-full bg-indigo-500/20 text-indigo-300"><Check className="w-3.5 h-3.5" /></span>{b}
              </li>
            ))}
          </ul>
        </div>
        <p className="relative text-xs text-slate-500">© {new Date().getFullYear()} Zavora Technologies Ltd · Made in Kenya 🇰🇪</p>
      </aside>

      {/* Form */}
      <div className="flex-1 flex items-center justify-center bg-gray-50 p-6">
      <div className="card p-8 w-full max-w-md">
        <div className="mb-6">
          <div className="lg:hidden mb-5"><Logo /></div>
          <h1 className="text-2xl font-bold text-gray-900">
            {mode === 'signin' ? 'Welcome back' : 'Create your organization'}
          </h1>
          <p className="text-sm text-gray-500 mt-1">
            {mode === 'signin' ? 'Sign in to your Zavora ERP workspace.' : 'Set up your business on Zavora ERP — free to start.'}
          </p>
        </div>

        {error && (
          <div className="mb-4 rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">
            {error}
          </div>
        )}

        {mode === 'signin' ? (
          <form onSubmit={handleSignIn} className="space-y-4">
            <div>
              <label className="label">Email</label>
              <input
                className="input"
                type="email"
                autoFocus
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@company.co.ke"
              />
            </div>
            <div>
              <label className="label">Password</label>
              <input
                className="input"
                type="password"
                required
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="••••••••"
              />
            </div>
            <button type="submit" className="btn-primary w-full justify-center" disabled={busy}>
              {busy ? 'Signing in…' : 'Sign in'}
            </button>
            <p className="text-center">
              <a href="/forgot-password" className="text-xs text-gray-500 hover:text-indigo-600 hover:underline">Forgot password?</a>
            </p>
            <p className="text-center text-sm text-gray-500">
              First time here?{' '}
              <button type="button" className="text-indigo-600 font-medium" onClick={() => { setMode('signup'); setError(null); }}>
                Create an organization
              </button>
            </p>
          </form>
        ) : (
          <form onSubmit={handleSignup} className="space-y-4">
            <div>
              <label className="label">Organization name</label>
              <input
                className="input"
                autoFocus
                required
                value={orgName}
                onChange={(e) => setOrgName(e.target.value)}
                placeholder="Acme Ltd"
              />
            </div>
            <div>
              <label className="label">Type of organization</label>
              <select
                className="input"
                required
                value={orgType}
                onChange={(e) => setOrgType(e.target.value)}
              >
                <option value="sole_proprietor">Sole proprietor</option>
                <option value="limited_company">Limited company</option>
                <option value="partnership">Partnership</option>
                <option value="ngo">NGO / Non-profit</option>
                <option value="other">Other</option>
              </select>
            </div>
            <div>
              <label className="label">KRA PIN <span className="text-gray-400">(optional)</span></label>
              <input
                className="input"
                value={kraPin}
                onChange={(e) => setKraPin(e.target.value)}
                placeholder="A123456789X"
              />
            </div>
            <div>
              <label className="label">Full name</label>
              <input
                className="input"
                required
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                placeholder="Jane Karanja"
              />
            </div>
            <div>
              <label className="label">Email</label>
              <input
                className="input"
                type="email"
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@company.co.ke"
              />
            </div>
            <div>
              <label className="label">Password</label>
              <input
                className="input"
                type="password"
                required
                minLength={8}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="At least 8 characters"
              />
            </div>
            <label className={`flex items-start gap-3 rounded-lg border px-3 py-2.5 cursor-pointer transition-colors ${withSampleData ? 'border-indigo-300 bg-indigo-50/60' : 'border-gray-200 hover:bg-gray-50'}`}>
              <input
                type="checkbox"
                className="mt-0.5"
                checked={withSampleData}
                onChange={(e) => setWithSampleData(e.target.checked)}
              />
              <span className="text-sm">
                <span className="font-medium text-gray-900">Explore with sample data</span>
                <span className="block text-xs text-gray-500">
                  Load a demo company — customers, vendors, products and invoices — so you can try things out. You can delete it anytime.
                </span>
              </span>
            </label>
            <button type="submit" className="btn-primary w-full justify-center" disabled={busy}>
              {busy ? 'Creating…' : 'Create organization & sign in'}
            </button>
            <p className="text-center text-sm text-gray-500">
              Already have an account?{' '}
              <button type="button" className="text-indigo-600 font-medium" onClick={() => { setMode('signin'); setError(null); }}>
                Sign in
              </button>
            </p>
          </form>
        )}
      </div>
      </div>
    </div>
  );
}
