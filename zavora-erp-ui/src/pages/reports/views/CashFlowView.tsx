import { num } from './primitives';

// Indirect-method cash flow statement. Backend shape (CashFlowReport):
//   operating_activities / investing_activities / financing_activities: { lines: [{description, amount}], total }
//   net_change, opening_cash, closing_cash, period_from, period_to
export default function CashFlowView({ c }: { c: any }) {
  const Section = ({ title, section }: { title: string; section: any }) => (
    <>
      <tr className="bg-gray-50"><td className="py-1.5 font-semibold text-gray-700" colSpan={2}>{title}</td></tr>
      {(section?.lines ?? []).map((l: any, i: number) => (
        <tr key={title + i} className="border-b border-gray-50">
          <td className="py-1.5 pl-4">{l.description}</td>
          <td className="text-right">{num(l.amount)}</td>
        </tr>
      ))}
      {(!section?.lines || section.lines.length === 0) && (
        <tr><td className="py-1.5 pl-4 text-gray-400" colSpan={2}>None</td></tr>
      )}
      <tr className="font-medium border-b"><td className="py-1.5 pl-4">Net cash from {title.toLowerCase()}</td><td className="text-right">{num(section?.total ?? 0)}</td></tr>
    </>
  );

  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="border-b-2"><th className="text-left py-2 text-gray-500 font-medium" colSpan={2}>For the period {c.period_from} – {c.period_to}</th></tr>
      </thead>
      <tbody>
        <Section title="Operating activities" section={c.operating_activities} />
        <Section title="Investing activities" section={c.investing_activities} />
        <Section title="Financing activities" section={c.financing_activities} />
        <tr className="font-bold border-t"><td className="py-2">Net change in cash</td><td className="text-right">{num(c.net_change)}</td></tr>
        <tr className="border-b border-gray-50"><td className="py-1.5 text-gray-500">Opening cash</td><td className="text-right text-gray-500">{num(c.opening_cash)}</td></tr>
        <tr className="font-bold border-t-2"><td className="py-2">Closing cash</td><td className="text-right">{num(c.closing_cash)}</td></tr>
      </tbody>
    </table>
  );
}
