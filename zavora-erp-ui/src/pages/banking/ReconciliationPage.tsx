import { useState, useMemo } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { getBankAccounts, computeBankRec, completeBankRec, getBankRecs } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency, formatDate } from '../../utils/format';
import { CheckCircle2, AlertTriangle, Lock } from 'lucide-react';

export default function ReconciliationPage() {
  const today = new Date().toISOString().split('T')[0];
  const { data: bankRes } = useQuery({ queryKey: ['bank-accounts'], queryFn: getBankAccounts });
  const banks: any[] = bankRes?.data ?? [];

  const [bankId, setBankId] = useState('');
  const [stmtDate, setStmtDate] = useState(today);
  const [closingBal, setClosingBal] = useState('');
  const [computed, setComputed] = useState<any>(null);
  const [checked, setChecked] = useState<Record<string, boolean>>({});

  const { data: recsRes, refetch: refetchRecs } = useQuery({ queryKey: ['bank-recs', bankId], queryFn: () => getBankRecs(bankId), enabled: !!bankId });
  const recs: any[] = recsRes?.data ?? [];

  const compute = useMutation({
    mutationFn: () => computeBankRec({ bank_account_id: bankId, statement_date: stmtDate }).then((r) => r.data),
    onSuccess: (d) => { setComputed(d); const m: Record<string, boolean> = {}; d.uncleared.forEach((u: any) => { m[u.journal_entry_id] = true; }); setChecked(m); },
  });

  const { clearedTotal, difference, reconciled } = useMemo(() => {
    if (!computed) return { clearedTotal: 0, difference: 0, reconciled: false };
    const newly = computed.uncleared.filter((u: any) => checked[u.journal_entry_id]).reduce((s: number, u: any) => s + Number(u.amount), 0);
    const clearedTotal = Number(computed.prior_cleared) + newly;
    const difference = Number(closingBal || 0) - clearedTotal;
    return { clearedTotal, difference, reconciled: Math.abs(difference) < 0.01 && closingBal !== '' };
  }, [computed, checked, closingBal]);

  const complete = useMutation({
    mutationFn: () => completeBankRec({
      bank_account_id: bankId, statement_date: stmtDate, statement_closing_balance: Number(closingBal),
      cleared_entry_ids: computed.uncleared.filter((u: any) => checked[u.journal_entry_id]).map((u: any) => u.journal_entry_id),
    }),
    onSuccess: () => { setComputed(null); setClosingBal(''); refetchRecs(); },
  });

  return (
    <div>
      <PageHeader title="Bank Reconciliation" subtitle="Tie the ledger to a bank statement: tick the transactions that appear on the statement, then complete & lock when it balances." />

      <div className="card p-4 mb-5 flex flex-wrap items-end gap-4">
        <div>
          <label className="label">Bank account</label>
          <select className="input min-w-[14rem]" value={bankId} onChange={(e) => { setBankId(e.target.value); setComputed(null); }}>
            <option value="">Select…</option>
            {banks.map((b) => <option key={b.id} value={b.id}>{b.name} — {b.bank_name}</option>)}
          </select>
        </div>
        <div><label className="label">Statement date</label><input type="date" className="input" value={stmtDate} onChange={(e) => setStmtDate(e.target.value)} /></div>
        <div><label className="label">Statement closing balance</label><input type="number" step="0.01" className="input" value={closingBal} onChange={(e) => setClosingBal(e.target.value)} /></div>
        <button className="btn-primary" disabled={!bankId || compute.isPending} onClick={() => compute.mutate()}>{compute.isPending ? 'Computing…' : 'Start reconciliation'}</button>
      </div>

      {computed && (
        <>
          <div className="card p-4 mb-4 grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div><p className="label">GL balance</p><p className="font-medium tabular-nums">{formatCurrency(Number(computed.gl_balance))}</p></div>
            <div><p className="label">Cleared (selected)</p><p className="font-medium tabular-nums">{formatCurrency(clearedTotal)}</p></div>
            <div><p className="label">Statement balance</p><p className="font-medium tabular-nums">{formatCurrency(Number(closingBal || 0))}</p></div>
            <div>
              <p className="label">Difference</p>
              {reconciled
                ? <span className="inline-flex items-center gap-1 text-green-700 font-medium"><CheckCircle2 className="w-4 h-4" /> Reconciled</span>
                : <span className="inline-flex items-center gap-1 text-amber-700 font-medium"><AlertTriangle className="w-4 h-4" /> {formatCurrency(Math.abs(difference))}</span>}
            </div>
          </div>

          <div className="card p-5 mb-4">
            <div className="flex items-center justify-between mb-3">
              <p className="text-sm text-gray-500">Tick each transaction that appears on the bank statement. Outstanding (unticked) items are timing differences.</p>
              <button className="btn-primary" disabled={!reconciled || complete.isPending} onClick={() => complete.mutate()}>
                <Lock className="w-4 h-4" /> {complete.isPending ? 'Completing…' : 'Complete & lock'}
              </button>
            </div>
            {complete.isError && <p className="text-sm text-red-600 mb-2">{(complete.error as any)?.response?.data?.error ?? 'Failed'}</p>}
            <table className="w-full text-sm">
              <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="w-10"></th><th className="text-left py-2">Date</th><th className="text-left">JE #</th><th className="text-left">Reference</th><th className="text-right">Amount</th></tr></thead>
              <tbody>
                {computed.uncleared.map((u: any) => (
                  <tr key={u.journal_entry_id} className="border-b border-gray-50">
                    <td className="text-center"><input type="checkbox" checked={checked[u.journal_entry_id] ?? false} onChange={(e) => setChecked((m) => ({ ...m, [u.journal_entry_id]: e.target.checked }))} /></td>
                    <td className="py-1.5">{u.date}</td>
                    <td className="font-mono text-xs">{u.number}</td>
                    <td className="text-gray-500">{u.reference}</td>
                    <td className="text-right tabular-nums">{formatCurrency(Number(u.amount))}</td>
                  </tr>
                ))}
                {computed.uncleared.length === 0 && <tr><td colSpan={5} className="py-4 text-center text-gray-400">No uncleared transactions up to this date.</td></tr>}
              </tbody>
            </table>
          </div>
        </>
      )}

      {bankId && recs.length > 0 && (
        <div className="card p-5">
          <h3 className="font-semibold text-gray-900 mb-2">Completed reconciliations</h3>
          <table className="w-full text-sm">
            <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Statement date</th><th className="text-right">Statement</th><th className="text-right">GL</th><th className="text-right">Cleared</th><th className="text-left pl-4">Completed</th></tr></thead>
            <tbody>
              {recs.map((r) => (
                <tr key={r.id} className="border-b border-gray-50">
                  <td className="py-1.5">{r.statement_date}</td>
                  <td className="text-right tabular-nums">{formatCurrency(Number(r.statement_closing_balance))}</td>
                  <td className="text-right tabular-nums">{formatCurrency(Number(r.gl_balance))}</td>
                  <td className="text-right tabular-nums">{formatCurrency(Number(r.cleared_balance))}</td>
                  <td className="pl-4 text-gray-500">{formatDate(r.completed_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
