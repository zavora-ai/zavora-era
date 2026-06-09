import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { runPayroll, approvePayRun, postPayRun } from '../../api/client';
import { formatCurrency } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import StatCard from '../../components/shared/StatCard';
import { Play, CheckCircle, BookOpen, Users } from 'lucide-react';

export default function PayrollPage() {
  const [payRun, setPayRun] = useState<any>(null);

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
    const today = new Date().toISOString().split('T')[0];
    runMut.mutate({
      period_id: '00000000-0000-0000-0000-000000000001', // would come from period selector
      pay_date: today,
      run_by: { type: 'Agent', id: 'ui' },
    });
  };

  return (
    <div>
      <PageHeader
        title="Payroll"
        subtitle="Kenya statutory payroll — PAYE, NSSF, SHA, Housing Levy, HELB"
        actions={
          <button onClick={handleRun} className="btn-primary" disabled={runMut.isPending}>
            <Play className="w-4 h-4" /> {runMut.isPending ? 'Running...' : 'Run Payroll'}
          </button>
        }
      />

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
              {payRun.status === 'draft' && (
                <button onClick={() => approveMut.mutate(payRun.id)} className="btn-success"><CheckCircle className="w-4 h-4" /> Approve</button>
              )}
              {payRun.status === 'approved' && (
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
