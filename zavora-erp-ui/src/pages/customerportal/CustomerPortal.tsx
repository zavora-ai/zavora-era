import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  bootstrapCustomerAuth, getCustomerToken, getCustomerIdentity, clearCustomerSession, customerLogout,
  customerGetInvoices, customerGetStatement, customerGetProfile, customerUpdateProfile,
  customerGetTickets, customerCreateTicket, customerGetTicket, customerReplyTicket,
} from '../../api/customerClient';
import { formatCurrency, formatDate } from '../../utils/format';
import { Building2, Receipt, LifeBuoy, UserCircle, LogOut, Plus } from 'lucide-react';

type View = 'overview' | 'support' | 'profile';

/** Self-service shell for customers — separate principal, own session. */
export default function CustomerPortal() {
  const navigate = useNavigate();
  const [ready, setReady] = useState(false);
  const [authed, setAuthed] = useState(false);
  const [view, setView] = useState<View>('overview');

  useEffect(() => {
    (async () => {
      const ok = getCustomerToken() != null || (await bootstrapCustomerAuth());
      setAuthed(ok); setReady(true);
      if (!ok) navigate('/customerportal/login', { replace: true });
    })();
  }, [navigate]);

  if (!ready) return <div className="min-h-screen flex items-center justify-center text-gray-400">Loading…</div>;
  if (!authed) return null;

  const identity = getCustomerIdentity();
  const logout = async () => { try { await customerLogout(); } catch { /* ignore */ } clearCustomerSession(); navigate('/customerportal/login', { replace: true }); };

  const nav: [View, string, any][] = [
    ['overview', 'Invoices & Statement', Receipt],
    ['support', 'Support', LifeBuoy],
    ['profile', 'Profile', UserCircle],
  ];

  return (
    <div className="min-h-screen bg-gray-50">
      <header className="bg-white border-b border-gray-200">
        <div className="max-w-4xl mx-auto px-4 h-14 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center">
              <Building2 className="w-4 h-4 text-white" />
            </div>
            <span className="font-semibold text-gray-800 text-sm">Customer Portal</span>
          </div>
          <div className="flex items-center gap-3">
            <span className="text-sm text-gray-500">{identity?.display_name ?? identity?.email}</span>
            <button onClick={logout} className="text-sm text-red-600 flex items-center gap-1 hover:bg-red-50 px-2 py-1 rounded"><LogOut className="w-4 h-4" /> Sign out</button>
          </div>
        </div>
      </header>

      <div className="max-w-4xl mx-auto px-4 py-6">
        <div className="flex gap-1 border-b border-gray-200 mb-5">
          {nav.map(([k, label, Icon]) => (
            <button key={k} onClick={() => setView(k)}
              className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px flex items-center gap-1.5 ${
                view === k ? 'border-indigo-500 text-indigo-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}>
              <Icon className="w-4 h-4" /> {label}
            </button>
          ))}
        </div>
        {view === 'overview' && <Overview />}
        {view === 'support' && <Support />}
        {view === 'profile' && <Profile />}
      </div>
    </div>
  );
}

const invStatusColor = (s: string) =>
  s === 'paid' ? 'bg-green-100 text-green-700'
  : s === 'voided' ? 'bg-gray-100 text-gray-600'
  : s === 'overdue' ? 'bg-red-100 text-red-700'
  : 'bg-amber-100 text-amber-700';

function Overview() {
  const { data: statement } = useQuery<any>({ queryKey: ['cust-statement'], queryFn: () => customerGetStatement().then(r => r.data) });
  const { data: invoices } = useQuery<any>({ queryKey: ['cust-invoices'], queryFn: () => customerGetInvoices().then(r => r.data) });
  const list: any[] = invoices?.invoices ?? [];
  const linked = statement?.linked ?? false;

  return (
    <div className="space-y-6">
      {!linked && (
        <div className="bg-amber-50 text-amber-800 text-sm px-4 py-3 rounded-lg">
          Your portal account isn't linked to a billing account yet. Once our team links it, your invoices and statement will appear here.
        </div>
      )}
      <div className="grid grid-cols-2 gap-3">
        <div className="card p-4">
          <p className="text-xs text-gray-500">Outstanding balance</p>
          <p className="text-2xl font-bold text-indigo-600">{formatCurrency(statement?.outstanding ?? 0, 'KES')}</p>
        </div>
        <div className="card p-4">
          <p className="text-xs text-gray-500">Open invoices</p>
          <p className="text-2xl font-bold text-gray-800">{statement?.open_invoices ?? 0}</p>
        </div>
      </div>
      <div>
        <h2 className="text-sm font-semibold text-gray-700 mb-2">Invoices</h2>
        <div className="card overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-gray-50 text-xs text-gray-500 uppercase"><tr>
              <th className="text-left px-4 py-2.5">Invoice</th><th className="text-left px-4 py-2.5">Issued</th>
              <th className="text-left px-4 py-2.5">Due</th><th className="text-right px-4 py-2.5">Total</th>
              <th className="text-right px-4 py-2.5">Balance</th><th className="text-left px-4 py-2.5">Status</th>
            </tr></thead>
            <tbody className="divide-y divide-gray-100">
              {list.map((inv) => (
                <tr key={inv.id}>
                  <td className="px-4 py-2.5 font-medium text-gray-800">{inv.invoice_number ?? '—'}</td>
                  <td className="px-4 py-2.5 text-gray-600">{inv.issue_date ? formatDate(inv.issue_date) : '—'}</td>
                  <td className="px-4 py-2.5 text-gray-600">{inv.due_date ? formatDate(inv.due_date) : '—'}</td>
                  <td className="px-4 py-2.5 text-right">{formatCurrency(inv.gross_total, inv.currency || 'KES')}</td>
                  <td className="px-4 py-2.5 text-right">{formatCurrency(inv.balance_due ?? 0, inv.currency || 'KES')}</td>
                  <td className="px-4 py-2.5"><span className={`px-2 py-0.5 rounded-full text-xs font-medium ${invStatusColor(inv.status)}`}>{inv.status}</span></td>
                </tr>
              ))}
              {list.length === 0 && <tr><td colSpan={6} className="px-4 py-8 text-center text-gray-400">No invoices yet</td></tr>}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

const ticketStatusColor = (s: string) =>
  s === 'Resolved' || s === 'Closed' ? 'bg-green-100 text-green-700'
  : s === 'Open' ? 'bg-amber-100 text-amber-700'
  : 'bg-indigo-100 text-indigo-700';

function Support() {
  const qc = useQueryClient();
  const [showNew, setShowNew] = useState(false);
  const [openId, setOpenId] = useState<string | null>(null);
  const { data: tickets = [] } = useQuery<any[]>({ queryKey: ['cust-tickets'], queryFn: () => customerGetTickets().then(r => r.data) });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-gray-700">Support tickets</h2>
        <button className="btn-primary" onClick={() => setShowNew(true)}><Plus className="w-4 h-4" /> New Ticket</button>
      </div>
      <div className="card overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-gray-50 text-xs text-gray-500 uppercase"><tr>
            <th className="text-left px-4 py-2.5">Subject</th><th className="text-left px-4 py-2.5">Priority</th>
            <th className="text-left px-4 py-2.5">Status</th><th className="text-left px-4 py-2.5">Updated</th>
          </tr></thead>
          <tbody className="divide-y divide-gray-100">
            {tickets.map((t) => (
              <tr key={t.id} className="hover:bg-gray-50 cursor-pointer" onClick={() => setOpenId(t.id)}>
                <td className="px-4 py-2.5 font-medium text-gray-800">{t.subject}</td>
                <td className="px-4 py-2.5 text-gray-600">{t.priority}</td>
                <td className="px-4 py-2.5"><span className={`px-2 py-0.5 rounded-full text-xs font-medium ${ticketStatusColor(t.status)}`}>{t.status}</span></td>
                <td className="px-4 py-2.5 text-gray-500">{formatDate(t.updated_at)}</td>
              </tr>
            ))}
            {tickets.length === 0 && <tr><td colSpan={4} className="px-4 py-8 text-center text-gray-400">No tickets yet</td></tr>}
          </tbody>
        </table>
      </div>
      {showNew && <NewTicketModal onClose={() => setShowNew(false)} onSaved={() => { qc.invalidateQueries({ queryKey: ['cust-tickets'] }); setShowNew(false); }} />}
      {openId && <TicketModal id={openId} onClose={() => setOpenId(null)} />}
    </div>
  );
}

function NewTicketModal({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const [form, setForm] = useState({ subject: '', description: '', priority: 'Normal' });
  const [err, setErr] = useState('');
  const mut = useMutation({ mutationFn: () => customerCreateTicket(form), onSuccess: onSaved, onError: (e: any) => setErr(e?.response?.data?.error ?? 'Failed') });
  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 p-4" onClick={onClose}>
      <div className="bg-white rounded-xl p-6 w-full max-w-md" onClick={e => e.stopPropagation()}>
        <h3 className="font-semibold text-gray-800 mb-4">New support ticket</h3>
        <div className="space-y-3">
          {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
          <div><label className="label">Subject</label><input className="input" value={form.subject} onChange={e => setForm({ ...form, subject: e.target.value })} /></div>
          <div><label className="label">Priority</label>
            <select className="input" value={form.priority} onChange={e => setForm({ ...form, priority: e.target.value })}>
              {['Low', 'Normal', 'High', 'Urgent'].map(p => <option key={p} value={p}>{p}</option>)}
            </select>
          </div>
          <div><label className="label">Describe the issue</label><textarea className="input" rows={3} value={form.description} onChange={e => setForm({ ...form, description: e.target.value })} /></div>
          <div className="flex justify-end gap-2 pt-2">
            <button className="btn-secondary" onClick={onClose}>Cancel</button>
            <button className="btn-primary" disabled={!form.subject.trim() || mut.isPending} onClick={() => { setErr(''); mut.mutate(); }}>{mut.isPending ? 'Submitting…' : 'Submit'}</button>
          </div>
        </div>
      </div>
    </div>
  );
}

function TicketModal({ id, onClose }: { id: string; onClose: () => void }) {
  const qc = useQueryClient();
  const [reply, setReply] = useState('');
  const { data } = useQuery<any>({ queryKey: ['cust-ticket', id], queryFn: () => customerGetTicket(id).then(r => r.data) });
  const mut = useMutation({ mutationFn: () => customerReplyTicket(id, reply), onSuccess: () => { setReply(''); qc.invalidateQueries({ queryKey: ['cust-ticket', id] }); qc.invalidateQueries({ queryKey: ['cust-tickets'] }); } });
  const ticket = data?.ticket;
  const messages: any[] = data?.messages ?? [];
  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 p-4" onClick={onClose}>
      <div className="bg-white rounded-xl p-6 w-full max-w-lg max-h-[85vh] flex flex-col" onClick={e => e.stopPropagation()}>
        <h3 className="font-semibold text-gray-800">{ticket?.subject ?? 'Ticket'}</h3>
        <p className="text-xs text-gray-500 mb-3">{ticket?.status} · {ticket?.priority}</p>
        <div className="flex-1 overflow-y-auto space-y-2 border-y border-gray-100 py-3">
          {ticket?.description && <div className="text-sm text-gray-700 bg-gray-50 rounded-lg p-3">{ticket.description}</div>}
          {messages.map((m) => (
            <div key={m.id} className={`text-sm rounded-lg p-3 ${m.author_kind === 'customer' ? 'bg-indigo-50 ml-6' : 'bg-gray-50 mr-6'}`}>
              <p className="text-[11px] text-gray-400 mb-0.5">{m.author_kind === 'customer' ? 'You' : 'Support'} · {formatDate(m.created_at)}</p>
              {m.body}
            </div>
          ))}
          {messages.length === 0 && !ticket?.description && <p className="text-sm text-gray-400 text-center py-4">No messages yet</p>}
        </div>
        <div className="flex gap-2 pt-3">
          <input className="input flex-1" placeholder="Write a reply…" value={reply} onChange={e => setReply(e.target.value)} onKeyDown={e => { if (e.key === 'Enter' && reply.trim()) mut.mutate(); }} />
          <button className="btn-primary" disabled={!reply.trim() || mut.isPending} onClick={() => mut.mutate()}>Send</button>
        </div>
      </div>
    </div>
  );
}

function Profile() {
  const qc = useQueryClient();
  const { data: profile } = useQuery<any>({ queryKey: ['cust-profile'], queryFn: () => customerGetProfile().then(r => r.data) });
  const [name, setName] = useState<string | null>(null);
  useEffect(() => { if (profile && name === null) setName(profile.display_name ?? ''); }, [profile]);
  const saveMut = useMutation({ mutationFn: () => customerUpdateProfile({ display_name: name ?? '' }), onSuccess: () => qc.invalidateQueries({ queryKey: ['cust-profile'] }) });
  if (!profile || name === null) return <div className="text-gray-400">Loading…</div>;
  return (
    <div className="card p-6 max-w-lg space-y-4">
      <div className="grid grid-cols-2 gap-4 text-sm">
        <div><p className="text-xs text-gray-400">Email</p><p className="font-medium text-gray-800">{profile.email}</p></div>
        <div><p className="text-xs text-gray-400">Billing account</p><p className="font-medium text-gray-800">{profile.customer_name ?? (profile.linked ? '—' : 'Not linked')}</p></div>
      </div>
      <div className="border-t border-gray-100 pt-4">
        <div><label className="label">Display name</label><input className="input" value={name} onChange={e => setName(e.target.value)} /></div>
        <button className="btn-primary mt-3" disabled={saveMut.isPending} onClick={() => saveMut.mutate()}>{saveMut.isPending ? 'Saving…' : 'Save'}</button>
      </div>
    </div>
  );
}
