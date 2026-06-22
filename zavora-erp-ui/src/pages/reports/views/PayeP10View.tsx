import { num } from './primitives';

export default function PayeP10View({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Employee</th>
          <th className="text-left">KRA PIN</th>
          <th className="text-right">Gross</th>
          <th className="text-right">Taxable</th>
          <th className="text-right">Tax</th>
          <th className="text-right">Relief</th>
          <th className="text-right">PAYE Due</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5">{l.employee_name} <span className="text-xs text-gray-400">{l.staff_number}</span></td>
            <td className="font-mono text-xs">{l.kra_pin}</td>
            <td className="text-right">{num(l.gross_pay)}</td>
            <td className="text-right">{num(l.taxable_pay)}</td>
            <td className="text-right">{num(l.tax)}</td>
            <td className="text-right text-gray-500">{num(Number(l.personal_relief) + Number(l.insurance_relief))}</td>
            <td className="text-right font-medium">{num(l.paye_payable)}</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={7} className="py-4 text-center text-gray-400">No payroll in this period</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={2}>Total</td>
          <td className="text-right">{num(c.total_gross)}</td>
          <td className="text-right">{num(c.total_taxable)}</td>
          <td className="text-right">{num(c.total_paye)}</td>
          <td className="text-right">{num(c.total_relief)}</td>
          <td className="text-right">{num(c.total_payable)}</td>
        </tr>
      </tfoot>
    </table>
  );
}
