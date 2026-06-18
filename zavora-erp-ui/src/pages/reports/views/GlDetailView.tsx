import { num } from './primitives';

export default function GlDetailView({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Date</th><th className="text-left">JE #</th><th className="text-left">Reference</th><th className="text-right">Debit</th><th className="text-right">Credit</th><th className="text-right">Balance</th></tr></thead>
      <tbody>
        <tr className="border-b border-gray-50 text-gray-500"><td className="py-1.5" colSpan={5}>Opening balance — {c.account_code} {c.account_name}</td><td className="text-right font-medium">{num(c.opening_balance)}</td></tr>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5">{l.date}</td><td className="font-mono text-xs">{l.journal_number}</td><td className="text-gray-500">{l.reference}</td>
            <td className="text-right">{l.debit ? num(l.debit) : '—'}</td><td className="text-right">{l.credit ? num(l.credit) : '—'}</td><td className="text-right">{num(l.balance)}</td>
          </tr>
        ))}
        <tr className="font-bold border-t-2"><td className="py-2" colSpan={5}>Closing balance</td><td className="text-right">{num(c.closing_balance)}</td></tr>
      </tbody>
    </table>
  );
}
