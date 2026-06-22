import { num } from './primitives';

export default function CashFlowDirectView({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <tbody>
        <tr className="bg-gray-50"><td className="py-1.5 font-semibold text-gray-700" colSpan={2}>Cash receipts</td></tr>
        {c.receipts.map((l: any) => (
          <tr key={'r' + l.account_code} className="border-b border-gray-50"><td className="py-1.5 pl-4">{l.account_name}</td><td className="text-right">{num(l.amount)}</td></tr>
        ))}
        {c.receipts.length === 0 && <tr><td className="py-1.5 pl-4 text-gray-400" colSpan={2}>None</td></tr>}
        <tr className="font-medium border-b"><td className="py-1.5 pl-4">Total receipts</td><td className="text-right">{num(c.total_receipts)}</td></tr>

        <tr className="bg-gray-50"><td className="py-1.5 font-semibold text-gray-700" colSpan={2}>Cash payments</td></tr>
        {c.payments.map((l: any) => (
          <tr key={'p' + l.account_code} className="border-b border-gray-50"><td className="py-1.5 pl-4">{l.account_name}</td><td className="text-right">({num(l.amount)})</td></tr>
        ))}
        {c.payments.length === 0 && <tr><td className="py-1.5 pl-4 text-gray-400" colSpan={2}>None</td></tr>}
        <tr className="font-medium border-b"><td className="py-1.5 pl-4">Total payments</td><td className="text-right">({num(c.total_payments)})</td></tr>

        <tr className="font-bold border-t"><td className="py-2">Net change in cash</td><td className="text-right">{num(c.net_change)}</td></tr>
        <tr className="border-b border-gray-50"><td className="py-1.5 text-gray-500">Opening cash</td><td className="text-right text-gray-500">{num(c.opening_cash)}</td></tr>
        <tr className="font-bold border-t-2"><td className="py-2">Closing cash</td><td className="text-right">{num(c.closing_cash)}</td></tr>
      </tbody>
    </table>
  );
}
