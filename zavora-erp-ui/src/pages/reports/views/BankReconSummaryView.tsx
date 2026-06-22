import { CheckCircle2, AlertTriangle } from 'lucide-react';
import { num } from './primitives';

export default function BankReconSummaryView({ c }: { c: any }) {
  return (
    <div className="space-y-6">
      {c.accounts.map((a: any) => (
        <div key={a.bank_account_id} className="report-section">
          <div className="flex items-center justify-between mb-2">
            <div>
              <p className="font-semibold text-gray-900">{a.account_name} <span className="text-xs font-normal text-gray-400">· {a.bank_name} · GL {a.gl_account}</span></p>
            </div>
            {a.is_reconciled ? (
              <span className="inline-flex items-center gap-1 text-xs font-medium text-green-700 bg-green-50 px-2 py-1 rounded">
                <CheckCircle2 className="w-3.5 h-3.5" /> Reconciled
              </span>
            ) : (
              <span className="inline-flex items-center gap-1 text-xs font-medium text-amber-700 bg-amber-50 px-2 py-1 rounded">
                <AlertTriangle className="w-3.5 h-3.5" /> Unreconciled
              </span>
            )}
          </div>
          <table className="w-full max-w-xl text-sm">
            <tbody>
              <tr className="border-b border-gray-50"><td className="py-1.5">Balance per bank statement</td><td className="text-right">{num(a.statement_balance)}</td></tr>
              <tr className="border-b border-gray-50"><td className="py-1.5">Balance per general ledger</td><td className="text-right">{num(a.gl_balance)}</td></tr>
              <tr className="border-b border-gray-50"><td className="py-1.5 text-gray-500">Difference</td><td className="text-right text-gray-500">{num(a.difference)}</td></tr>
              <tr className="border-b border-gray-50"><td className="py-1.5">Unreconciled feed items ({a.unmatched_count})</td><td className="text-right">{num(a.unreconciled_amount)}</td></tr>
              <tr className="border-b border-gray-50"><td className="py-1.5 text-gray-500">Matched feed items</td><td className="text-right text-gray-500">{a.matched_count}</td></tr>
            </tbody>
          </table>
        </div>
      ))}
      {c.accounts.length === 0 && <p className="py-4 text-center text-gray-400">No bank accounts</p>}
    </div>
  );
}
