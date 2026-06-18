import { num } from './primitives';

export default function WhtReportView({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Date</th>
          <th className="text-left">Bill</th>
          <th className="text-left">Vendor</th>
          <th className="text-left">KRA PIN</th>
          <th className="text-left">Category</th>
          <th className="text-right">Base</th>
          <th className="text-right">WHT</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5">{l.date}</td>
            <td className="font-mono text-xs">{l.document_number}</td>
            <td>{l.vendor_name}</td>
            <td className="font-mono text-xs">{l.kra_pin || '—'}</td>
            <td className="text-gray-500">{l.wht_category || '—'}</td>
            <td className="text-right">{num(l.base_amount)}</td>
            <td className="text-right font-medium">{num(l.wht_amount)}</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={7} className="py-4 text-center text-gray-400">No withholding tax in this period</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={5}>Total</td>
          <td className="text-right">{num(c.total_base)}</td>
          <td className="text-right">{num(c.total_wht)}</td>
        </tr>
      </tfoot>
    </table>
  );
}
