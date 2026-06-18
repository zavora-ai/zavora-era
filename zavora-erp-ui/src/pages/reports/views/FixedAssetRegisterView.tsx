import { num } from './primitives';

export default function FixedAssetRegisterView({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Asset</th>
          <th className="text-left">Category</th>
          <th className="text-left">Acquired</th>
          <th className="text-right">Cost</th>
          <th className="text-right">Accum. Depr.</th>
          <th className="text-right">Net Book Value</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5">{l.description} <span className="text-xs text-gray-400">{l.asset_number}</span></td>
            <td className="text-gray-500">{l.category}</td>
            <td>{l.acquisition_date}</td>
            <td className="text-right">{num(l.cost)}</td>
            <td className="text-right text-gray-500">{num(l.accumulated_depreciation)}</td>
            <td className="text-right font-medium">{num(l.net_book_value)}</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={6} className="py-4 text-center text-gray-400">No assets on register</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={3}>Total</td>
          <td className="text-right">{num(c.total_cost)}</td>
          <td className="text-right">{num(c.total_accumulated_depreciation)}</td>
          <td className="text-right">{num(c.total_net_book_value)}</td>
        </tr>
      </tfoot>
    </table>
  );
}
