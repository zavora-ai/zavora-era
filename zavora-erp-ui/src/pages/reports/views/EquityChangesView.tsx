import { num } from './primitives';

export default function EquityChangesView({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Account</th>
          <th className="text-right">Opening</th>
          <th className="text-right">Movement</th>
          <th className="text-right">Closing</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any) => (
          <tr key={l.account_code} className="border-b border-gray-50">
            <td className="py-1.5"><span className="font-mono text-xs text-gray-400">{l.account_code}</span> {l.account_name}</td>
            <td className="text-right">{num(l.opening)}</td>
            <td className="text-right">{num(l.movement)}</td>
            <td className="text-right">{num(l.closing)}</td>
          </tr>
        ))}
        <tr className="border-b border-gray-50">
          <td className="py-1.5 font-medium">Profit for the period</td>
          <td className="text-right">—</td>
          <td className="text-right font-medium">{num(c.profit_for_period)}</td>
          <td className="text-right">—</td>
        </tr>
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2">Total equity</td>
          <td className="text-right">{num(c.opening_total)}</td>
          <td />
          <td className="text-right">{num(c.closing_total)}</td>
        </tr>
      </tfoot>
    </table>
  );
}
