import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  runPayroll, approvePayRun, postPayRun, markPayRunPaid, listPayRuns, getPayRun,
  recomputePayRun, deletePayRun, listRunInputs, addRunInput, deleteRunInput,
  getPeriods, getPayslipPdf, getEmployees, listEarningTypes, listDeductionTypes,
} from '../../api/client';
import { formatCurrency } from '../../utils/format';
import { hasRole, ROLES_APPROVE, ROLES_POST } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import StatCard from '../../components/shared/StatCard';
import { Play, CheckCircle, BookOpen, Users, Trash2, RefreshCw, Plus, ArrowLeft, Wallet } from 'lucide-react';

const STATUS_BADGE: Record<string, string> = {
  draft: 'bg-gray-100 text-gray-700',
  approved: 'bg-amber-100 text-amber-700',
  posted: 'bg-blue-100 text-blue-700',
  paid: 'bg-green-100 text-green-700',
};

export default function PayrollPage() {
  const qc = useQueryClient();
  const [selected, setSelected] = useState<string | null>(null);
  const [showNew, setShowNew] = useState(false);
  const [periodId, setPeriodId] = useState('');
  const [payDate, setPayDate] = useState(new Date().toISOString().split('T')[0]);
  const [err, setErr] = useState('');

  const { data: periods = [] } = useQuery<any[]>({
    queryKey: ['periods'],
    queryFn: () => getPeriods().then(r => (Array.isArray(r.data) ? r.data : r.data?.data ?? [])),
  });
  const openPeriods = periods.filter(p => p.status !== 'hard_closed');

  const { data: runs = [], refetch: refetchRuns } = useQuery<any[]>({
    queryKey: ['pay-runs'],
    queryFn: () => listPayRuns().then(r => r.data),
  });

  const { data: employees = [] } = useQuery<any[]>({
    queryKey: ['employees'],
    queryFn: () => getEmployees().then((r: any) => (Array.isArray(r.data) ? r.data : r.data?.data ?? [])),
  });
  const issues = (employees as any[]).filter(e => e.is_active && (!e.kra_pin || !e.bank_account?.account_number))
    .map(e => ({ name: e.full_name, missing: [!e.kra_pin ? 'KRA PIN' : null, !e.bank_account?.account_number ? 'bank account' : null].filter(Boolean).join(', ') }));

  const fail = (e: any) => setErr(e?.response?.data?.error ?? 'Operation failed');

  const runMut = useMutation({
    mutationFn: (data: any) => runPayroll(data),
    onSuccess: (res) => { setErr(''); setShowNew(false); refetchRuns(); setSelected(res.data.id); },
    onError: fail,
  });

  const handleRun = () => {
    if (!periodId) return;
    runMut.mutate({ period_id: periodId, pay_date: payDate, run_by: { type: 'Agent', id: 'ui' } });
  };
  const onPeriodChange = (id: string) => {
    setPeriodId(id);
    const p = periods.find(x => x.id === id);
    if (p?.end_date) setPayDate(p.end_date);
  };

  return (
    <div>
      <PageHeader
        title="Payroll"
        subtitle="Prepare → review → commit. Kenya statutory: PAYE, NSSF, SHA, Housing Levy, HELB."
        actions={!selected && (
          <button onClick={() => { setShowNew(v => !v); setErr(''); }} className="btn-primary">
            <Plus className="w-4 h-4" /> New Pay Run
          </button>
        )}
      />

      {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded mb-4">{err}</div>}

      {showNew && !selected && (
        <div className="mb-4 space-y-3">
          {issues.length > 0 && (
            <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800">
              <p className="font-medium">{issues.length} employee(s) have incomplete payroll details:</p>
              <ul className="mt-1 list-disc list-inside text-xs">
                {issues.slice(0, 8).map((i, k) => <li key={k}>{i.name} — missing {i.missing}</li>)}
                {issues.length > 8 && <li>…and {issues.length - 8} more</li>}
              </ul>
              <p className="mt-1 text-xs">You can still run payroll; fix these before generating the bank/statutory files.</p>
            </div>
          )}
          <div className="card p-4 flex items-end gap-3">
          <div>
            <label className="block text-[11px] text-gray-500 mb-0.5">Period</label>
            <select className="input py-1.5 text-sm" value={periodId} onChange={e => onPeriodChange(e.target.value)}>
              <option value="">Select period…</option>
              {openPeriods.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </div>
          <div>
            <label className="block text-[11px] text-gray-500 mb-0.5">Pay date</label>
            <input type="date" className="input py-1.5 text-sm" value={payDate} onChange={e => setPayDate(e.target.value)} />
          </div>
          <button onClick={handleRun} className="btn-primary" disabled={runMut.isPending || !periodId}>
            <Play className="w-4 h-4" /> {runMut.isPending ? 'Running…' : 'Run Payroll'}
          </button>
          </div>
        </div>
      )}

      {selected
        ? <RunDetail id={selected} onBack={() => { setSelected(null); refetchRuns(); }} onErr={fail} qc={qc} />
        : <RunHistory runs={runs} onOpen={setSelected} />}
    </div>
  );
}

function RunHistory({ runs, onOpen }: { runs: any[]; onOpen: (id: string) => void }) {
  if (runs.length === 0) {
    return (
      <div className="card p-12 text-center">
        <Users className="w-12 h-12 text-gray-300 mx-auto mb-4" />
        <h3 className="text-lg font-medium text-gray-900 mb-2">No pay runs yet</h3>
        <p className="text-sm text-gray-500">Click "New Pay Run" to compute salaries for active employees.</p>
      </div>
    );
  }
  return (
    <div className="card overflow-x-auto">
      <table className="w-full text-sm">
        <thead><tr className="border-b text-left text-xs text-gray-500 uppercase">
          <th className="px-3 py-2">Pay date</th><th className="px-3 py-2">Status</th>
          <th className="px-3 py-2 text-right">Employees</th><th className="px-3 py-2 text-right">Gross</th>
          <th className="px-3 py-2 text-right">Net</th><th className="px-3 py-2 text-right">Employer cost</th>
        </tr></thead>
        <tbody>
          {runs.map(r => (
            <tr key={r.id} className="border-b hover:bg-gray-50 cursor-pointer" onClick={() => onOpen(r.id)}>
              <td className="px-3 py-2 font-medium">{r.pay_date}</td>
              <td className="px-3 py-2"><span className={`px-2 py-0.5 rounded text-xs capitalize ${STATUS_BADGE[r.status] ?? ''}`}>{r.status}</span></td>
              <td className="px-3 py-2 text-right">{r.employee_count}</td>
              <td className="px-3 py-2 text-right">{formatCurrency(r.total_gross)}</td>
              <td className="px-3 py-2 text-right font-medium">{formatCurrency(r.total_net)}</td>
              <td className="px-3 py-2 text-right text-gray-500">{formatCurrency(r.total_employer_cost)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function RunDetail({ id, onBack, onErr, qc }: { id: string; onBack: () => void; onErr: (e: any) => void; qc: any }) {
  const { data: run, refetch } = useQuery<any>({ queryKey: ['pay-run', id], queryFn: () => getPayRun(id).then(r => r.data) });
  const { data: inputs = [], refetch: refetchInputs } = useQuery<any[]>({ queryKey: ['pay-run-inputs', id], queryFn: () => listRunInputs(id).then(r => r.data) });
  const invalidate = () => { refetch(); refetchInputs(); qc.invalidateQueries({ queryKey: ['pay-runs'] }); };

  const approveMut = useMutation({ mutationFn: () => approvePayRun(id), onSuccess: invalidate, onError: onErr });
  const postMut = useMutation({ mutationFn: () => postPayRun(id), onSuccess: invalidate, onError: onErr });
  const paidMut = useMutation({ mutationFn: () => markPayRunPaid(id), onSuccess: invalidate, onError: onErr });
  const recomputeMut = useMutation({ mutationFn: () => recomputePayRun(id), onSuccess: invalidate, onError: onErr });
  const deleteMut = useMutation({ mutationFn: () => deletePayRun(id), onSuccess: onBack, onError: onErr });

  if (!run) return <div className="card p-8 text-center text-gray-500">Loading…</div>;
  const isDraft = run.status === 'draft';

  return (
    <div>
      <button onClick={onBack} className="text-sm text-gray-500 hover:text-gray-800 mb-3 flex items-center gap-1"><ArrowLeft className="w-4 h-4" /> All pay runs</button>

      <div className="grid grid-cols-2 sm:grid-cols-3 gap-4 mb-6">
        <StatCard title="Gross" value={formatCurrency(run.total_gross)} icon={<Users className="w-5 h-5" />} />
        <StatCard title="PAYE" value={formatCurrency(run.total_paye)} />
        <StatCard title="NSSF" value={formatCurrency(run.total_nssf)} />
        <StatCard title="SHA" value={formatCurrency(run.total_sha)} />
        <StatCard title="Housing" value={formatCurrency(run.total_housing_levy)} />
        <StatCard title="Net Pay" value={formatCurrency(run.total_net)} icon={<Wallet className="w-5 h-5" />} />
      </div>

      <div className="card p-6">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h3 className="font-medium">Pay Run — {run.pay_date}</h3>
            <p className="text-sm text-gray-500">Status: <span className={`px-2 py-0.5 rounded text-xs capitalize ${STATUS_BADGE[run.status] ?? ''}`}>{run.status}</span></p>
          </div>
          <div className="flex gap-2">
            {isDraft && <button onClick={() => recomputeMut.mutate()} className="btn-secondary" disabled={recomputeMut.isPending}><RefreshCw className="w-4 h-4" /> Recompute</button>}
            {isDraft && hasRole(ROLES_APPROVE) && <button onClick={() => approveMut.mutate()} className="btn-success"><CheckCircle className="w-4 h-4" /> Approve</button>}
            {isDraft && <button onClick={() => deleteMut.mutate()} className="btn-danger"><Trash2 className="w-4 h-4" /> Delete</button>}
            {run.status === 'approved' && hasRole(ROLES_POST) && <button onClick={() => postMut.mutate()} className="btn-primary"><BookOpen className="w-4 h-4" /> Post to GL</button>}
            {run.status === 'posted' && hasRole(ROLES_POST) && <button onClick={() => paidMut.mutate()} className="btn-success"><Wallet className="w-4 h-4" /> Mark Paid</button>}
          </div>
        </div>

        {isDraft && <InputsPanel runId={id} inputs={inputs} onChange={refetchInputs} onApplied={() => recomputeMut.mutate()} recomputing={recomputeMut.isPending} onErr={onErr} />}

        <div className="overflow-x-auto mt-4">
          <table className="w-full text-sm">
            <thead><tr className="border-b text-left text-xs text-gray-500 uppercase">
              <th className="px-3 py-2">Employee</th><th className="px-3 py-2 text-right">Gross</th><th className="px-3 py-2 text-right">Taxable</th>
              <th className="px-3 py-2 text-right">PAYE</th><th className="px-3 py-2 text-right">NSSF</th><th className="px-3 py-2 text-right">SHA</th>
              <th className="px-3 py-2 text-right">Housing</th><th className="px-3 py-2 text-right">HELB</th><th className="px-3 py-2 text-right">Net</th><th className="px-3 py-2"></th>
            </tr></thead>
            <tbody>
              {(run.payslips ?? []).map((ps: any) => (
                <tr key={ps.id} className="border-b">
                  <td className="px-3 py-2 font-medium">{ps.employee_name}</td>
                  <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.gross_salary)}</td>
                  <td className="px-3 py-2 text-right text-gray-500">{formatCurrency(ps.deductions.taxable_income)}</td>
                  <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.net_paye)}</td>
                  <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.nssf_employee)}</td>
                  <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.sha)}</td>
                  <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.housing_levy_employee)}</td>
                  <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.helb)}</td>
                  <td className="px-3 py-2 text-right font-medium">{formatCurrency(ps.deductions.net_salary)}</td>
                  <td className="px-3 py-2 text-right">
                    <button onClick={async () => { const r = await getPayslipPdf(run.id, ps.employee_id); window.open(URL.createObjectURL(r.data), '_blank'); }} className="text-indigo-600 text-xs hover:underline">PDF</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function InputsPanel({ runId, inputs, onChange, onApplied, recomputing, onErr }: { runId: string; inputs: any[]; onChange: () => void; onApplied: () => void; recomputing: boolean; onErr: (e: any) => void }) {
  const { data: employees = [] } = useQuery<any[]>({ queryKey: ['employees'], queryFn: () => getEmployees().then((r: any) => (Array.isArray(r.data) ? r.data : r.data?.data ?? [])) });
  const { data: earningTypes = [] } = useQuery<any[]>({ queryKey: ['earning-types'], queryFn: () => listEarningTypes().then(r => r.data) });
  const { data: deductionTypes = [] } = useQuery<any[]>({ queryKey: ['deduction-types'], queryFn: () => listDeductionTypes().then(r => r.data) });
  const [f, setF] = useState<any>({ employee_id: '', kind: 'earning', type_code: '', name: '', amount: '', taxable: true });
  const empName = (id: string) => (employees as any[]).find(e => e.id === id)?.full_name ?? '';

  const addMut = useMutation({
    mutationFn: () => addRunInput(runId, { ...f, amount: Number(f.amount) }),
    onSuccess: () => { setF({ employee_id: '', kind: 'earning', type_code: '', name: '', amount: '', taxable: true }); onChange(); onApplied(); },
    onError: onErr,
  });
  const delMut = useMutation({ mutationFn: (inputId: string) => deleteRunInput(runId, inputId), onSuccess: () => { onChange(); onApplied(); }, onError: onErr });

  const types = f.kind === 'earning' ? earningTypes : deductionTypes;
  const canAdd = f.employee_id && f.name && Number(f.amount) > 0;

  return (
    <div className="rounded-lg border border-indigo-100 bg-indigo-50/40 p-3">
      <p className="text-sm font-medium text-gray-700 mb-2 flex items-center gap-2">
        Adjustments (bonuses, overtime, deductions)
        {recomputing && <span className="text-xs font-normal text-indigo-600 inline-flex items-center gap-1"><RefreshCw className="w-3 h-3 animate-spin" /> updating run…</span>}
      </p>
      <div className="flex flex-wrap items-end gap-2 mb-2">
        <select className="input py-1 text-sm" value={f.employee_id} onChange={e => setF({ ...f, employee_id: e.target.value })}>
          <option value="">Employee…</option>
          {employees.map(e => <option key={e.id} value={e.id}>{e.full_name}</option>)}
        </select>
        <select className="input py-1 text-sm" value={f.kind} onChange={e => setF({ ...f, kind: e.target.value, type_code: '', name: '' })}>
          <option value="earning">Earning</option>
          <option value="deduction">Deduction</option>
        </select>
        <select className="input py-1 text-sm" value={f.type_code} onChange={e => { const t = types.find((x: any) => x.code === e.target.value); setF({ ...f, type_code: e.target.value, name: t?.name ?? f.name, taxable: t?.taxable ?? f.taxable }); }}>
          <option value="">Type…</option>
          {types.map((t: any) => <option key={t.id} value={t.code}>{t.name}</option>)}
        </select>
        <input className="input py-1 text-sm w-36" placeholder="Description" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} />
        <input className="input py-1 text-sm w-28" type="number" placeholder="Amount" value={f.amount} onChange={e => setF({ ...f, amount: e.target.value })} />
        {f.kind === 'earning' && (
          <label className="flex items-center gap-1 text-xs text-gray-600"><input type="checkbox" checked={f.taxable} onChange={e => setF({ ...f, taxable: e.target.checked })} /> Taxable</label>
        )}
        <button className="btn-secondary py-1" disabled={!canAdd || addMut.isPending} onClick={() => addMut.mutate()}><Plus className="w-4 h-4" /> Add</button>
      </div>
      {inputs.length > 0 && (
        <div className="mt-1">
          <p className="text-[11px] font-medium text-gray-500 mb-1">Applied to this run ({inputs.length})</p>
          <div className="flex flex-wrap gap-2 max-h-40 overflow-y-auto pr-1">
            {inputs.map(i => (
              <span key={i.id} className="inline-flex items-center gap-1.5 bg-white border rounded px-2 py-1 text-xs max-w-full">
                <span className="text-gray-500 truncate max-w-[8rem]">{empName(i.employee_id)}</span>
                <span className={i.kind === 'deduction' ? 'text-red-600 font-medium' : 'text-green-700 font-medium'}>{i.kind === 'deduction' ? '−' : '+'}{formatCurrency(i.amount)}</span>
                <span className="text-gray-700 truncate max-w-[8rem]">{i.name}</span>
                <button onClick={() => delMut.mutate(i.id)} className="text-gray-400 hover:text-red-600 shrink-0"><Trash2 className="w-3 h-3" /></button>
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
