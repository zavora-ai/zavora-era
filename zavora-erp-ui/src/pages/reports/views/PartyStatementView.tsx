import { num } from './primitives';

export default function PartyStatementView({ c }: { c: any }) {
  return (
    <div>
      <p className="text-sm mb-3"><span className="text-gray-500">Statement for:</span> <span className="font-semibold text-gray-900">{c.party_name}</span></p>
      <table className="w-full text-sm">
        <thead>
          <tr className="text-xs text-gray-500 uppercase border-b">
            <th className="text-left py-2">Date</th>
            <th className="text-left">Type</th>
            <th className="text-left">Reference</th>
            <th className="text-right">Charge</th>
            <th className="text-right">Payment</th>
            <th className="text-right">Balance</th>
          </tr>
        </thead>
        <tbody>
          <tr className="border-b border-gray-50 text-gray-500">
            <td className="py-1.5" colSpan={5}>Opening balance</td>
            <td className="text-right font-medium">{num(c.opening_balance)}</td>
          </tr>
          {c.lines.map((l: any, i: number) => (
            <tr key={i} className="border-b border-gray-50">
              <td className="py-1.5">{l.date}</td>
              <td>{l.doc_type}</td>
              <td className="text-gray-500">{l.reference}</td>
              <td className="text-right">{Number(l.charge) ? num(l.charge) : '—'}</td>
              <td className="text-right">{Number(l.payment) ? num(l.payment) : '—'}</td>
              <td className="text-right">{num(l.balance)}</td>
            </tr>
          ))}
          {c.lines.length === 0 && (
            <tr><td colSpan={6} className="py-4 text-center text-gray-400">No activity in this period</td></tr>
          )}
        </tbody>
        <tfoot>
          <tr className="font-bold border-t-2">
            <td className="py-2" colSpan={3}>Closing balance</td>
            <td className="text-right">{num(c.total_charges)}</td>
            <td className="text-right">{num(c.total_payments)}</td>
            <td className="text-right">{num(c.closing_balance)}</td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}
