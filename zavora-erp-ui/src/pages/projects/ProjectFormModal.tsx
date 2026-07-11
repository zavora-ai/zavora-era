import { useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { createProject, updateProject, getCustomers, getAccounts, type Project } from '../../api/client';
import Modal from '../../components/shared/Modal';
import { useToast } from '../../components/toast/ToastProvider';
import { Plus, Trash2 } from 'lucide-react';

const n = (v: any) => Number(v ?? 0);
const XS = 'inline-flex items-center gap-1 text-xs font-medium px-2 py-1 rounded-md border border-gray-200 text-gray-600 hover:bg-gray-50';

const STATUSES = ['planning', 'active', 'on_hold', 'completed', 'closed'];
const BILLING = [
  ['time_and_materials', 'Time & materials'],
  ['fixed_fee', 'Fixed fee'],
  ['milestone', 'Milestone'],
  ['non_billable', 'Non-billable / grant'],
];

export default function ProjectFormModal({ project, onClose, onDone }: { project: Project | null; onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const editing = !!project;
  const { data: customers = [] } = useQuery<any[]>({ queryKey: ['customers'], queryFn: () => getCustomers().then((r) => Array.isArray(r.data) ? r.data : (r.data?.data ?? [])) });
  const { data: accounts = [] } = useQuery<any[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then((r) => Array.isArray(r.data) ? r.data : []) });
  const expenseAccounts = accounts.filter((a: any) => ['Expense', 'ContraExpense'].includes(a.account_type));

  const [f, setF] = useState({
    code: project?.code ?? '', name: project?.name ?? '', client_id: project?.client_id ?? '',
    donor: project?.donor ?? '', manager: project?.manager ?? '', status: project?.status ?? 'active',
    billing_method: project?.billing_method ?? 'time_and_materials', currency: project?.currency ?? 'KES',
    start_date: project?.start_date ?? '', end_date: project?.end_date ?? '', budget_amount: String(n(project?.budget_amount)),
    notes: project?.notes ?? '',
  });
  const [lines, setLines] = useState(
    project?.budget_lines?.length ? project.budget_lines.map((l) => ({ category: l.category, account_code: l.account_code ?? '', amount: String(n(l.amount)) }))
      : [{ category: '', account_code: '', amount: '' }]
  );
  const [tasks, setTasks] = useState(
    project?.tasks?.length ? project.tasks.map((t) => ({ name: t.name, budget_amount: String(n(t.budget_amount)) })) : [] as { name: string; budget_amount: string }[]
  );

  const payload = () => ({
    code: f.code.trim(), name: f.name.trim(), client_id: f.client_id || undefined, donor: f.donor || undefined,
    manager: f.manager || undefined, status: f.status, billing_method: f.billing_method, currency: f.currency,
    start_date: f.start_date || undefined, end_date: f.end_date || undefined, budget_amount: Number(f.budget_amount) || 0,
    notes: f.notes || undefined,
    budget_lines: lines.filter((l) => l.category.trim()).map((l) => ({ category: l.category.trim(), account_code: l.account_code || undefined, amount: Number(l.amount) || 0 })),
    tasks: tasks.filter((t) => t.name.trim()).map((t) => ({ name: t.name.trim(), budget_amount: Number(t.budget_amount) || 0 })),
  });
  const mut = useMutation({
    mutationFn: () => editing ? updateProject(project!.id, payload()) : createProject(payload()),
    onSuccess: () => { toast.success(editing ? 'Project updated.' : 'Project created.'); onDone(); },
    onError: (e: any) => toast.fromError(e, 'Could not save the project.'),
  });
  const valid = f.code.trim() && f.name.trim();

  return (
    <Modal open={true} onClose={onClose} title={editing ? `Edit ${project!.code}` : 'New project'} size="lg">
      <div className="space-y-4">
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Code *</label><input className="input" value={f.code} disabled={editing} onChange={(e) => setF({ ...f, code: e.target.value })} placeholder="e.g. GRANT-24 / SITE-A" /></div>
          <div><label className="label">Name *</label><input className="input" value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} /></div>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Client <span className="text-gray-400 font-normal">(customer)</span></label>
            <select className="input" value={f.client_id} onChange={(e) => setF({ ...f, client_id: e.target.value })}>
              <option value="">— none —</option>
              {customers.map((c: any) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </div>
          <div><label className="label">Donor / funder <span className="text-gray-400 font-normal">(NGO)</span></label><input className="input" value={f.donor} onChange={(e) => setF({ ...f, donor: e.target.value })} placeholder="e.g. USAID, County Govt" /></div>
        </div>
        <div className="grid grid-cols-3 gap-3">
          <div><label className="label">Status</label>
            <select className="input" value={f.status} onChange={(e) => setF({ ...f, status: e.target.value })}>{STATUSES.map((s) => <option key={s} value={s}>{s.replace('_', ' ')}</option>)}</select>
          </div>
          <div><label className="label">Billing method</label>
            <select className="input" value={f.billing_method} onChange={(e) => setF({ ...f, billing_method: e.target.value })}>{BILLING.map(([v, l]) => <option key={v} value={v}>{l}</option>)}</select>
          </div>
          <div><label className="label">Manager</label><input className="input" value={f.manager} onChange={(e) => setF({ ...f, manager: e.target.value })} /></div>
        </div>
        <div className="grid grid-cols-3 gap-3">
          <div><label className="label">Start date</label><input className="input" type="date" value={f.start_date} onChange={(e) => setF({ ...f, start_date: e.target.value })} /></div>
          <div><label className="label">End date</label><input className="input" type="date" value={f.end_date} onChange={(e) => setF({ ...f, end_date: e.target.value })} /></div>
          <div><label className="label">Total budget</label><input className="input" type="number" value={f.budget_amount} onChange={(e) => setF({ ...f, budget_amount: e.target.value })} /></div>
        </div>

        <div>
          <label className="label">Budget lines <span className="text-gray-400 font-normal">(by cost category — map to a GL account for budget-vs-actual)</span></label>
          <div className="space-y-2">
            {lines.map((l, i) => (
              <div key={i} className="flex gap-2 items-center">
                <input className="input flex-1" placeholder="Category (e.g. Labour, Materials)" value={l.category} onChange={(e) => setLines(lines.map((x, j) => j === i ? { ...x, category: e.target.value } : x))} />
                <select className="input flex-1" value={l.account_code} onChange={(e) => setLines(lines.map((x, j) => j === i ? { ...x, account_code: e.target.value } : x))}>
                  <option value="">— GL account (optional) —</option>
                  {expenseAccounts.map((a: any) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
                </select>
                <input className="input w-32" type="number" placeholder="Amount" value={l.amount} onChange={(e) => setLines(lines.map((x, j) => j === i ? { ...x, amount: e.target.value } : x))} />
                <button className="text-gray-400 hover:text-red-500" onClick={() => setLines(lines.filter((_, j) => j !== i))}><Trash2 className="w-4 h-4" /></button>
              </div>
            ))}
          </div>
          <button className={`${XS} mt-2`} onClick={() => setLines([...lines, { category: '', account_code: '', amount: '' }])}><Plus className="w-3 h-3" /> Add budget line</button>
        </div>

        <div>
          <label className="label">Tasks / phases <span className="text-gray-400 font-normal">(optional)</span></label>
          <div className="space-y-2">
            {tasks.map((t, i) => (
              <div key={i} className="flex gap-2 items-center">
                <input className="input flex-1" placeholder="Task / phase name" value={t.name} onChange={(e) => setTasks(tasks.map((x, j) => j === i ? { ...x, name: e.target.value } : x))} />
                <input className="input w-32" type="number" placeholder="Budget" value={t.budget_amount} onChange={(e) => setTasks(tasks.map((x, j) => j === i ? { ...x, budget_amount: e.target.value } : x))} />
                <button className="text-gray-400 hover:text-red-500" onClick={() => setTasks(tasks.filter((_, j) => j !== i))}><Trash2 className="w-4 h-4" /></button>
              </div>
            ))}
          </div>
          <button className={`${XS} mt-2`} onClick={() => setTasks([...tasks, { name: '', budget_amount: '' }])}><Plus className="w-3 h-3" /> Add task</button>
        </div>

        <div className="flex justify-end gap-2 pt-2 border-t">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!valid || mut.isPending} onClick={() => mut.mutate()}>{editing ? 'Save' : 'Create project'}</button>
        </div>
      </div>
    </Modal>
  );
}
