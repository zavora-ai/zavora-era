import { num } from './primitives';

export default function BudgetVsActualView({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Account</th>
          <th className="text-right">Actual</th>
          <th className="text-right">Budget</th>
          <th className="text-right">Variance</th>
          <th className="text-right">Var %</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any) => {
          const v = Number(l.variance);
          return (
            <tr key={l.account_code} className="border-b border-gray-50">
              <td className="py-1.5"><span className="font-mono text-xs text-gray-400">{l.account_code}</span> {l.account_name}</td>
              <td className="text-right">{num(l.actual)}</td>
              <td className="text-right text-gray-500">{num(l.budget)}</td>
              <td className={`text-right ${v < 0 ? 'text-red-600' : 'text-green-700'}`}>{num(l.variance)}</td>
              <td className="text-right text-gray-500">{l.variance_pct != null ? `${Number(l.variance_pct).toFixed(1)}%` : '—'}</td>
            </tr>
          );
        })}
        {c.lines.length === 0 && (
          <tr><td colSpan={5} className="py-4 text-center text-gray-400">No budget or activity in this period</td></tr>
        )}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2">Total</td>
          <td className="text-right">{num(c.total_actual)}</td>
          <td className="text-right">{num(c.total_budget)}</td>
          <td className="text-right">{num(c.total_variance)}</td>
          <td />
        </tr>
      </tfoot>
    </table>
  );
}
