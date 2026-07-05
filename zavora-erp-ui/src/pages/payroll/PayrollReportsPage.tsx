import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { generateReport, exportReport } from '../../api/client';
import { formatCurrency } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import { Download } from 'lucide-react';

type Report = { key: string; type: string; label: string };
const REPORTS: Report[] = [
  { key: 'register', type: 'PayrollRegister', label: 'Payroll Register' },
  { key: 'schedule', type: 'StatutorySchedule', label: 'Statutory Schedule' },
  { key: 'p9', type: 'PayeP9', label: 'P9 Cards' },
  { key: 'bank', type: 'PayrollBankFile', label: 'Bank / EFT File' },
];

export default function PayrollReportsPage() {
  const year = new Date().getFullYear();
  const [from, setFrom] = useState(`${year}-01-01`);
  const [to, setTo] = useState(`${year}-12-31`);
  const [active, setActive] = useState<Report>(REPORTS[0]);

  const { data, isLoading, isError } = useQuery<any>({
    queryKey: ['payroll-report', active.type, from, to],
    queryFn: () => generateReport({ entity_id: '00000000-0000-0000-0000-000000000000', report_type: active.type, parameters: { period_from: from, period_to: to } }).then(r => r.data?.content?.Generic ?? {}),
  });

  const doExport = async () => {
    const r = await exportReport({ entity_id: '00000000-0000-0000-0000-000000000000', report_type: active.type, parameters: { period_from: from, period_to: to } });
    const url = URL.createObjectURL(r.data);
    const a = document.createElement('a');
    a.href = url; a.download = `${active.key}-${from}_${to}.csv`; a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div>
      <PageHeader title="Payroll Reports" subtitle="Register, statutory remittance schedule, P9 cards and the net-pay bank file."
        actions={
          <div className="flex items-end gap-2">
            <div><label className="block text-[11px] text-gray-500 mb-0.5">From</label><input type="date" className="input py-1.5 text-sm" value={from} onChange={e => setFrom(e.target.value)} /></div>
            <div><label className="block text-[11px] text-gray-500 mb-0.5">To</label><input type="date" className="input py-1.5 text-sm" value={to} onChange={e => setTo(e.target.value)} /></div>
            <button className="btn-secondary" onClick={doExport}><Download className="w-4 h-4" /> CSV</button>
          </div>
        } />

      <div className="flex gap-1 mb-5 border-b flex-wrap">
        {REPORTS.map(r => (
          <button key={r.key} onClick={() => setActive(r)} className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px ${active.key === r.key ? 'border-indigo-600 text-indigo-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}>{r.label}</button>
        ))}
      </div>

      {isLoading && <div className="card p-8 text-center text-gray-500">Loading…</div>}
      {isError && <div className="card p-8 text-center text-red-600">Failed to load report.</div>}
      {data && !isLoading && (
        active.key === 'register' ? <Register d={data} /> :
        active.key === 'schedule' ? <Schedule d={data} /> :
        active.key === 'p9' ? <P9 d={data} /> :
        <BankFile d={data} />
      )}
    </div>
  );
}

const th = 'px-3 py-2 text-xs text-gray-500 uppercase';
const money = (v: any) => formatCurrency(v ?? 0);

function Register({ d }: { d: any }) {
  const t = d.totals ?? {};
  return (
    <div className="card overflow-x-auto">
      <table className="w-full text-sm">
        <thead><tr className="border-b text-left">
          <th className={th}>Staff #</th><th className={th}>Employee</th><th className={th}>Dept</th>
          <th className={`${th} text-right`}>Gross</th><th className={`${th} text-right`}>PAYE</th><th className={`${th} text-right`}>NSSF</th>
          <th className={`${th} text-right`}>SHA</th><th className={`${th} text-right`}>Housing</th><th className={`${th} text-right`}>HELB</th><th className={`${th} text-right`}>Net</th>
        </tr></thead>
        <tbody>
          {(d.lines ?? []).map((l: any, i: number) => (
            <tr key={i} className="border-b">
              <td className="px-3 py-2 font-mono">{l.staff_number}</td><td className="px-3 py-2">{l.employee_name}</td><td className="px-3 py-2 text-gray-500">{l.department || '—'}</td>
              <td className="px-3 py-2 text-right">{money(l.gross)}</td><td className="px-3 py-2 text-right">{money(l.paye)}</td><td className="px-3 py-2 text-right">{money(l.nssf)}</td>
              <td className="px-3 py-2 text-right">{money(l.sha)}</td><td className="px-3 py-2 text-right">{money(l.housing)}</td><td className="px-3 py-2 text-right">{money(l.helb)}</td><td className="px-3 py-2 text-right font-medium">{money(l.net)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot><tr className="border-t-2 font-semibold bg-gray-50">
          <td className="px-3 py-2" colSpan={3}>Totals ({d.employee_count ?? (d.lines?.length ?? 0)})</td>
          <td className="px-3 py-2 text-right">{money(t.gross)}</td><td className="px-3 py-2 text-right">{money(t.paye)}</td><td className="px-3 py-2 text-right">{money(t.nssf)}</td>
          <td className="px-3 py-2 text-right">{money(t.sha)}</td><td className="px-3 py-2 text-right">{money(t.housing)}</td><td className="px-3 py-2 text-right">{money(t.helb)}</td><td className="px-3 py-2 text-right">{money(t.net)}</td>
        </tr></tfoot>
      </table>
    </div>
  );
}

function Schedule({ d }: { d: any }) {
  const t = d.totals ?? {};
  return (
    <div className="card overflow-x-auto">
      <table className="w-full text-sm">
        <thead><tr className="border-b text-left">
          <th className={th}>Employee</th><th className={th}>KRA PIN</th><th className={th}>NSSF No</th>
          <th className={`${th} text-right`}>PAYE</th><th className={`${th} text-right`}>NSSF (ee+er)</th><th className={`${th} text-right`}>SHA</th><th className={`${th} text-right`}>Housing (ee+er)</th><th className={`${th} text-right`}>HELB</th>
        </tr></thead>
        <tbody>
          {(d.lines ?? []).map((l: any, i: number) => (
            <tr key={i} className="border-b">
              <td className="px-3 py-2">{l.employee_name}</td><td className="px-3 py-2 font-mono text-xs">{l.kra_pin || '—'}</td><td className="px-3 py-2 font-mono text-xs">{l.nssf_number || '—'}</td>
              <td className="px-3 py-2 text-right">{money(l.paye)}</td><td className="px-3 py-2 text-right">{money(l.nssf_total)}</td><td className="px-3 py-2 text-right">{money(l.sha)}</td><td className="px-3 py-2 text-right">{money(l.housing_total)}</td><td className="px-3 py-2 text-right">{money(l.helb)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot><tr className="border-t-2 font-semibold bg-gray-50">
          <td className="px-3 py-2" colSpan={3}>Totals</td>
          <td className="px-3 py-2 text-right">{money(t.paye)}</td><td className="px-3 py-2 text-right">{money(t.nssf_total)}</td><td className="px-3 py-2 text-right">{money(t.sha)}</td><td className="px-3 py-2 text-right">{money(t.housing_total)}</td><td className="px-3 py-2 text-right">{money(t.helb)}</td>
        </tr></tfoot>
      </table>
    </div>
  );
}

function P9({ d }: { d: any }) {
  const emps = d.employees ?? [];
  if (emps.length === 0) return <div className="card p-8 text-center text-gray-500">No data for this period.</div>;
  return (
    <div className="space-y-4">
      {emps.map((e: any, i: number) => (
        <div key={i} className="card p-4">
          <div className="flex items-center justify-between mb-2">
            <h3 className="font-medium">{e.employee_name}</h3>
            <span className="text-xs text-gray-500 font-mono">{e.kra_pin || ''}</span>
          </div>
          <table className="w-full text-sm">
            <thead><tr className="border-b text-left"><th className={th}>Month</th><th className={`${th} text-right`}>Gross</th><th className={`${th} text-right`}>Taxable</th><th className={`${th} text-right`}>Tax charged</th><th className={`${th} text-right`}>Relief</th><th className={`${th} text-right`}>PAYE</th></tr></thead>
            <tbody>
              {(e.months ?? []).map((m: any, j: number) => (
                <tr key={j} className="border-b">
                  <td className="px-3 py-2">{m.month}</td><td className="px-3 py-2 text-right">{money(m.gross)}</td><td className="px-3 py-2 text-right">{money(m.taxable)}</td>
                  <td className="px-3 py-2 text-right">{money(m.tax_charged)}</td><td className="px-3 py-2 text-right">{money(m.personal_relief)}</td><td className="px-3 py-2 text-right font-medium">{money(m.paye)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
    </div>
  );
}

function BankFile({ d }: { d: any }) {
  return (
    <div className="card overflow-x-auto">
      <div className="px-3 py-2 text-sm text-gray-600 border-b">Total net: <span className="font-semibold">{money(d.total_net)}</span> · {d.count ?? (d.lines?.length ?? 0)} payment(s)</div>
      <table className="w-full text-sm">
        <thead><tr className="border-b text-left"><th className={th}>Employee</th><th className={th}>Bank</th><th className={th}>Branch</th><th className={th}>Account</th><th className={`${th} text-right`}>Amount</th></tr></thead>
        <tbody>
          {(d.lines ?? []).map((l: any, i: number) => (
            <tr key={i} className="border-b">
              <td className="px-3 py-2">{l.employee_name}</td><td className="px-3 py-2">{l.bank_name || '—'}</td><td className="px-3 py-2 text-gray-500">{l.branch || '—'}</td>
              <td className="px-3 py-2 font-mono">{l.account_number || '—'}</td><td className="px-3 py-2 text-right font-medium">{money(l.amount)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
