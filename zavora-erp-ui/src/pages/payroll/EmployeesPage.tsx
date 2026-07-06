import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getEmployees, createEmployeeApi, updateEmployee, getUsers, inviteEss, listEarningTypes } from '../../api/client';
import type { Employee } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import Attachments from '../../components/shared/Attachments';
import DepartmentSelect from '../../components/shared/DepartmentSelect';
import { Plus, Shield, Network, Users as UsersIcon } from 'lucide-react';

export default function EmployeesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [editing, setEditing] = useState<Employee | null>(null);
  const [view, setView] = useState<'list' | 'org'>('list');

  const { data: employees = [], isLoading } = useQuery<Employee[]>({
    queryKey: ['employees'],
    queryFn: () => getEmployees().then(r => Array.isArray(r.data) ? r.data : []),
  });

  const columns: Column<Employee>[] = [
    { key: 'staff_number', header: 'Staff #', render: (r) => <span className="font-mono text-sm">{r.staff_number}</span> },
    { key: 'full_name', header: 'Employee', render: (r) => (
      <div><p className="font-medium text-gray-900">{r.full_name}</p><p className="text-xs text-gray-500">{(r as any).job_title || r.employment_type}</p></div>
    )},
    { key: 'department', header: 'Department', render: (r) => <span className="text-sm text-gray-600">{(r as any).department || '—'}</span> },
    { key: 'basic_salary', header: 'Basic Salary', render: (r) => <span className="font-medium">{formatCurrency(r.basic_salary)}</span>, className: 'text-right' },
    { key: 'is_active', header: 'Status', render: (r) => <span className={r.is_active ? 'badge-success' : 'badge-gray'}>{r.is_active ? 'Active' : 'Inactive'}</span> },
    { key: 'start_date', header: 'Start Date', render: (r) => formatDate(r.start_date) },
  ];

  return (
    <div>
      <PageHeader
        title="Employees"
        subtitle={`${employees.length} employee${employees.length !== 1 ? 's' : ''}`}
        actions={
          <div className="flex gap-2">
            <div className="flex rounded-lg border border-gray-200 overflow-hidden">
              <button onClick={() => setView('list')} className={`px-3 py-1.5 text-sm flex items-center gap-1 ${view === 'list' ? 'bg-indigo-600 text-white' : 'bg-white text-gray-600'}`}><UsersIcon className="w-4 h-4" /> List</button>
              <button onClick={() => setView('org')} className={`px-3 py-1.5 text-sm flex items-center gap-1 ${view === 'org' ? 'bg-indigo-600 text-white' : 'bg-white text-gray-600'}`}><Network className="w-4 h-4" /> Org chart</button>
            </div>
            <button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> Add Employee</button>
          </div>
        }
      />
      {view === 'list' ? (
        <DataTable columns={columns} data={employees} loading={isLoading} onRowClick={(r) => setEditing(r)}
          emptyMessage="No employees yet. Add your first employee to start running payroll." />
      ) : (
        <OrgChart employees={employees} onSelect={(e) => setEditing(e)} />
      )}
      {showCreate && <EmployeeModal onClose={() => setShowCreate(false)} />}
      {editing && <EmployeeModal employee={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}

// ─── Org chart ───────────────────────────────────────────────────────────────

function OrgChart({ employees, onSelect }: { employees: Employee[]; onSelect: (e: Employee) => void }) {
  const byId = new Map(employees.map(e => [e.id, e]));
  const roots = employees.filter(e => !(e as any).manager_id || !byId.has((e as any).manager_id));
  const childrenOf = (id: string) => employees.filter(e => (e as any).manager_id === id);
  const Node = ({ e, depth }: { e: Employee; depth: number }) => (
    <div>
      <button onClick={() => onSelect(e)} className="flex items-center gap-2 py-1.5 hover:bg-gray-50 rounded px-2 w-full text-left" style={{ marginLeft: depth * 20 }}>
        <div className="w-7 h-7 rounded-full bg-indigo-100 text-indigo-700 flex items-center justify-center text-xs font-bold">{e.full_name.split(' ').map(p => p[0]).slice(0,2).join('')}</div>
        <div><p className="text-sm font-medium text-gray-800">{e.full_name}</p><p className="text-xs text-gray-400">{(e as any).job_title || e.employment_type}{(e as any).department ? ` · ${(e as any).department}` : ''}</p></div>
      </button>
      {childrenOf(e.id).map(c => <Node key={c.id} e={c} depth={depth + 1} />)}
    </div>
  );
  return (
    <div className="card p-4">
      {roots.length === 0 && <p className="text-gray-400 text-sm">No employees.</p>}
      {roots.map(e => <Node key={e.id} e={e} depth={0} />)}
    </div>
  );
}

// ─── Create / Edit modal ─────────────────────────────────────────────────────

type TabKey = 'personal' | 'salary' | 'bank' | 'org' | 'docs';

function EmployeeModal({ employee, onClose }: { employee?: Employee; onClose: () => void }) {
  const qc = useQueryClient();
  const isEdit = !!employee;
  const [tab, setTab] = useState<TabKey>('personal');
  const { data: employees = [] } = useQuery<Employee[]>({ queryKey: ['employees'], queryFn: () => getEmployees().then(r => Array.isArray(r.data) ? r.data : []) });
  const { data: users = [] } = useQuery<any[]>({ queryKey: ['users'], queryFn: () => getUsers().then(r => Array.isArray(r.data) ? r.data : (r.data?.data ?? [])) });
  const { data: earningTypes = [] } = useQuery<any[]>({ queryKey: ['earning-types'], queryFn: () => listEarningTypes().then(r => r.data) });

  const e = employee as any;
  const [allowances, setAllowances] = useState<any[]>((e?.allowances ?? []).map((a: any) => ({ name: a.name, amount: String(a.amount), taxable: a.taxable })));
  const [form, setForm] = useState({
    staff_number: e?.staff_number ?? '',
    full_name: e?.full_name ?? '',
    kra_pin: e?.kra_pin ?? '',
    nssf_number: e?.nssf_number ?? '',
    nhif_number: e?.nhif_number ?? '',
    helb_deduction: e?.helb_deduction?.toString() ?? '',
    employment_type: e?.employment_type ?? 'Permanent',
    basic_salary: e?.basic_salary?.toString() ?? '',
    bank_name: e?.bank_account?.bank_name ?? '',
    account_name: e?.bank_account?.account_name ?? '',
    bank_branch: e?.bank_account?.branch ?? '',
    account_number: e?.bank_account?.account_number ?? '',
    tax_relief: e?.tax_relief?.toString() ?? '2400',
    disability_exemption: e?.disability_exemption ?? false,
    start_date: e?.start_date ?? new Date().toISOString().split('T')[0],
    department: e?.department ?? '',
    department_id: e?.department_id ?? '',
    job_title: e?.job_title ?? '',
    manager_id: e?.manager_id ?? '',
    approver_user_id: e?.approver_user_id ?? '',
    phone: e?.phone ?? '',
    personal_email: e?.personal_email ?? '',
    is_active: e?.is_active ?? true,
  });
  const [err, setErr] = useState('');

  const payload = () => ({
    staff_number: form.staff_number,
    full_name: form.full_name,
    kra_pin: form.kra_pin,
    nssf_number: form.nssf_number || undefined,
    nhif_number: form.nhif_number || undefined,
    helb_deduction: form.helb_deduction ? parseFloat(form.helb_deduction) : undefined,
    employment_type: form.employment_type,
    basic_salary: parseFloat(form.basic_salary) || 0,
    allowances: allowances
      .filter(a => a.name && parseFloat(a.amount) > 0)
      .map(a => ({ name: a.name, amount: parseFloat(a.amount) || 0, taxable: !!a.taxable })),
    bank_account: form.account_number ? { bank_name: form.bank_name, account_name: form.account_name || form.full_name, branch: form.bank_branch, account_number: form.account_number } : undefined,
    tax_relief: parseFloat(form.tax_relief) || 2400,
    disability_exemption: form.disability_exemption,
    start_date: form.start_date,
    department: form.department || undefined,
    department_id: form.department_id || undefined,
    job_title: form.job_title || undefined,
    manager_id: form.manager_id || undefined,
    approver_user_id: form.approver_user_id || undefined,
    personal_email: form.personal_email || undefined,
    phone: form.phone || undefined,
    is_active: form.is_active,
  });

  const mutation = useMutation({
    mutationFn: (data: any) => isEdit ? updateEmployee(employee!.id, data) : createEmployeeApi(data),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['employees'] }); onClose(); },
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Save failed'),
  });

  const tabs: [TabKey, string][] = [['personal','Personal'],['salary','Salary & Deductions'],['bank','Bank'],['org','Organization']];
  if (isEdit) tabs.push(['docs','Documents']);

  return (
    <Modal open onClose={onClose} title={isEdit ? `Edit — ${employee!.full_name}` : 'Add Employee'} size="lg">
      <div className="flex gap-1 mb-5 border-b flex-wrap">
        {tabs.map(([t, label]) => (
          <button key={t} type="button" onClick={() => setTab(t)}
            className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px ${tab === t ? 'border-indigo-600 text-indigo-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}>{label}</button>
        ))}
      </div>
      {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded mb-3">{err}</div>}

      {tab === 'personal' && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div><label className="label">Staff Number *</label><input className="input font-mono" value={form.staff_number} onChange={e => setForm({ ...form, staff_number: e.target.value })} /></div>
            <div><label className="label">Employment Type</label><select className="input" value={form.employment_type} onChange={e => setForm({ ...form, employment_type: e.target.value })}><option>Permanent</option><option>Contract</option><option>Casual</option></select></div>
          </div>
          <div><label className="label">Full Name *</label><input className="input" value={form.full_name} onChange={e => setForm({ ...form, full_name: e.target.value })} /></div>
          <div className="grid grid-cols-2 gap-4">
            <div><label className="label">KRA PIN *</label><input className="input font-mono" value={form.kra_pin} onChange={e => setForm({ ...form, kra_pin: e.target.value.toUpperCase() })} maxLength={11} /></div>
            <div><label className="label">Start Date</label><input type="date" className="input" value={form.start_date} onChange={e => setForm({ ...form, start_date: e.target.value })} /></div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div><label className="label">NSSF Number</label><input className="input font-mono" value={form.nssf_number} onChange={e => setForm({ ...form, nssf_number: e.target.value })} /></div>
            <div><label className="label">NHIF/SHA Number</label><input className="input font-mono" value={form.nhif_number} onChange={e => setForm({ ...form, nhif_number: e.target.value })} /></div>
          </div>
        </div>
      )}

      {tab === 'salary' && (
        <div className="space-y-4">
          <div><label className="label">Basic Salary (KES/month) *</label><input type="number" className="input" value={form.basic_salary} onChange={e => setForm({ ...form, basic_salary: e.target.value })} /></div>
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-medium text-gray-700">Allowances & Earnings</h4>
            <button type="button" className="text-indigo-600 text-xs hover:underline" onClick={() => setAllowances([...allowances, { name: '', amount: '', taxable: true }])}>+ Add allowance</button>
          </div>
          {allowances.length === 0 && <p className="text-xs text-gray-400">No allowances. Add from your earning types (defined in Payroll Settings).</p>}
          {allowances.map((a, i) => (
            <div key={i} className="flex items-end gap-2">
              <div className="flex-1">
                <label className="label">Type / name</label>
                <input list="ee-earning-types" className="input" value={a.name} placeholder="e.g. Housing Allowance"
                  onChange={ev => { const t = (earningTypes as any[]).find(x => x.name === ev.target.value); setAllowances(allowances.map((x, j) => j === i ? { ...x, name: ev.target.value, taxable: t ? t.taxable : x.taxable } : x)); }} />
              </div>
              <div className="w-32"><label className="label">Amount</label><input type="number" className="input" value={a.amount} onChange={ev => setAllowances(allowances.map((x, j) => j === i ? { ...x, amount: ev.target.value } : x))} /></div>
              <label className="flex items-center gap-1 text-xs pb-2.5 whitespace-nowrap"><input type="checkbox" checked={a.taxable} onChange={ev => setAllowances(allowances.map((x, j) => j === i ? { ...x, taxable: ev.target.checked } : x))} /> Taxable</label>
              <button type="button" className="text-gray-400 hover:text-red-600 pb-2.5" onClick={() => setAllowances(allowances.filter((_, j) => j !== i))}>✕</button>
            </div>
          ))}
          <datalist id="ee-earning-types">{(earningTypes as any[]).map(t => <option key={t.id} value={t.name} />)}</datalist>
          <hr />
          <h4 className="text-sm font-medium text-gray-700 flex items-center gap-2"><Shield className="w-4 h-4" /> Tax & Deductions</h4>
          <div className="grid grid-cols-2 gap-4">
            <div><label className="label">HELB Monthly</label><input type="number" className="input" value={form.helb_deduction} onChange={e => setForm({ ...form, helb_deduction: e.target.value })} /></div>
            <div><label className="label">Personal Relief (KES/mo)</label><input type="number" className="input" value={form.tax_relief} onChange={e => setForm({ ...form, tax_relief: e.target.value })} /></div>
          </div>
          <label className="flex items-center gap-2 cursor-pointer text-sm"><input type="checkbox" checked={form.disability_exemption} onChange={e => setForm({ ...form, disability_exemption: e.target.checked })} /> Disability exemption</label>
        </div>
      )}

      {tab === 'bank' && (
        <div className="space-y-4">
          <div><label className="label">Bank Name</label><input className="input" value={form.bank_name} onChange={e => setForm({ ...form, bank_name: e.target.value })} /></div>
          <div><label className="label">Account Name</label><input className="input" value={form.account_name} onChange={e => setForm({ ...form, account_name: e.target.value })} placeholder="Defaults to full name" /></div>
          <div className="grid grid-cols-2 gap-4">
            <div><label className="label">Branch</label><input className="input" value={form.bank_branch} onChange={e => setForm({ ...form, bank_branch: e.target.value })} /></div>
            <div><label className="label">Account Number</label><input className="input font-mono" value={form.account_number} onChange={e => setForm({ ...form, account_number: e.target.value })} /></div>
          </div>
        </div>
      )}

      {tab === 'org' && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div><label className="label">Job Title</label><input className="input" value={form.job_title} onChange={e => setForm({ ...form, job_title: e.target.value })} /></div>
            <div><label className="label">Department</label>
              <DepartmentSelect value={form.department_id} onChange={(id, name) => setForm({ ...form, department_id: id, department: name })} />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div><label className="label">Manager</label>
              <select className="input" value={form.manager_id} onChange={e => setForm({ ...form, manager_id: e.target.value })}>
                <option value="">— None —</option>
                {employees.filter(x => x.id !== employee?.id).map(x => <option key={x.id} value={x.id}>{x.full_name}</option>)}
              </select>
            </div>
            <div><label className="label">Leave approver</label>
              <select className="input" value={form.approver_user_id} onChange={e => setForm({ ...form, approver_user_id: e.target.value })}>
                <option value="">— HR pool (default) —</option>
                {users.map(u => <option key={u.id} value={u.id}>{u.display_name} ({u.role})</option>)}
              </select>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div><label className="label">Phone</label><input className="input" value={form.phone} onChange={e => setForm({ ...form, phone: e.target.value })} /></div>
            <div><label className="label">Personal Email</label><input className="input" value={form.personal_email} onChange={e => setForm({ ...form, personal_email: e.target.value })} /></div>
          </div>
          <label className="flex items-center gap-2 cursor-pointer text-sm"><input type="checkbox" checked={form.is_active} onChange={e => setForm({ ...form, is_active: e.target.checked })} /> Active</label>
          {isEdit && <EssInvite employee={employee!} />}
        </div>
      )}

      {tab === 'docs' && isEdit && (
        <Attachments linkedType="employee" linkedId={employee!.id} label="Employee documents (contract, ID, certificates)" />
      )}

      <div className="flex justify-end gap-3 pt-6 mt-4 border-t">
        <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
        <button className="btn-primary" disabled={mutation.isPending || !form.full_name || !form.kra_pin || !form.staff_number} onClick={() => { setErr(''); mutation.mutate(payload()); }}>
          {mutation.isPending ? 'Saving…' : isEdit ? 'Save Changes' : 'Save Employee'}
        </button>
      </div>
    </Modal>
  );
}

function EssInvite({ employee }: { employee: Employee }) {
  const [email, setEmail] = useState((employee as any).personal_email ?? '');
  const [password, setPassword] = useState('');
  const [msg, setMsg] = useState('');
  const mut = useMutation({
    mutationFn: () => inviteEss(employee.id, email, password || undefined),
    onSuccess: (r) => setMsg(`Self-service ${r.data.status}${password ? ' — they can log in now' : ' — set a password to activate'}`),
    onError: (e: any) => setMsg(e?.response?.data?.error ?? 'Invite failed'),
  });
  return (
    <div className="mt-2 rounded-lg border border-gray-200 p-3 bg-gray-50">
      <p className="text-sm font-medium text-gray-700 mb-2">Employee Self-Service access</p>
      {msg && <p className="text-xs text-indigo-600 mb-2">{msg}</p>}
      <div className="flex items-end gap-2">
        <div className="flex-1"><label className="label">Work email</label><input className="input" value={email} onChange={e => setEmail(e.target.value)} placeholder="employee@company.com" /></div>
        <div className="flex-1"><label className="label">Initial password (optional)</label><input className="input" type="text" value={password} onChange={e => setPassword(e.target.value)} placeholder="Leave blank to invite only" /></div>
        <button className="btn-secondary" disabled={!email || mut.isPending} onClick={() => { setMsg(''); mut.mutate(); }}>{mut.isPending ? '…' : 'Grant access'}</button>
      </div>
    </div>
  );
}
