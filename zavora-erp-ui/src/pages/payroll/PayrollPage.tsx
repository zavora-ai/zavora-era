import { useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { runPayroll, approvePayRun, postPayRun, getPeriods } from '../../api/client';
import { formatCurrency } from '../../utils/format';
import { hasRole, ROLES_APPROVE, ROLES_POST } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import StatCard from '../../components/shared/StatCard';
import { Play, CheckCircle, BookOpen, Users } from 'lucide-react';

export default function PayrollPage() {
  const [payRun, setPayRun] = useState<any>(null);
  const [periodId, setPeriodId] = useState('');
  const [payDate, setPayDate] = useState(new Date().toISOString().split('T')[0]);

  // Open periods to run payroll against (can't post into a closed period).
  const { data: periods = [] } = useQuery<any[]>({
    queryKey: ['periods'],
    queryFn: () => getPeriods().then(r => (Array.isArray(r.data) ? r.data : r.data?.data ?? [])),
  });
  const openPeriods = periods.filter(p => p.status !== 'hard_closed');

  const runMut = useMutation({
    mutationFn: (data: any) => runPayroll(data),
    onSuccess: (res) => setPayRun(res.data),
  });

  const approveMut = useMutation({
    mutationFn: (id: string) => approvePayRun(id),
    onSuccess: () => setPayRun({ ...payRun, status: 'approved' }),
  });

  const postMut = useMutation({
    mutationFn: (id: string) => postPayRun(id),
    onSuccess: () => setPayRun({ ...payRun, status: 'posted' }),
  });

  const handleRun = () => {
    if (!periodId) return;
    runMut.mutate({
      period_id: periodId,
      pay_date: payDate,
      run_by: { type: 'Agent', id: 'ui' },
    });
  };

  // When a period is picked, default the pay date to its month-end-ish (last day).
  const onPeriodChange = (id: string) => {
    setPeriodId(id);
    const p = periods.find(x => x.id === id);
    if (p?.end_date) setPayDate(p.end_date);
  };

  return (
    <div>
      <PageHeader
        title="Payroll"
        subtitle="Kenya statutory payroll — PAYE, NSSF, SHA, Housing Levy, HELB"
        actions={
          <div className="flex items-end gap-2">
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
              <Play className="w-4 h-4" /> {runMut.isPending ? 'Running...' : 'Run Payroll'}
            </button>
          </div>
        }
      />

      {runMut.isError && (
        <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded mb-4">
          {(runMut.error as any)?.response?.data?.error ?? 'Payroll run failed'}
        </div>
      )}

      {/* Statutory deductions summary */}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-4 mb-6">
        <StatCard title="Gross" value={payRun ? formatCurrency(payRun.total_gross) : '—'} icon={<Users className="w-5 h-5" />} />
        <StatCard title="PAYE" value={payRun ? formatCurrency(payRun.total_paye) : '—'} />
        <StatCard title="NSSF" value={payRun ? formatCurrency(payRun.total_nssf) : '—'} />
        <StatCard title="SHA" value={payRun ? formatCurrency(payRun.total_sha) : '—'} />
        <StatCard title="Housing Levy" value={payRun ? formatCurrency(payRun.total_housing_levy) : '—'} />
        <StatCard title="Net Pay" value={payRun ? formatCurrency(payRun.total_net) : '—'} icon={<BookOpen className="w-5 h-5" />} />
      </div>

      {payRun && (
        <div className="card p-6">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="font-medium">Pay Run — {payRun.pay_date}</h3>
              <p className="text-sm text-gray-500">Status: <span className="font-medium capitalize">{payRun.status}</span></p>
            </div>
            <div className="flex gap-2">
              {payRun.status === 'draft' && hasRole(ROLES_APPROVE) && (
                <button onClick={() => approveMut.mutate(payRun.id)} className="btn-success"><CheckCircle className="w-4 h-4" /> Approve</button>
              )}
              {payRun.status === 'approved' && hasRole(ROLES_POST) && (
                <button onClick={() => postMut.mutate(payRun.id)} className="btn-primary"><BookOpen className="w-4 h-4" /> Post to GL</button>
              )}
            </div>
          </div>

          {payRun.payslips && payRun.payslips.length > 0 && (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead><tr className="border-b text-left text-xs text-gray-500 uppercase">
                  <th className="px-3 py-2">Employee</th><th className="px-3 py-2 text-right">Gross</th><th className="px-3 py-2 text-right">PAYE</th><th className="px-3 py-2 text-right">NSSF</th><th className="px-3 py-2 text-right">SHA</th><th className="px-3 py-2 text-right">Net</th>
                </tr></thead>
                <tbody>
                  {payRun.payslips.map((ps: any) => (
                    <tr key={ps.id} className="border-b">
                      <td className="px-3 py-2 font-medium">{ps.employee_name}</td>
                      <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.gross_salary)}</td>
                      <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.net_paye)}</td>
                      <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.nssf_employee)}</td>
                      <td className="px-3 py-2 text-right">{formatCurrency(ps.deductions.sha)}</td>
                      <td className="px-3 py-2 text-right font-medium">{formatCurrency(ps.deductions.net_salary)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {!payRun && (
        <div className="card p-12 text-center">
          <Users className="w-12 h-12 text-gray-300 mx-auto mb-4" />
          <h3 className="text-lg font-medium text-gray-900 mb-2">No pay run yet</h3>
          <p className="text-sm text-gray-500 mb-4">Click "Run Payroll" to compute salaries for all active employees.<br />Kenya statutory deductions (PAYE, NSSF, SHA, Housing Levy, HELB) are calculated automatically.</p>
        </div>
      )}
    </div>
  );
}
