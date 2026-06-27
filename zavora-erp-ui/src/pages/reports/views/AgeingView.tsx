import { num } from './primitives';

// Accounts Receivable / Payable ageing — party balances bucketed by age.
export default function AgeingView({ c, partyLabel = 'Party' }: { c: any; partyLabel?: string }) {
  const lines: any[] = c.lines ?? [];
  const t = c.totals ?? {};
  const n = (x: any) => num(Number(x ?? 0));

  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-gray-500 border-b">
          <th className="text-left py-2 font-medium">{partyLabel}</th>
          <th className="text-right py-2 font-medium">Current</th>
          <th className="text-right py-2 font-medium">1–30</th>
          <th className="text-right py-2 font-medium">31–60</th>
          <th className="text-right py-2 font-medium">61–90</th>
          <th className="text-right py-2 font-medium">90+</th>
          <th className="text-right py-2 font-medium">Total</th>
        </tr>
      </thead>
      <tbody>
        {lines.length === 0 && (
          <tr><td colSpan={7} className="py-6 text-center text-gray-400">Nothing outstanding as at {c.as_at}.</td></tr>
        )}
        {lines.map((l, i) => (
          <tr key={l.party_id ?? i} className="border-b border-gray-50 hover:bg-gray-50">
            <td className="py-1.5">{l.party_name || l.party_id?.slice(0, 8)}</td>
            <td className="text-right">{n(l.current)}</td>
            <td className="text-right">{n(l.days_1_30)}</td>
            <td className="text-right">{n(l.days_31_60)}</td>
            <td className="text-right">{n(l.days_61_90)}</td>
            <td className="text-right">{n(l.over_90)}</td>
            <td className="text-right font-medium">{n(l.total)}</td>
          </tr>
        ))}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2">Total</td>
          <td className="text-right">{n(t.current)}</td>
          <td className="text-right">{n(t.days_1_30)}</td>
          <td className="text-right">{n(t.days_31_60)}</td>
          <td className="text-right">{n(t.days_61_90)}</td>
          <td className="text-right">{n(t.over_90)}</td>
          <td className="text-right">{n(t.total)}</td>
        </tr>
      </tfoot>
    </table>
  );
}
