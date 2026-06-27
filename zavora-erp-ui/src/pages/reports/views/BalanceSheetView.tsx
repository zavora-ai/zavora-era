import { num, Section, TwoColHead } from './primitives';

export default function BalanceSheetView({ c, onDrill }: { c: any; onDrill?: (code: string) => void }) {
  const cmp = c.comparative_as_at as string | null;
  return (
    <table className="w-full text-sm">
      <thead><TwoColHead label={`As at ${c.as_at}`} comparative={cmp ?? undefined} /></thead>
      <tbody>
        {c.assets.map((s: any, i: number) => <Section key={'a' + i} title={s.name} section={s} comparative={cmp ?? undefined} onDrill={onDrill} />)}
        <tr className="font-bold"><td className="py-2">Total Assets</td><td className="text-right">{num(c.total_assets)}</td>{cmp && <td className="text-right">{c.total_assets_comparative != null ? num(c.total_assets_comparative) : '—'}</td>}</tr>
        {c.liabilities.map((s: any, i: number) => <Section key={'l' + i} title={s.name} section={s} comparative={cmp ?? undefined} onDrill={onDrill} />)}
        {c.equity.map((s: any, i: number) => <Section key={'e' + i} title={s.name} section={s} comparative={cmp ?? undefined} onDrill={onDrill} />)}
        <tr className="font-bold border-t-2"><td className="py-2">Total Liabilities + Equity</td><td className="text-right">{num(Number(c.total_liabilities) + Number(c.total_equity))}</td>{cmp && <td className="text-right">{num(Number(c.total_liabilities_comparative ?? 0) + Number(c.total_equity_comparative ?? 0))}</td>}</tr>
      </tbody>
    </table>
  );
}
