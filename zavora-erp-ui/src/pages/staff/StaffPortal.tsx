import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  bootstrapStaffAuth, getStaffToken, getStaffIdentity, clearStaffSession, staffLogout,
  staffGetLeaveBalances, staffGetLeaveRequests, staffCreateLeaveRequest, staffCancelLeaveRequest,
  staffGetPayslips, staffGetProfile, staffUpdateProfile, staffGetLeaveTypes, staffGetHolidays, staffGetPayslipPdf,
} from '../../api/staffClient';
import { CalendarClock, Receipt, UserCircle, LogOut, Plus } from 'lucide-react';

type View = 'leave' | 'payslips' | 'profile';

/** Reduced self-service shell for employees — separate principal, own session. */
export default function StaffPortal() {
  const navigate = useNavigate();
  const [ready, setReady] = useState(false);
  const [authed, setAuthed] = useState(false);
  const [view, setView] = useState<View>('leave');

  useEffect(() => {
    (async () => {
      const ok = getStaffToken() != null || (await bootstrapStaffAuth());
      setAuthed(ok); setReady(true);
      if (!ok) navigate('/staff/login', { replace: true });
    })();
  }, [navigate]);

  if (!ready) return <div className="min-h-screen flex items-center justify-center text-gray-400">Loading…</div>;
  if (!authed) return null;

  const identity = getStaffIdentity();
  const logout = async () => { try { await staffLogout(); } catch { /* ignore */ } clearStaffSession(); navigate('/staff/login', { replace: true }); };

  const nav: [View, string, any][] = [
    ['leave', 'My Leave', CalendarClock],
    ['payslips', 'My Payslips', Receipt],
    ['profile', 'My Profile', UserCircle],
  ];

  return (
    <div className="min-h-screen bg-gray-50">
      <header className="bg-white border-b border-gray-200">
        <div className="max-w-4xl mx-auto px-4 h-14 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center">
              <UserCircle className="w-4 h-4 text-white" />
            </div>
            <span className="font-semibold text-gray-800 text-sm">Employee Self-Service</span>
          </div>
          <div className="flex items-center gap-3">
            <span className="text-sm text-gray-500">{identity?.display_name ?? identity?.email}</span>
            <button onClick={logout} className="text-sm text-red-600 flex items-center gap-1 hover:bg-red-50 px-2 py-1 rounded"><LogOut className="w-4 h-4" /> Sign out</button>
          </div>
        </div>
      </header>

      <div className="max-w-4xl mx-auto px-4 py-6">
        <div className="flex gap-1 border-b border-gray-200 mb-5 overflow-x-auto">
          {nav.map(([k, label, Icon]) => (
            <button key={k} onClick={() => setView(k)}
              className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px flex items-center gap-1.5 ${
                view === k ? 'border-indigo-500 text-indigo-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}>
              <Icon className="w-4 h-4" /> {label}
            </button>
          ))}
        </div>
        {view === 'leave' && <MyLeave />}
        {view === 'payslips' && <MyPayslips />}
        {view === 'profile' && <MyProfile />}
      </div>
    </div>
  );
}

const statusColor = (s: string) =>
  s === 'Approved' ? 'bg-green-100 text-green-700'
  : s === 'Declined' ? 'bg-red-100 text-red-700'
  : s === 'Cancelled' ? 'bg-gray-100 text-gray-600'
  : 'bg-amber-100 text-amber-700';

function MyLeave() {
  const qc = useQueryClient();
  const [showNew, setShowNew] = useState(false);
  const { data: balances = [] } = useQuery<any[]>({ queryKey: ['my-balances'], queryFn: () => staffGetLeaveBalances().then(r => r.data) });
  const { data: requests = [] } = useQuery<any[]>({ queryKey: ['my-requests'], queryFn: () => staffGetLeaveRequests().then(r => r.data) });
  const { data: types = [] } = useQuery<any[]>({ queryKey: ['staff-leave-types'], queryFn: () => staffGetLeaveTypes().then(r => r.data).catch(() => []) });
  const typeName = (id: string) => typeById(types, id);
  const avail = (b: any) => Number(b.accrued_days) + Number(b.carried_over) - Number(b.taken_days) - Number(b.pending_days);
  const cancelMut = useMutation({ mutationFn: (id: string) => staffCancelLeaveRequest(id), onSuccess: () => { qc.invalidateQueries({ queryKey: ['my-requests'] }); qc.invalidateQueries({ queryKey: ['my-balances'] }); } });

  return (
    <div className="space-y-6">
      <div>
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-sm font-semibold text-gray-700">Leave balances</h2>
          <button className="btn-primary" onClick={() => setShowNew(true)}><Plus className="w-4 h-4" /> Request Leave</button>
        </div>
        <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
          {balances.map(b => (
            <div key={b.id} className="card p-4">
              <p className="text-xs text-gray-500">{typeName(b.leave_type_id)}</p>
              <p className="text-2xl font-bold text-indigo-600">{avail(b).toFixed(1)}</p>
              <p className="text-[11px] text-gray-400">days available · {b.taken_days} taken · {b.pending_days} pending</p>
            </div>
          ))}
        </div>
      </div>
      <div>
        <h2 className="text-sm font-semibold text-gray-700 mb-2">My requests</h2>
        <div className="card overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-gray-50 text-xs text-gray-500 uppercase"><tr><th className="text-left px-4 py-2.5">Type</th><th className="text-left px-4 py-2.5">Dates</th><th className="text-right px-4 py-2.5">Days</th><th className="text-left px-4 py-2.5">Status</th><th className="px-4 py-2.5"></th></tr></thead>
            <tbody className="divide-y divide-gray-100">
              {requests.map(r => (
                <tr key={r.id}>
                  <td className="px-4 py-2.5">{typeName(r.leave_type_id)}</td>
                  <td className="px-4 py-2.5 text-gray-600">{r.start_date} → {r.end_date}</td>
                  <td className="px-4 py-2.5 text-right">{r.working_days}</td>
                  <td className="px-4 py-2.5"><span className={`px-2 py-0.5 rounded-full text-xs font-medium ${statusColor(r.status)}`}>{r.status}</span></td>
                  <td className="px-4 py-2.5 text-right">{(r.status === 'Pending' || r.status === 'Approved') && <button onClick={() => cancelMut.mutate(r.id)} className="text-xs text-red-600 hover:underline">Cancel</button>}</td>
                </tr>
              ))}
              {requests.length === 0 && <tr><td colSpan={5} className="px-4 py-8 text-center text-gray-400">No requests yet</td></tr>}
            </tbody>
          </table>
        </div>
      </div>
      {showNew && <RequestModal types={types} onClose={() => setShowNew(false)} onSaved={() => { qc.invalidateQueries({ queryKey: ['my-requests'] }); qc.invalidateQueries({ queryKey: ['my-balances'] }); setShowNew(false); }} />}
      <UpcomingHolidays />
    </div>
  );
}

function UpcomingHolidays() {
  const { data: holidays = [] } = useQuery<any[]>({ queryKey: ['staff-holidays'], queryFn: () => staffGetHolidays().then(r => r.data).catch(() => []) });
  const today = new Date().toISOString().split('T')[0];
  // Expand recurring to this year for display; show upcoming, sorted.
  const yr = new Date().getFullYear();
  const upcoming = holidays.map(h => {
    let date = h.date;
    if (h.recurring) { const [, m, d] = h.date.split('-'); date = `${yr}-${m}-${d}`; if (date < today) date = `${yr + 1}-${m}-${d}`; }
    return { name: h.name, date };
  }).filter(h => h.date >= today).sort((a, b) => a.date < b.date ? -1 : 1).slice(0, 6);
  if (upcoming.length === 0) return null;
  return (
    <div>
      <h2 className="text-sm font-semibold text-gray-700 mb-2">Upcoming public holidays</h2>
      <div className="card divide-y divide-gray-100">
        {upcoming.map((h, i) => (
          <div key={i} className="flex items-center gap-3 px-4 py-2 text-sm">
            <span className="text-gray-400 w-24">{h.date}</span>
            <span className="font-medium text-gray-800">{h.name}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function typeById(types: any[], id: string) { return types.find(t => t.id === id)?.name ?? id.slice(0, 8); }

function RequestModal({ types, onClose, onSaved }: { types: any[]; onClose: () => void; onSaved: () => void }) {
  const today = new Date().toISOString().split('T')[0];
  const [form, setForm] = useState({ leave_type_id: '', start_date: today, end_date: today, reason: '' });
  const [err, setErr] = useState('');
  const mut = useMutation({ mutationFn: () => staffCreateLeaveRequest(form), onSuccess: onSaved, onError: (e: any) => setErr(e?.response?.data?.error ?? 'Failed') });
  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 p-4" onClick={onClose}>
      <div className="bg-white rounded-xl p-6 w-full max-w-md" onClick={e => e.stopPropagation()}>
        <h3 className="font-semibold text-gray-800 mb-4">Request Leave</h3>
        <div className="space-y-3">
          {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
          <div><label className="label">Leave type</label>
            <select className="input" value={form.leave_type_id} onChange={e => setForm({ ...form, leave_type_id: e.target.value })}>
              <option value="">Choose…</option>
              {types.filter(t => t.active).map(t => <option key={t.id} value={t.id}>{t.name}</option>)}
            </select>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div><label className="label">Start</label><input type="date" className="input" value={form.start_date} onChange={e => setForm({ ...form, start_date: e.target.value })} /></div>
            <div><label className="label">End</label><input type="date" className="input" value={form.end_date} onChange={e => setForm({ ...form, end_date: e.target.value })} /></div>
          </div>
          <div><label className="label">Reason</label><input className="input" value={form.reason} onChange={e => setForm({ ...form, reason: e.target.value })} placeholder="Optional" /></div>
          <div className="flex justify-end gap-2 pt-2">
            <button className="btn-secondary" onClick={onClose}>Cancel</button>
            <button className="btn-primary" disabled={!form.leave_type_id || mut.isPending} onClick={() => { setErr(''); mut.mutate(); }}>{mut.isPending ? 'Submitting…' : 'Submit'}</button>
          </div>
        </div>
      </div>
    </div>
  );
}

function MyPayslips() {
  const { data: payslips = [] } = useQuery<any[]>({ queryKey: ['my-payslips'], queryFn: () => staffGetPayslips().then(r => r.data) });
  const download = async (runId: string) => { const r = await staffGetPayslipPdf(runId); window.open(URL.createObjectURL(r.data), '_blank'); };
  return (
    <div className="card overflow-hidden">
      <table className="w-full text-sm">
        <thead className="bg-gray-50 text-xs text-gray-500 uppercase"><tr><th className="text-left px-4 py-2.5">Pay date</th><th className="text-left px-4 py-2.5">Status</th><th className="text-left px-4 py-2.5">Deductions</th><th className="px-4 py-2.5"></th></tr></thead>
        <tbody className="divide-y divide-gray-100">
          {payslips.map((p, i) => (
            <tr key={i}>
              <td className="px-4 py-2.5 font-medium text-gray-800">{p.pay_date}</td>
              <td className="px-4 py-2.5"><span className="px-2 py-0.5 rounded-full text-xs bg-green-100 text-green-700">{p.status}</span></td>
              <td className="px-4 py-2.5 text-gray-500 text-xs">{p.deductions ? Object.keys(p.deductions).length + ' items' : '—'}</td>
              <td className="px-4 py-2.5 text-right"><button onClick={() => download(p.pay_run_id)} className="text-indigo-600 text-xs hover:underline">Download PDF</button></td>
            </tr>
          ))}
          {payslips.length === 0 && <tr><td colSpan={4} className="px-4 py-8 text-center text-gray-400">No payslips available yet</td></tr>}
        </tbody>
      </table>
    </div>
  );
}

function MyProfile() {
  const qc = useQueryClient();
  const { data: profile } = useQuery<any>({ queryKey: ['my-profile'], queryFn: () => staffGetProfile().then(r => r.data) });
  const [form, setForm] = useState<{ phone: string; personal_email: string } | null>(null);
  useEffect(() => { if (profile && !form) setForm({ phone: profile.phone ?? '', personal_email: profile.personal_email ?? '' }); }, [profile]);
  const saveMut = useMutation({ mutationFn: () => staffUpdateProfile(form!), onSuccess: () => qc.invalidateQueries({ queryKey: ['my-profile'] }) });
  if (!profile || !form) return <div className="text-gray-400">Loading…</div>;
  return (
    <div className="card p-6 max-w-lg space-y-4">
      <div className="grid grid-cols-2 gap-4 text-sm">
        <Field label="Name" value={profile.full_name} />
        <Field label="Staff number" value={profile.staff_number} />
        <Field label="Job title" value={profile.job_title ?? '—'} />
        <Field label="Department" value={profile.department ?? '—'} />
        <Field label="Basic salary" value={`KES ${Number(profile.basic_salary).toLocaleString()}`} />
        <Field label="KRA PIN" value={profile.kra_pin} />
      </div>
      <div className="border-t border-gray-100 pt-4">
        <p className="text-xs text-gray-500 mb-2">You can update your contact details. Payroll fields are managed by HR.</p>
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Phone</label><input className="input" value={form.phone} onChange={e => setForm({ ...form, phone: e.target.value })} /></div>
          <div><label className="label">Personal email</label><input className="input" value={form.personal_email} onChange={e => setForm({ ...form, personal_email: e.target.value })} /></div>
        </div>
        <button className="btn-primary mt-3" disabled={saveMut.isPending} onClick={() => saveMut.mutate()}>{saveMut.isPending ? 'Saving…' : 'Save contact details'}</button>
      </div>
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return <div><p className="text-xs text-gray-400">{label}</p><p className="font-medium text-gray-800">{value}</p></div>;
}
