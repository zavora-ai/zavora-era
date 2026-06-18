import { formatCurrency } from '../../utils/format';

export interface LineItem {
  description: string;
  quantity: number;
  unit_price: number;
  discount_percent?: number;
  vat_amount?: number;
  line_total: number;
}

interface DocumentLineItemsProps {
  lines: LineItem[];
  currency?: string;
  subtotal: number;
  taxTotal: number;
  grossTotal: number;
}

export default function DocumentLineItems({ lines, currency = 'KES', subtotal, taxTotal, grossTotal }: DocumentLineItemsProps) {
  return (
    <div className="border rounded-lg overflow-hidden">
      <table className="w-full">
        <thead>
          <tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
            <th className="px-4 py-3 text-left">Description</th>
            <th className="px-4 py-3 text-right">Qty</th>
            <th className="px-4 py-3 text-right">Unit Price</th>
            <th className="px-4 py-3 text-right">Disc %</th>
            <th className="px-4 py-3 text-right">VAT</th>
            <th className="px-4 py-3 text-right">Line Total</th>
          </tr>
        </thead>
        <tbody className="divide-y">
          {lines.map((line, i) => (
            <tr key={i}>
              <td className="px-4 py-3 text-sm text-gray-900">{line.description}</td>
              <td className="px-4 py-3 text-sm text-right text-gray-700">{line.quantity}</td>
              <td className="px-4 py-3 text-sm text-right text-gray-700">{formatCurrency(line.unit_price, currency)}</td>
              <td className="px-4 py-3 text-sm text-right text-gray-700">{line.discount_percent ? `${line.discount_percent}%` : '—'}</td>
              <td className="px-4 py-3 text-sm text-right text-gray-700">{line.vat_amount != null ? formatCurrency(line.vat_amount, currency) : '—'}</td>
              <td className="px-4 py-3 text-sm text-right font-medium text-gray-900">{formatCurrency(line.line_total, currency)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot className="bg-gray-50">
          <tr className="border-t">
            <td colSpan={5} className="px-4 py-2 text-sm text-right font-medium text-gray-600">Subtotal</td>
            <td className="px-4 py-2 text-sm text-right font-medium">{formatCurrency(subtotal, currency)}</td>
          </tr>
          <tr>
            <td colSpan={5} className="px-4 py-2 text-sm text-right text-gray-600">VAT</td>
            <td className="px-4 py-2 text-sm text-right">{formatCurrency(taxTotal, currency)}</td>
          </tr>
          <tr className="border-t">
            <td colSpan={5} className="px-4 py-2 text-right font-bold">Total</td>
            <td className="px-4 py-2 text-right font-bold">{formatCurrency(grossTotal, currency)}</td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}
