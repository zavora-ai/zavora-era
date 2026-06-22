import { num } from './primitives';

export default function DimensionalAnalysisView({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">{c.dimension_type}</th>
          <th className="text-right">Debit</th>
          <th className="text-right">Credit</th>
          <th className="text-right">Net</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any) => (
          <tr key={l.value_code} className="border-b border-gray-50">
            <td className="py-1.5">{l.value_name}{l.value_name !== l.value_code && <span className="ml-1 font-mono text-xs text-gray-400">{l.value_code}</span>}</td>
            <td className="text-right">{num(l.debit)}</td>
            <td className="text-right">{num(l.credit)}</td>
            <td className="text-right font-medium">{num(l.net)}</td>
          </tr>
        ))}
        {c.lines.length === 0 && (
          <tr><td colSpan={4} className="py-4 text-center text-gray-400">No activity tagged with this dimension in the period</td></tr>
        )}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2">Total</td>
          <td className="text-right">{num(c.total_debit)}</td>
          <td className="text-right">{num(c.total_credit)}</td>
          <td className="text-right">{num(c.total_net)}</td>
        </tr>
      </tfoot>
    </table>
  );
}
