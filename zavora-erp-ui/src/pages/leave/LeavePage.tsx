import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getLeaveTypes, createLeaveType, setLeaveTypeActive,
  getHolidays, createHoliday, deleteHoliday, getLeaveCalendar,
  getLeaveRequests, approveLeave, declineLeave, createLeaveRequest,
  getLeaveBalances, getEmployees,
} from '../../api/client';
import { usePermissions } from '../../hooks/usePermissions';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import { workToday } from '../../utils/workDate';
import { CheckCircle, XCircle, Plus, Trash2, CalendarDays } from 'lucide-react';

type Tab = 'requests' | 'calendar' | 'balances' | 'types' | 'holidays';

const statusColor = (s: string) =>
  s === 'Approved' ? 'bg-green-100 text-green-700'
  : s === 'Declined' ? 'bg-red-100 text-red-700'
  : s === 'Cancelled' ? 'bg-gray-100 text-gray-600'
  : 'bg-amber-100 text-amber-700';

export default function LeavePage() {
  const [tab, setTab] = useState<Tab>('requests');
  const { can } = usePermissions();
  const canManage = can('leave.create');
  const canApprove = can('leave.approve');

  return (
    <div>
      <PageHeader title="Leave" subtitle="Requests, balances, leave types and holidays" />
      <div className="flex gap-1 border-b border-gray-200 mb-5 overflow-x-auto">
        {([['requests','Requests'],['calendar','Calendar'],['balances','Balances'],['types','Leave Types'],['holidays','Holidays']] as [Tab,string][])
          .map(([k, label]) => (
          <button key={k} onClick={() => setTab(k)}
            className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors whitespace-nowrap shrink-0 ${
              tab === k ? 'border-indigo-500 text-indigo-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}>
            {label}
          </button>
        ))}
      </div>
      {tab === 'requests' && <RequestsTab canApprove={canApprove} canManage={canManage} />}
      {tab === 'calendar' && <CalendarTab />}
      {tab === 'balances' && <BalancesTab />}
      {tab === 'types' && <TypesTab canManage={canManage} />}
      {tab === 'holidays' && <HolidaysTab canManage={canManage} />}
    </div>
  );
}

// ─── Requests ────────────────────────────────────────────────────────────────

function RequestsTab({ canApprove, canManage }: { canApprove: boolean; canManage: boolean }) {
  const qc = useQueryClient();
  const [filter, setFilter] = useState('');
  const [mine, setMine] = useState(false);
  const [showNew, setShowNew] = useState(false);
  const { data: requests = [] } = useQuery<any[]>({ queryKey: ['leave-requests', mine], queryFn: () => getLeaveRequests(mine ? { mine: true } : {}).then(r => r.data) });
  const { data: employees = [] } = useQuery<any[]>({ queryKey: ['employees'], queryFn: () => getEmployees().then(r => r.data) });
  const { data: types = [] } = useQuery<any[]>({ queryKey: ['leave-types'], queryFn: () => getLeaveTypes().then(r => r.data) });

  const empName = (id: string) => employees.find(e => e.id === id)?.full_name ?? id.slice(0, 8);
  const typeName = (id: string) => types.find(t => t.id === id)?.name ?? id.slice(0, 8);

  const approveMut = useMutation({ mutationFn: (id: string) => approveLeave(id), onSuccess: () => qc.invalidateQueries({ queryKey: ['leave-requests'] }) });
  const declineMut = useMutation({ mutationFn: (id: string) => declineLeave(id), onSuccess: () => qc.invalidateQueries({ queryKey: ['leave-requests'] }) });

  const filtered = filter ? requests.filter(r => r.status === filter) : requests;

  return (
    <div>
      <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
        <div className="flex gap-1">
          {['', 'Pending', 'Approved', 'Declined'].map(s => (
            <button key={s || 'all'} onClick={() => setFilter(s)}
              className={`px-3 py-1 text-xs rounded-full ${filter === s ? 'bg-indigo-600 text-white' : 'bg-gray-100 text-gray-600'}`}>
              {s || 'All'}
            </button>
          ))}
          {canApprove && (
            <button onClick={() => setMine(m => !m)}
              className={`px-3 py-1 text-xs rounded-full ${mine ? 'bg-indigo-600 text-white' : 'bg-gray-100 text-gray-600'}`}>
              Assigned to me
            </button>
          )}
        </div>
        {canManage && <button className="btn-primary" onClick={() => setShowNew(true)}><Plus className="w-4 h-4" /> New Request</button>}
      </div>
      <div className="card overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="bg-gray-50 text-xs text-gray-500 uppercase">
            <tr>
              <th className="text-left px-4 py-2.5">Employee</th>
              <th className="text-left px-4 py-2.5">Type</th>
              <th className="text-left px-4 py-2.5">Dates</th>
              <th className="text-right px-4 py-2.5">Days</th>
              <th className="text-left px-4 py-2.5">Status</th>
              <th className="text-right px-4 py-2.5">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100">
            {filtered.map(r => (
              <tr key={r.id} className="hover:bg-gray-50">
                <td className="px-4 py-2.5 font-medium text-gray-800">{empName(r.employee_id)}</td>
                <td className="px-4 py-2.5">{typeName(r.leave_type_id)}</td>
                <td className="px-4 py-2.5 text-gray-600">{r.start_date} → {r.end_date}</td>
                <td className="px-4 py-2.5 text-right">{r.working_days}</td>
                <td className="px-4 py-2.5"><span className={`px-2 py-0.5 rounded-full text-xs font-medium ${statusColor(r.status)}`}>{r.status}</span></td>
                <td className="px-4 py-2.5 text-right">
                  {r.status === 'Pending' && canApprove && (
                    <div className="flex gap-1 justify-end">
                      <button onClick={() => approveMut.mutate(r.id)} className="text-green-600 hover:bg-green-50 p-1 rounded" title="Approve"><CheckCircle className="w-4 h-4" /></button>
                      <button onClick={() => declineMut.mutate(r.id)} className="text-red-600 hover:bg-red-50 p-1 rounded" title="Decline"><XCircle className="w-4 h-4" /></button>
                    </div>
                  )}
                </td>
              </tr>
            ))}
            {filtered.length === 0 && <tr><td colSpan={6} className="px-4 py-8 text-center text-gray-400">No leave requests</td></tr>}
          </tbody>
        </table>
      </div>
      {showNew && <NewRequestModal employees={employees} types={types} onClose={() => setShowNew(false)}
        onSaved={() => { qc.invalidateQueries({ queryKey: ['leave-requests'] }); setShowNew(false); }} />}
    </div>
  );
}

function NewRequestModal({ employees, types, onClose, onSaved }: { employees: any[]; types: any[]; onClose: () => void; onSaved: () => void }) {
  const [form, setForm] = useState({ employee_id: '', leave_type_id: '', start_date: workToday(), end_date: workToday(), reason: '' });
  const [err, setErr] = useState('');
  const mut = useMutation({
    mutationFn: () => createLeaveRequest(form),
    onSuccess: onSaved,
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Failed to create request'),
  });
  return (
    <Modal open onClose={onClose} title="New Leave Request" size="md">
      <div className="space-y-3">
        {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
        <div>
          <label className="label">Employee</label>
          <select className="input" value={form.employee_id} onChange={e => setForm({ ...form, employee_id: e.target.value })}>
            <option value="">Choose…</option>
            {employees.map(e => <option key={e.id} value={e.id}>{e.full_name}</option>)}
          </select>
        </div>
        <div>
          <label className="label">Leave Type</label>
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
          <button className="btn-primary" disabled={!form.employee_id || !form.leave_type_id || mut.isPending} onClick={() => { setErr(''); mut.mutate(); }}>
            {mut.isPending ? 'Saving…' : 'Submit'}
          </button>
        </div>
      </div>
    </Modal>
  );
}

// ─── Calendar ────────────────────────────────────────────────────────────────

function CalendarTab() {
  const [month, setMonth] = useState(() => { const d = new Date(); return new Date(d.getFullYear(), d.getMonth(), 1); });
  const from = new Date(month.getFullYear(), month.getMonth(), 1).toISOString().split('T')[0];
  const to = new Date(month.getFullYear(), month.getMonth() + 1, 0).toISOString().split('T')[0];
  const { data } = useQuery<any>({ queryKey: ['leave-calendar', from], queryFn: () => getLeaveCalendar(from, to).then(r => r.data) });
  const leave: any[] = data?.leave ?? [];
  const holidays: any[] = data?.holidays ?? [];
  const holidaySet = new Set(holidays.map(h => h.date));

  const daysInMonth = new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate();
  const monthLabel = month.toLocaleString('en', { month: 'long', year: 'numeric' });
  const dayHas = (day: number) => {
    const dstr = new Date(month.getFullYear(), month.getMonth(), day).toISOString().split('T')[0];
    return leave.filter(l => l.start_date <= dstr && l.end_date >= dstr);
  };

  return (
    <div>
      <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
        <div className="flex items-center gap-2">
          <button className="btn-secondary py-1" onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() - 1, 1))}>‹</button>
          <span className="font-medium text-gray-800 w-40 text-center">{monthLabel}</span>
          <button className="btn-secondary py-1" onClick={() => setMonth(new Date(month.getFullYear(), month.getMonth() + 1, 1))}>›</button>
        </div>
        <div className="text-xs text-gray-500 flex gap-3">
          <span><span className="inline-block w-3 h-3 rounded bg-indigo-200 align-middle"></span> Leave</span>
          <span><span className="inline-block w-3 h-3 rounded bg-rose-200 align-middle"></span> Holiday</span>
        </div>
      </div>
      <div className="grid grid-cols-7 gap-1">
        {['Mon','Tue','Wed','Thu','Fri','Sat','Sun'].map(d => <div key={d} className="text-center text-[11px] text-gray-400 font-medium py-1">{d}</div>)}
        {(() => {
          const firstDow = (new Date(month.getFullYear(), month.getMonth(), 1).getDay() + 6) % 7; // Mon=0
          const cells = [] as any[];
          for (let i = 0; i < firstDow; i++) cells.push(<div key={'e'+i} />);
          for (let day = 1; day <= daysInMonth; day++) {
            const dstr = new Date(month.getFullYear(), month.getMonth(), day).toISOString().split('T')[0];
            const isHol = holidaySet.has(dstr);
            const onLeave = dayHas(day);
            cells.push(
              <div key={day} className={`min-h-[64px] rounded border p-1 text-xs ${isHol ? 'bg-rose-50 border-rose-200' : 'bg-white border-gray-100'}`}>
                <div className="text-gray-400">{day}</div>
                {onLeave.slice(0,3).map((l, i) => (
                  <div key={i} className={`truncate rounded px-1 mt-0.5 ${l.status === 'Approved' ? 'bg-indigo-100 text-indigo-700' : 'bg-amber-100 text-amber-700'}`} title={`${l.employee_name} — ${l.leave_type} (${l.status})`}>
                    {l.employee_name.split(' ')[0]}
                  </div>
                ))}
                {onLeave.length > 3 && <div className="text-[10px] text-gray-400 mt-0.5">+{onLeave.length - 3} more</div>}
              </div>
            );
          }
          return cells;
        })()}
      </div>
    </div>
  );
}

// ─── Balances ────────────────────────────────────────────────────────────────

function BalancesTab() {
  const { data: employees = [] } = useQuery<any[]>({ queryKey: ['employees'], queryFn: () => getEmployees().then(r => r.data) });
  const { data: types = [] } = useQuery<any[]>({ queryKey: ['leave-types'], queryFn: () => getLeaveTypes().then(r => r.data) });
  const [emp, setEmp] = useState('');
  const { data: balances = [] } = useQuery<any[]>({
    queryKey: ['leave-balances', emp], enabled: !!emp,
    queryFn: () => getLeaveBalances(emp).then(r => r.data),
  });
  const typeName = (id: string) => types.find(t => t.id === id)?.name ?? id.slice(0, 8);
  const avail = (b: any) => Number(b.accrued_days) + Number(b.carried_over) - Number(b.taken_days) - Number(b.pending_days);

  return (
    <div>
      <select className="input max-w-xs mb-4" value={emp} onChange={e => setEmp(e.target.value)}>
        <option value="">Select an employee…</option>
        {employees.map(e => <option key={e.id} value={e.id}>{e.full_name}</option>)}
      </select>
      {emp && (
        <div className="card overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="bg-gray-50 text-xs text-gray-500 uppercase">
              <tr><th className="text-left px-4 py-2.5">Type</th><th className="text-right px-4 py-2.5">Entitled</th><th className="text-right px-4 py-2.5">Accrued</th><th className="text-right px-4 py-2.5">Taken</th><th className="text-right px-4 py-2.5">Pending</th><th className="text-right px-4 py-2.5">Available</th></tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {balances.map(b => (
                <tr key={b.id}>
                  <td className="px-4 py-2.5 font-medium text-gray-800">{typeName(b.leave_type_id)}</td>
                  <td className="px-4 py-2.5 text-right">{b.entitled_days}</td>
                  <td className="px-4 py-2.5 text-right">{b.accrued_days}</td>
                  <td className="px-4 py-2.5 text-right">{b.taken_days}</td>
                  <td className="px-4 py-2.5 text-right">{b.pending_days}</td>
                  <td className="px-4 py-2.5 text-right font-semibold text-indigo-600">{avail(b).toFixed(2)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ─── Leave types ─────────────────────────────────────────────────────────────

function TypesTab({ canManage }: { canManage: boolean }) {
  const qc = useQueryClient();
  const [showNew, setShowNew] = useState(false);
  const { data: types = [] } = useQuery<any[]>({ queryKey: ['leave-types'], queryFn: () => getLeaveTypes().then(r => r.data) });
  const activeMut = useMutation({ mutationFn: ({ id, active }: { id: string; active: boolean }) => setLeaveTypeActive(id, active), onSuccess: () => qc.invalidateQueries({ queryKey: ['leave-types'] }) });

  return (
    <div>
      {canManage && <div className="flex justify-end mb-3"><button className="btn-primary" onClick={() => setShowNew(true)}><Plus className="w-4 h-4" /> New Type</button></div>}
      <div className="card overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="bg-gray-50 text-xs text-gray-500 uppercase">
            <tr><th className="text-left px-4 py-2.5">Name</th><th className="text-left px-4 py-2.5">Code</th><th className="text-right px-4 py-2.5">Days/yr</th><th className="text-left px-4 py-2.5">Accrual</th><th className="text-left px-4 py-2.5">Paid</th><th className="text-left px-4 py-2.5">Active</th></tr>
          </thead>
          <tbody className="divide-y divide-gray-100">
            {types.map(t => (
              <tr key={t.id}>
                <td className="px-4 py-2.5 font-medium text-gray-800">{t.name} {t.is_statutory && <span className="text-[10px] text-gray-400">(statutory)</span>}</td>
                <td className="px-4 py-2.5 text-gray-500">{t.code}</td>
                <td className="px-4 py-2.5 text-right">{t.days_per_year}</td>
                <td className="px-4 py-2.5 text-gray-600">{t.accrual_method}</td>
                <td className="px-4 py-2.5">{t.paid ? 'Yes' : 'No'}</td>
                <td className="px-4 py-2.5">
                  <button disabled={!canManage} onClick={() => activeMut.mutate({ id: t.id, active: !t.active })}
                    className={`px-2 py-0.5 rounded-full text-xs font-medium ${t.active ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-gray-500'}`}>
                    {t.active ? 'Active' : 'Inactive'}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {showNew && <NewTypeModal onClose={() => setShowNew(false)} onSaved={() => { qc.invalidateQueries({ queryKey: ['leave-types'] }); setShowNew(false); }} />}
    </div>
  );
}

function NewTypeModal({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const [form, setForm] = useState({ name: '', code: '', days_per_year: 0, accrual_method: 'FixedAnnual', paid: true, requires_attachment: false });
  const mut = useMutation({ mutationFn: () => createLeaveType(form), onSuccess: onSaved });
  return (
    <Modal open onClose={onClose} title="New Leave Type" size="md">
      <div className="space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Name</label><input className="input" value={form.name} onChange={e => setForm({ ...form, name: e.target.value })} /></div>
          <div><label className="label">Code</label><input className="input" value={form.code} onChange={e => setForm({ ...form, code: e.target.value.toUpperCase() })} /></div>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Days per year</label><input type="number" className="input" value={form.days_per_year} onChange={e => setForm({ ...form, days_per_year: Number(e.target.value) })} /></div>
          <div><label className="label">Accrual</label>
            <select className="input" value={form.accrual_method} onChange={e => setForm({ ...form, accrual_method: e.target.value })}>
              <option value="FixedAnnual">Fixed annual</option><option value="MonthlyAccrual">Monthly accrual</option><option value="Unlimited">Unlimited</option>
            </select>
          </div>
        </div>
        <div className="flex gap-4">
          <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={form.paid} onChange={e => setForm({ ...form, paid: e.target.checked })} /> Paid</label>
          <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={form.requires_attachment} onChange={e => setForm({ ...form, requires_attachment: e.target.checked })} /> Requires attachment</label>
        </div>
        <div className="flex justify-end gap-2 pt-2">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!form.name || !form.code || mut.isPending} onClick={() => mut.mutate()}>Save</button>
        </div>
      </div>
    </Modal>
  );
}

// ─── Holidays ────────────────────────────────────────────────────────────────

function HolidaysTab({ canManage }: { canManage: boolean }) {
  const qc = useQueryClient();
  const [form, setForm] = useState({ date: workToday(), name: '', recurring: false });
  const { data: holidays = [] } = useQuery<any[]>({ queryKey: ['holidays'], queryFn: () => getHolidays().then(r => r.data) });
  const addMut = useMutation({ mutationFn: () => createHoliday(form), onSuccess: () => { qc.invalidateQueries({ queryKey: ['holidays'] }); setForm({ ...form, name: '' }); } });
  const delMut = useMutation({ mutationFn: (id: string) => deleteHoliday(id), onSuccess: () => qc.invalidateQueries({ queryKey: ['holidays'] }) });

  return (
    <div className="max-w-2xl">
      {canManage && (
        <div className="card p-4 mb-4 flex items-end gap-3">
          <div><label className="label">Date</label><input type="date" className="input" value={form.date} onChange={e => setForm({ ...form, date: e.target.value })} /></div>
          <div className="flex-1"><label className="label">Name</label><input className="input" value={form.name} onChange={e => setForm({ ...form, name: e.target.value })} placeholder="e.g. Jamhuri Day" /></div>
          <label className="flex items-center gap-1.5 text-sm pb-2"><input type="checkbox" checked={form.recurring} onChange={e => setForm({ ...form, recurring: e.target.checked })} /> Yearly</label>
          <button className="btn-primary" disabled={!form.name || addMut.isPending} onClick={() => addMut.mutate()}><Plus className="w-4 h-4" /> Add</button>
        </div>
      )}
      <div className="card overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="bg-gray-50 text-xs text-gray-500 uppercase"><tr><th className="text-left px-4 py-2.5">Date</th><th className="text-left px-4 py-2.5">Name</th><th className="text-left px-4 py-2.5">Recurring</th><th className="px-4 py-2.5"></th></tr></thead>
          <tbody className="divide-y divide-gray-100">
            {holidays.map(h => (
              <tr key={h.id}>
                <td className="px-4 py-2.5"><CalendarDays className="w-4 h-4 inline mr-2 text-gray-400" />{h.recurring ? new Date(h.date).toLocaleString('en', { month: 'short', day: 'numeric' }) : h.date}</td>
                <td className="px-4 py-2.5 font-medium text-gray-800">{h.name}</td>
                <td className="px-4 py-2.5 text-gray-500">{h.recurring ? 'Yearly' : '—'}</td>
                <td className="px-4 py-2.5 text-right">{canManage && <button onClick={() => delMut.mutate(h.id)} className="text-red-500 hover:bg-red-50 p-1 rounded"><Trash2 className="w-4 h-4" /></button>}</td>
              </tr>
            ))}
            {holidays.length === 0 && <tr><td colSpan={4} className="px-4 py-8 text-center text-gray-400">No holidays configured</td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );
}
