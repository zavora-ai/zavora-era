import { num } from './primitives';

export default function PayrollSummaryView({ c }: { c: any }) {
  const t = c.totals;
  const cols = ['Gross', 'PAYE', 'NSSF', 'SHA', 'Housing Levy', 'HELB', 'Net'];
  return (
    <div>
      <p className="text-sm text-gray-500 mb-3">
        {c.run_count} pay run{c.run_count === 1 ? '' : 's'} · {c.employee_count} employee{c.employee_count === 1 ? '' : 's'}
      </p>
      <table className="w-full text-sm">
        <thead>
          <tr className="text-xs text-gray-500 uppercase border-b">
            <th className="text-left py-2">Employee</th>
            {cols.map((h) => <th key={h} className="text-right">{h}</th>)}
          </tr>
        </thead>
        <tbody>
          {c.employees.map((e: any) => (
            <tr key={e.employee_id} className="border-b border-gray-50">
              <td className="py-1.5">{e.employee_name}</td>
              <td className="text-right">{num(e.gross)}</td>
              <td className="text-right">{num(e.paye)}</td>
              <td className="text-right">{num(e.nssf)}</td>
              <td className="text-right">{num(e.sha)}</td>
              <td className="text-right">{num(e.housing_levy)}</td>
              <td className="text-right">{num(e.helb)}</td>
              <td className="text-right font-medium">{num(e.net)}</td>
            </tr>
          ))}
          {c.employees.length === 0 && (
            <tr><td colSpan={8} className="py-4 text-center text-gray-400">No payroll in this period</td></tr>
          )}
        </tbody>
        <tfoot>
          <tr className="font-bold border-t-2">
            <td className="py-2">Total</td>
            <td className="text-right">{num(t.gross)}</td>
            <td className="text-right">{num(t.paye)}</td>
            <td className="text-right">{num(t.nssf)}</td>
            <td className="text-right">{num(t.sha)}</td>
            <td className="text-right">{num(t.housing_levy)}</td>
            <td className="text-right">{num(t.helb)}</td>
            <td className="text-right">{num(t.net)}</td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}
