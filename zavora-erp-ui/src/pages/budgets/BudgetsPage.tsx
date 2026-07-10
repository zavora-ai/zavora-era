import { useEffect, useMemo, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getAccounts, getPeriods, getBudgets, setBudget } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { Save } from 'lucide-react';

// P&L accounts are what a budget is set against.
const PNL_TYPES = ['Revenue', 'ContraRevenue', 'Expense', 'ContraExpense'];

export default function BudgetsPage() {
  const qc = useQueryClient();
  const { data: accountsRes } = useQuery({ queryKey: ['accounts'], queryFn: getAccounts });
  const { data: periodsRes } = useQuery({ queryKey: ['periods'], queryFn: getPeriods });
  const { data: budgetsRes } = useQuery({ queryKey: ['budgets'], queryFn: getBudgets });

  const accounts: any[] = (accountsRes?.data ?? []).filter((a: any) => PNL_TYPES.includes(a.account_type));
  const periods: any[] = periodsRes?.data ?? [];
  const budgets: any[] = budgetsRes?.data ?? [];

  const [periodId, setPeriodId] = useState('');
  const [amounts, setAmounts] = useState<Record<string, string>>({});

  // Default to the first period once loaded.
  useEffect(() => {
    if (!periodId && periods.length) setPeriodId(periods[0].id);
  }, [periods, periodId]);

  // Prefill amounts from existing budget entries for the selected period.
  const existing = useMemo(() => {
    const m: Record<string, number> = {};
    budgets.filter((b) => b.period_id === periodId).forEach((b) => { m[b.account_code] = Number(b.amount); });
    return m;
  }, [budgets, periodId]);

  useEffect(() => {
    const next: Record<string, string> = {};
    Object.entries(existing).forEach(([code, amt]) => { next[code] = String(amt); });
    setAmounts(next);
  }, [existing]);

  const save = useMutation({
    mutationFn: async () => {
      const entries = accounts
        .map((a) => ({ code: a.code, val: amounts[a.code] }))
        .filter((e) => e.val !== undefined && e.val !== '' && Number(e.val) !== (existing[e.code] ?? 0));
      for (const e of entries) {
        await setBudget({ period_id: periodId, account_code: e.code, amount: Number(e.val) });
      }
      return entries.length;
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ['budgets'] }),
  });

  return (
    <div>
      <PageHeader title="Budgets" subtitle="Set budget figures per account and period; compare in the Budget vs Actual report" />

      <div className="card p-4 mb-5 flex flex-wrap items-end gap-4">
        <div>
          <label className="label">Period</label>
          <select className="input min-w-[14rem]" value={periodId} onChange={(e) => setPeriodId(e.target.value)}>
            {periods.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
          </select>
        </div>
        <div className="flex-1" />
        <button onClick={() => save.mutate()} className="btn-primary" disabled={save.isPending || !periodId}>
          <Save className="w-4 h-4" /> {save.isPending ? 'Saving…' : 'Save budgets'}
        </button>
      </div>
      {save.isSuccess && <div className="mb-4 text-sm text-green-700">Saved {save.data} budget {save.data === 1 ? 'entry' : 'entries'}.</div>}

      <div className="card p-5 overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-xs text-gray-500 uppercase border-b">
              <th className="text-left py-2">Account</th>
              <th className="text-left">Type</th>
              <th className="text-right">Budget amount</th>
            </tr>
          </thead>
          <tbody>
            {accounts.map((a) => (
              <tr key={a.code} className="border-b border-gray-50">
                <td className="py-1.5"><span className="font-mono text-xs text-gray-400">{a.code}</span> {a.name}</td>
                <td className="text-gray-500">{a.account_type}</td>
                <td className="text-right">
                  <input
                    type="number"
                    className="input w-36 text-right"
                    value={amounts[a.code] ?? ''}
                    placeholder="0.00"
                    onChange={(e) => setAmounts((m) => ({ ...m, [a.code]: e.target.value }))}
                  />
                </td>
              </tr>
            ))}
            {accounts.length === 0 && <tr><td colSpan={3} className="py-4 text-center text-gray-400">No P&L accounts found</td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );
}
