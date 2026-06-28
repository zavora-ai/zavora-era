import { num } from './primitives';

export default function PaymentHistoryView({ c }: { c: any }) {
  return (
    <div>
      <p className="text-sm mb-3">
        <span className="text-gray-500">Payment history for:</span>{' '}
        <span className="font-semibold text-gray-900">{c.customer_name}</span>
        <span className="text-gray-400"> · {c.period_from} → {c.period_to}</span>
      </p>
      <table className="w-full text-sm">
        <thead>
          <tr className="text-xs text-gray-500 uppercase border-b">
            <th className="text-left py-2">Date</th>
            <th className="text-left">Payment No</th>
            <th className="text-left">Method</th>
            <th className="text-left">Reference</th>
            <th className="text-right">Amount</th>
            <th className="text-right">Unapplied</th>
            <th className="text-left">Status</th>
          </tr>
        </thead>
        <tbody>
          {c.lines.map((l: any, i: number) => (
            <tr key={i} className="border-b border-gray-50">
              <td className="py-1.5">{l.date}</td>
              <td className="font-mono text-gray-600">{l.number}</td>
              <td>{l.method}</td>
              <td className="text-gray-500">{l.reference || '—'}</td>
              <td className="text-right">{num(l.amount)}</td>
              <td className="text-right">{Number(l.unapplied) ? num(l.unapplied) : '—'}</td>
              <td className="capitalize text-gray-500">{l.status}</td>
            </tr>
          ))}
          {c.lines.length === 0 && (
            <tr><td colSpan={7} className="py-4 text-center text-gray-400">No payments in this period</td></tr>
          )}
        </tbody>
        <tfoot>
          <tr className="font-bold border-t-2">
            <td className="py-2" colSpan={4}>Total ({c.payment_count})</td>
            <td className="text-right">{num(c.total_received)}</td>
            <td className="text-right">{Number(c.total_unapplied) ? num(c.total_unapplied) : '—'}</td>
            <td></td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}
