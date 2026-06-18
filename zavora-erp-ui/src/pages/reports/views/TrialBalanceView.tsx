import { num, AccountCell } from './primitives';

export default function TrialBalanceView({ c, onDrill }: { c: any; onDrill?: (code: string) => void }) {
  return (
    <table className="w-full text-sm">
      <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Account</th><th className="text-right">Debit</th><th className="text-right">Credit</th></tr></thead>
      <tbody>
        {c.lines.map((l: any) => (
          <tr key={l.account_code} className="border-b border-gray-50">
            <td className="py-1.5"><AccountCell code={l.account_code} name={l.account_name} onDrill={onDrill} /></td>
            <td className="text-right">{l.closing_debit ? num(l.closing_debit) : '—'}</td>
            <td className="text-right">{l.closing_credit ? num(l.closing_credit) : '—'}</td>
          </tr>
        ))}
      </tbody>
      <tfoot><tr className="font-bold border-t-2"><td className="py-2">Total</td><td className="text-right">{num(c.total_debits)}</td><td className="text-right">{num(c.total_credits)}</td></tr></tfoot>
    </table>
  );
}
