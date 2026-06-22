import { num } from './primitives';

export default function InventoryValuationView({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">SKU</th>
          <th className="text-left">Description</th>
          <th className="text-left">UoM</th>
          <th className="text-right">On Hand</th>
          <th className="text-right">Unit Cost</th>
          <th className="text-right">Total Value</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5 font-mono text-xs">{l.sku}</td>
            <td>{l.description}</td>
            <td className="text-gray-500">{l.uom}</td>
            <td className="text-right">{Number(l.on_hand).toLocaleString()}</td>
            <td className="text-right">{num(l.unit_cost)}</td>
            <td className="text-right font-medium">{num(l.total_value)}</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={6} className="py-4 text-center text-gray-400">No stock items</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={5}>Total ({c.item_count} item{c.item_count === 1 ? '' : 's'})</td>
          <td className="text-right">{num(c.total_value)}</td>
        </tr>
      </tfoot>
    </table>
  );
}
