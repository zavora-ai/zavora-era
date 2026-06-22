import { formatCurrency } from '../../../utils/format';
import { num } from './primitives';

export default function VatReturnView({ c }: { c: any }) {
  return (
    <table className="w-full max-w-lg text-sm">
      <tbody>
        <tr className="border-b border-gray-50"><td className="py-1.5">Output VAT (on sales)</td><td className="text-right">{num(c.output_vat)}</td></tr>
        <tr className="border-b border-gray-50"><td className="py-1.5">Input VAT (on purchases)</td><td className="text-right">{num(c.input_vat)}</td></tr>
        <tr className="font-bold border-t-2">
          <td className="py-2">{c.is_payable ? 'Net VAT payable to KRA' : 'Net VAT credit carried forward'}</td>
          <td className={`text-right ${c.is_payable ? 'text-red-600' : 'text-green-600'}`}>{formatCurrency(Math.abs(c.net_vat))}</td>
        </tr>
      </tbody>
    </table>
  );
}
