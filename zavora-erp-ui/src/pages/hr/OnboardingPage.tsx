import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getOnboardingCases, createOnboarding, getOnboardingCase, setOnboardingTask, completeOnboarding, getEmployees } from '../../api/client';
import { hasRole, ROLES_MANAGE } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import { workToday } from '../../utils/workDate';
import { Plus, CheckCircle2, Circle, UserPlus } from 'lucide-react';

export default function OnboardingPage() {
  const qc = useQueryClient();
  const canManage = hasRole(ROLES_MANAGE);
  const [showNew, setShowNew] = useState(false);
  const [openCase, setOpenCase] = useState<string | null>(null);
  const { data: cases = [] } = useQuery<any[]>({ queryKey: ['onboarding'], queryFn: () => getOnboardingCases().then(r => r.data) });

  return (
    <div>
      <PageHeader title="Onboarding" subtitle="New-hire checklists & probation"
        actions={canManage && <button className="btn-primary" onClick={() => setShowNew(true)}><Plus className="w-4 h-4" /> Start Onboarding</button>} />
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {cases.map(c => {
          const pct = c.total ? Math.round((c.done / c.total) * 100) : 0;
          return (
            <button key={c.id} onClick={() => setOpenCase(c.id)} className="card p-4 text-left hover:shadow-md transition-shadow">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div className="w-9 h-9 rounded-full bg-indigo-100 text-indigo-700 flex items-center justify-center"><UserPlus className="w-4 h-4" /></div>
                  <div><p className="font-medium text-gray-900">{c.employee_name}</p><p className="text-xs text-gray-400">{c.job_title || '—'}</p></div>
                </div>
                <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${c.status === 'Complete' ? 'bg-green-100 text-green-700' : 'bg-amber-100 text-amber-700'}`}>{c.status}</span>
              </div>
              <div className="mt-3">
                <div className="flex justify-between text-xs text-gray-500 mb-1"><span>Checklist</span><span>{c.done}/{c.total}</span></div>
                <div className="h-2 bg-gray-100 rounded-full overflow-hidden"><div className="h-full bg-indigo-500" style={{ width: `${pct}%` }} /></div>
              </div>
              <p className="text-xs text-gray-400 mt-3">Start {c.start_date}{c.probation_end ? ` · Probation ends ${c.probation_end}` : ''}</p>
            </button>
          );
        })}
        {cases.length === 0 && <div className="col-span-full card p-12 text-center text-gray-400">No onboarding cases yet.</div>}
      </div>
      {showNew && <NewOnboardingModal onClose={() => setShowNew(false)} onSaved={() => { qc.invalidateQueries({ queryKey: ['onboarding'] }); setShowNew(false); }} />}
      {openCase && <CaseModal caseId={openCase} canManage={canManage} onClose={() => setOpenCase(null)} onChanged={() => qc.invalidateQueries({ queryKey: ['onboarding'] })} />}
    </div>
  );
}

function NewOnboardingModal({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const { data: employees = [] } = useQuery<any[]>({ queryKey: ['employees'], queryFn: () => getEmployees().then(r => Array.isArray(r.data) ? r.data : []) });
  const [form, setForm] = useState({ employee_id: '', start_date: workToday(), probation_end: '' });
  const [err, setErr] = useState('');
  const mut = useMutation({
    mutationFn: () => createOnboarding({ employee_id: form.employee_id, start_date: form.start_date, probation_end: form.probation_end || undefined }),
    onSuccess: onSaved,
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Failed'),
  });
  return (
    <Modal open onClose={onClose} title="Start Onboarding" size="md">
      <div className="space-y-3">
        {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
        <div><label className="label">Employee</label>
          <select className="input" value={form.employee_id} onChange={e => setForm({ ...form, employee_id: e.target.value })}>
            <option value="">Choose…</option>
            {employees.map(e => <option key={e.id} value={e.id}>{e.full_name}</option>)}
          </select>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Start date</label><input type="date" className="input" value={form.start_date} onChange={e => setForm({ ...form, start_date: e.target.value })} /></div>
          <div><label className="label">Probation ends</label><input type="date" className="input" value={form.probation_end} onChange={e => setForm({ ...form, probation_end: e.target.value })} /></div>
        </div>
        <p className="text-xs text-gray-400">A standard onboarding checklist will be created automatically.</p>
        <div className="flex justify-end gap-2 pt-2">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!form.employee_id || mut.isPending} onClick={() => { setErr(''); mut.mutate(); }}>{mut.isPending ? 'Creating…' : 'Create'}</button>
        </div>
      </div>
    </Modal>
  );
}

function CaseModal({ caseId, canManage, onClose, onChanged }: { caseId: string; canManage: boolean; onClose: () => void; onChanged: () => void }) {
  const qc = useQueryClient();
  const { data } = useQuery<any>({ queryKey: ['onboarding', caseId], queryFn: () => getOnboardingCase(caseId).then(r => r.data) });
  const toggle = useMutation({ mutationFn: ({ taskId, done }: { taskId: string; done: boolean }) => setOnboardingTask(caseId, taskId, done), onSuccess: () => { qc.invalidateQueries({ queryKey: ['onboarding', caseId] }); onChanged(); } });
  const complete = useMutation({ mutationFn: () => completeOnboarding(caseId), onSuccess: () => { qc.invalidateQueries({ queryKey: ['onboarding', caseId] }); onChanged(); onClose(); } });
  const tasks: any[] = data?.tasks ?? [];
  const c = data?.case;
  const allDone = tasks.length > 0 && tasks.every(t => t.is_done);
  return (
    <Modal open onClose={onClose} title="Onboarding checklist" size="md">
      {c && (
        <div className="space-y-3">
          <p className="text-sm text-gray-500">Status: <span className="font-medium capitalize">{c.status}</span>{c.probation_end ? ` · Probation ends ${c.probation_end}` : ''}</p>
          <div className="divide-y divide-gray-100 border rounded-lg">
            {tasks.map(t => (
              <button key={t.id} disabled={!canManage || c.status === 'Complete'} onClick={() => toggle.mutate({ taskId: t.id, done: !t.is_done })}
                className="flex items-center gap-3 w-full text-left px-3 py-2.5 hover:bg-gray-50 disabled:cursor-default">
                {t.is_done ? <CheckCircle2 className="w-5 h-5 text-green-500 shrink-0" /> : <Circle className="w-5 h-5 text-gray-300 shrink-0" />}
                <span className={`text-sm ${t.is_done ? 'text-gray-400 line-through' : 'text-gray-700'}`}>{t.title}</span>
              </button>
            ))}
          </div>
          {canManage && c.status !== 'Complete' && (
            <div className="flex justify-end">
              <button className="btn-primary" disabled={!allDone || complete.isPending} onClick={() => complete.mutate()} title={allDone ? '' : 'Complete all tasks first'}>
                {complete.isPending ? 'Completing…' : 'Mark onboarding complete'}
              </button>
            </div>
          )}
        </div>
      )}
    </Modal>
  );
}
