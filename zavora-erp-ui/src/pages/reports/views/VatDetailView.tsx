import { formatCurrency } from '../../../utils/format';
import { VatBands } from './primitives';

export default function VatDetailView({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Rate band</th><th /><th className="text-right">Taxable</th><th className="text-right">VAT</th>
        </tr>
      </thead>
      <tbody>
        <VatBands title="Output VAT (Sales)" bands={c.output} totalTaxable={c.total_output_taxable} totalVat={c.total_output_vat} />
        <VatBands title="Input VAT (Purchases)" bands={c.input} totalTaxable={c.total_input_taxable} totalVat={c.total_input_vat} />
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={3}>{c.is_payable ? 'Net VAT payable to KRA' : 'Net VAT credit carried forward'}</td>
          <td className={`text-right ${c.is_payable ? 'text-red-600' : 'text-green-600'}`}>{formatCurrency(Math.abs(c.net_vat))}</td>
        </tr>
      </tbody>
    </table>
  );
}
