import { Section, TwoColHead, PnlRow } from './primitives';

export default function ProfitAndLossView({ c, onDrill }: { c: any; onDrill?: (code: string) => void }) {
  const cmp = c.comparative_from != null;
  const cmpLabel = cmp ? `${c.comparative_from} – ${c.comparative_to}` : undefined;
  return (
    <table className="w-full text-sm">
      <thead><TwoColHead label={`${c.period_from} – ${c.period_to}`} comparative={cmpLabel} /></thead>
      <tbody>
        {c.revenue.map((s: any, i: number) => <Section key={'r' + i} title={s.name} section={s} comparative={cmpLabel} onDrill={onDrill} />)}
        {c.cost_of_sales.map((s: any, i: number) => <Section key={'c' + i} title={s.name} section={s} comparative={cmpLabel} onDrill={onDrill} />)}
        <PnlRow label="Gross Profit" amount={c.gross_profit} comparative={c.gross_profit_comparative} cmp={cmp} bold />
        {c.operating_expenses.map((s: any, i: number) => <Section key={'o' + i} title={s.name} section={s} comparative={cmpLabel} onDrill={onDrill} />)}
        <PnlRow label="Operating Profit" amount={c.operating_profit} comparative={c.operating_profit_comparative} cmp={cmp} bold />
        {c.other_income_expense.map((s: any, i: number) => <Section key={'x' + i} title={s.name} section={s} comparative={cmpLabel} onDrill={onDrill} />)}
        <PnlRow label="Net Profit" amount={c.net_profit} comparative={c.net_profit_comparative} cmp={cmp} bold />
      </tbody>
    </table>
  );
}
