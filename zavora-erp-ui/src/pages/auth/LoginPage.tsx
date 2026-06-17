import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { login, signup, storeSession } from '../../api/client';

export default function LoginPage() {
  const navigate = useNavigate();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [orgName, setOrgName] = useState('');
  const [orgType, setOrgType] = useState('limited_company');
  const [kraPin, setKraPin] = useState('');
  const [mode, setMode] = useState<'signin' | 'signup'>('signin');
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
      });
      storeAndGo(data);
    } catch (err: any) {
      setError(err?.response?.data?.error ?? 'Could not create the organization.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 p-6">
      <div className="card p-8 w-full max-w-md">
        <div className="mb-6 text-center">
          <h1 className="text-2xl font-semibold text-gray-900">Zavora ERP</h1>
          <p className="text-sm text-gray-500 mt-1">
            {mode === 'signin' ? 'Sign in to your workspace' : 'Create your organization'}
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
  );
}
