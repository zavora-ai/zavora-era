// Branded, full-page statement shell — letterhead + body + footer.
// Extracted faithfully from the monolith's ReportDocument.
import type { ReactNode } from 'react';
import { CheckCircle2, AlertTriangle } from 'lucide-react';
import { formatCurrency } from '../../../utils/format';

function periodLabel(c: any): string {
  if (c?.as_at) return `As at ${c.as_at}`;
  if (c?.period_from && c?.period_to) return `For the period ${c.period_from} to ${c.period_to}`;
  return '';
}

function Balanced({ ok, diff }: { ok: boolean; diff: number }) {
  return ok ? (
    <span className="inline-flex items-center gap-1 text-xs font-medium text-green-700 bg-green-50 px-2 py-1 rounded">
      <CheckCircle2 className="w-3.5 h-3.5" /> Balanced
    </span>
  ) : (
    <span className="inline-flex items-center gap-1 text-xs font-medium text-red-700 bg-red-50 px-2 py-1 rounded">
      <AlertTriangle className="w-3.5 h-3.5" /> Out of balance by {formatCurrency(Math.abs(diff))}
    </span>
  );
}

export default function ReportLayout({ result, branding, children }: { result: any; branding: any; children: ReactNode }) {
  const content = result.content ?? {};
  const key = Object.keys(content)[0];
  const c = content[key];
  const b = branding ?? {};
  const generatedAt = new Date().toLocaleString('en-KE', { dateStyle: 'medium', timeStyle: 'short' });

  return (
    <div id="report-document" className="print-area mx-auto max-w-4xl bg-white border border-gray-200 rounded-lg shadow-sm">
      {/* Letterhead */}
      <div className="px-10 pt-10 pb-6 border-b border-gray-200">
        <div className="flex items-start justify-between gap-6">
          <div className="flex items-center gap-3">
            {b.logo_url && <img src={b.logo_url} alt="" className="h-12 w-auto object-contain" />}
            <div>
              <h1 className="text-xl font-bold text-gray-900 leading-tight">{b.company_name || 'Your Company'}</h1>
              <p className="text-xs text-gray-500 mt-0.5">
                {[b.address, b.phone, b.email].filter(Boolean).join('  ·  ')}
              </p>
              <p className="text-xs text-gray-500">
                {[b.kra_pin && `KRA PIN: ${b.kra_pin}`, b.vat_number && `VAT: ${b.vat_number}`].filter(Boolean).join('  ·  ')}
              </p>
            </div>
          </div>
          <div className="text-right shrink-0">
            {key === 'TrialBalance' && <Balanced ok={c.is_balanced} diff={c.difference} />}
            {key === 'BalanceSheet' && <Balanced ok={c.is_balanced} diff={c.difference} />}
          </div>
        </div>
        <div className="text-center mt-6">
          <h2 className="text-lg font-semibold text-gray-900">{result.title || key}</h2>
          <p className="text-sm text-gray-500 mt-0.5">{periodLabel(c)}</p>
        </div>
      </div>

      {/* Body */}
      <div className="px-10 py-8">
        {children}
      </div>

      {/* Footer */}
      <div className="px-10 py-4 border-t border-gray-200 text-[11px] text-gray-400 flex justify-between">
        <span>{b.footer_text || `${b.company_name || ''}`}</span>
        <span>Generated {generatedAt}</span>
      </div>
    </div>
  );
}
