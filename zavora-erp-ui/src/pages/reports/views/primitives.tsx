// Shared rendering primitives used by the individual report views.
// Extracted verbatim from the original ReportsPage monolith.
import { formatCurrency } from '../../../utils/format';

export const num = (n: number) => <span className="tabular-nums">{formatCurrency(n)}</span>;

// An account label that drills into the General Ledger when clicked.
export function AccountCell({ code, name, onDrill }: { code: string; name: string; onDrill?: (code: string) => void }) {
  const inner = <><span className="font-mono text-xs text-gray-400">{code}</span> {name}</>;
  if (!onDrill || !code) return <>{inner}</>;
  return (
    <button onClick={() => onDrill(code)} className="text-left hover:text-indigo-600 hover:underline cursor-pointer" title="View general ledger">
      {inner}
    </button>
  );
}

export function TwoColHead({ comparative, label }: { comparative?: string; label: string }) {
  return (
    <tr className="text-xs text-gray-500 uppercase border-b">
      <th className="text-left py-2">{label}</th>
      <th className="text-right">Amount</th>
      {comparative && <th className="text-right">{comparative}</th>}
    </tr>
  );
}

export function Section({ title, section, comparative, onDrill }: { title: string; section: any; comparative?: string; onDrill?: (code: string) => void }) {
  return (
    <>
      <tr className="bg-gray-50"><td className="py-1.5 font-semibold text-gray-700" colSpan={comparative ? 3 : 2}>{title}</td></tr>
      {section.lines.map((l: any) => (
        <tr key={l.account_code + l.account_name} className="border-b border-gray-50">
          <td className="py-1.5 pl-4"><AccountCell code={l.account_code} name={l.account_name} onDrill={onDrill} /></td>
          <td className="text-right">{num(l.amount)}</td>
          {comparative && <td className="text-right text-gray-500">{l.comparative != null ? num(l.comparative) : '—'}</td>}
        </tr>
      ))}
      <tr className="font-medium border-b"><td className="py-1.5 pl-4">Total {title}</td><td className="text-right">{num(section.total)}</td>{comparative && <td />}</tr>
    </>
  );
}

export function PnlRow({ label, amount, comparative, cmp, bold }: { label: string; amount: number; comparative?: number | null; cmp: boolean; bold?: boolean }) {
  return (
    <tr className={bold ? 'font-bold border-t' : 'border-b border-gray-50'}>
      <td className="py-1.5">{label}</td>
      <td className="text-right">{num(amount)}</td>
      {cmp && <td className="text-right text-gray-500">{comparative != null ? num(comparative) : '—'}</td>}
    </tr>
  );
}

export function VatBands({ title, bands, totalTaxable, totalVat }: { title: string; bands: any[]; totalTaxable: number; totalVat: number }) {
  return (
    <>
      <tr className="bg-gray-50"><td className="py-1.5 font-semibold text-gray-700" colSpan={4}>{title}</td></tr>
      {bands.map((b: any) => (
        <tr key={b.treatment} className="border-b border-gray-50">
          <td className="py-1.5 pl-4">{b.treatment} <span className="text-xs text-gray-400">({b.document_count} doc{b.document_count === 1 ? '' : 's'})</span></td>
          <td />
          <td className="text-right">{num(b.taxable_amount)}</td>
          <td className="text-right">{num(b.vat_amount)}</td>
        </tr>
      ))}
      {bands.length === 0 && <tr><td className="py-1.5 pl-4 text-gray-400" colSpan={4}>None</td></tr>}
      <tr className="font-medium border-b"><td className="py-1.5 pl-4">Total {title}</td><td /><td className="text-right">{num(totalTaxable)}</td><td className="text-right">{num(totalVat)}</td></tr>
    </>
  );
}
