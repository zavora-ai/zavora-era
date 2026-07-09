import { CheckCircle2, AlertTriangle } from 'lucide-react';
import { num } from './primitives';

export default function ControlAccountReconView({ c }: { c: any }) {
  return (
    <div className="space-y-6">
      {c.sides.map((s: any) => (
        <div key={s.side} className="report-section">
          <div className="flex items-center justify-between mb-2">
            <p className="font-semibold text-gray-900">
              {s.side === 'AR' ? 'Accounts Receivable' : 'Accounts Payable'}
              <span className="text-xs font-normal text-gray-400"> · {s.open_documents} open document{s.open_documents === 1 ? '' : 's'}</span>
            </p>
            {s.in_balance ? (
              <span className="inline-flex items-center gap-1 text-xs font-medium text-green-700 bg-green-50 px-2 py-1 rounded">
                <CheckCircle2 className="w-3.5 h-3.5" /> In balance
              </span>
            ) : (
              <span className="inline-flex items-center gap-1 text-xs font-medium text-red-700 bg-red-50 px-2 py-1 rounded">
                <AlertTriangle className="w-3.5 h-3.5" /> Difference {num(s.difference)}
              </span>
            )}
          </div>
          <table className="w-full max-w-xl text-sm">
            <tbody>
              <tr className="border-b border-gray-50">
                <td className="py-1.5">Subledger total (open {s.side === 'AR' ? 'invoices' : 'bills'})</td>
                <td className="text-right font-medium">{num(s.subledger_total)}</td>
              </tr>
              {s.control_accounts.map((a: any) => (
                <tr key={a.code} className="border-b border-gray-50">
                  <td className="py-1.5 text-gray-500 pl-4">GL {a.code} {a.name}</td>
                  <td className="text-right text-gray-500">{num(a.balance)}</td>
                </tr>
              ))}
              <tr className="border-b border-gray-50">
                <td className="py-1.5">Control account total</td>
                <td className="text-right font-medium">{num(s.control_total)}</td>
              </tr>
              <tr>
                <td className={`py-1.5 ${s.in_balance ? 'text-gray-500' : 'text-red-600 font-medium'}`}>Difference (subledger − control)</td>
                <td className={`text-right ${s.in_balance ? 'text-gray-500' : 'text-red-600 font-medium'}`}>{num(s.difference)}</td>
              </tr>
            </tbody>
          </table>
        </div>
      ))}
      <p className="text-xs text-gray-400">
        Reconciles current open document balances against the posted GL balance of every AR/AP control account
        (posting defaults plus business-group overrides). A difference means documents and ledger have diverged —
        investigate before period sign-off.
      </p>
    </div>
  );
}
