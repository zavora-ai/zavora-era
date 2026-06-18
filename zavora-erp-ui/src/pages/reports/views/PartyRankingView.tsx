import { num } from './primitives';

export default function PartyRankingView({ c }: { c: any }) {
  const party = c.party_kind === 'vendor' ? 'Vendor' : 'Customer';
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">{party}</th>
          <th className="text-right">Documents</th>
          <th className="text-right">Amount</th>
          <th className="text-right">% of Total</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any) => (
          <tr key={l.party_id} className="border-b border-gray-50">
            <td className="py-1.5">{l.party_name}</td>
            <td className="text-right text-gray-500">{l.document_count}</td>
            <td className="text-right">{num(l.amount)}</td>
            <td className="text-right text-gray-500">{Number(l.percent).toFixed(1)}%</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={4} className="py-4 text-center text-gray-400">No activity in this period</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2">Total</td>
          <td />
          <td className="text-right">{num(c.total)}</td>
          <td className="text-right">100.0%</td>
        </tr>
      </tfoot>
    </table>
  );
}
