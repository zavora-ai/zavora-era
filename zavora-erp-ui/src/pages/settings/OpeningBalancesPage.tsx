import { useState, useMemo, useEffect } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { getAccounts, getPeriods, postOpeningBalances } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency } from '../../utils/format';
import { AlertTriangle, CheckCircle2 } from 'lucide-react';

// Enter a business's opening trial balance at the conversion date. A TB must
// balance by definition, so Post is disabled until debits == credits.
export default function OpeningBalancesPage() {
  const { data: accountsRes } = useQuery({ queryKey: ['accounts'], queryFn: getAccounts });
  const accounts: any[] = accountsRes?.data ?? [];
  const { data: periodsRes } = useQuery({ queryKey: ['periods'], queryFn: getPeriods });
  const periods: any[] = periodsRes?.data ?? [];

  // Opening balances sit at the start of the first fiscal period — default to it
  // so the entry always lands in an existing period.
  const [asOf, setAsOf] = useState('');
  useEffect(() => {
    if (!asOf && periods.length) {
      const first = [...periods].sort((a, b) => a.start_date.localeCompare(b.start_date))[0];
      setAsOf(first.start_date);
    }
  }, [periods, asOf]);
  const [vals, setVals] = useState<Record<string, { debit: string; credit: string }>>({});
  const [filter, setFilter] = useState('');

  const set = (code: string, side: 'debit' | 'credit', v: string) =>
    setVals((m) => ({ ...m, [code]: { ...(m[code] ?? { debit: '', credit: '' }), [side]: v } }));

  const { totalDebit, totalCredit } = useMemo(() => {
    let totalDebit = 0, totalCredit = 0;
    Object.values(vals).forEach((v) => { totalDebit += Number(v.debit) || 0; totalCredit += Number(v.credit) || 0; });
    return { totalDebit, totalCredit };
  }, [vals]);
  const diff = totalDebit - totalCredit;
  const balanced = Math.abs(diff) < 0.01 && (totalDebit > 0 || totalCredit > 0);

  const post = useMutation({
    mutationFn: () => postOpeningBalances({
      as_of_date: asOf,
      lines: Object.entries(vals)
        .filter(([, v]) => Number(v.debit) || Number(v.credit))
        .map(([account_code, v]) => ({ account_code, debit: Number(v.debit) || undefined, credit: Number(v.credit) || undefined })),
    }),
  });

  const shown = accounts.filter((a) => !filter || a.code.includes(filter) || a.name.toLowerCase().includes(filter.toLowerCase()));

  return (
    <div>
      <PageHeader title="Opening Balances" subtitle="Enter your trial balance at go-live. It posts as an opening-balance journal once it balances." />

      <div className="card p-4 mb-4 flex flex-wrap items-end gap-4">
        <div><label className="label">As at (conversion date)</label><input type="date" className="input" value={asOf} onChange={(e) => setAsOf(e.target.value)} /></div>
        <div><label className="label">Filter accounts</label><input className="input" value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="code or name" /></div>
        <div className="flex-1" />
        <div className="text-right text-sm">
          <div>Debits <span className="font-medium tabular-nums">{formatCurrency(totalDebit)}</span> · Credits <span className="font-medium tabular-nums">{formatCurrency(totalCredit)}</span></div>
          {balanced
            ? <span className="inline-flex items-center gap-1 text-xs font-medium text-green-700"><CheckCircle2 className="w-3.5 h-3.5" /> Balanced</span>
            : <span className="inline-flex items-center gap-1 text-xs font-medium text-amber-700"><AlertTriangle className="w-3.5 h-3.5" /> Out of balance by {formatCurrency(Math.abs(diff))}</span>}
        </div>
        <button className="btn-primary" disabled={!balanced || post.isPending} onClick={() => post.mutate()}>
          {post.isPending ? 'Posting…' : 'Post opening balances'}
        </button>
      </div>

      {post.isSuccess && <div className="mb-4 text-sm text-green-700">Opening balances posted ({post.data?.data?.number}).</div>}
      {post.isError && <div className="mb-4 text-sm text-red-700">{(post.error as any)?.response?.data?.error ?? 'Could not post opening balances.'}</div>}

      <div className="card p-5">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-xs text-gray-500 uppercase border-b">
              <th className="text-left py-2">Account</th>
              <th className="text-left">Type</th>
              <th className="text-right">Debit</th>
              <th className="text-right">Credit</th>
            </tr>
          </thead>
          <tbody>
            {shown.map((a) => (
              <tr key={a.code} className="border-b border-gray-50">
                <td className="py-1.5"><span className="font-mono text-xs text-gray-400">{a.code}</span> {a.name}</td>
                <td className="text-gray-500">{a.account_type}</td>
                <td className="text-right"><input type="number" step="0.01" className="input w-32 text-right" value={vals[a.code]?.debit ?? ''} onChange={(e) => set(a.code, 'debit', e.target.value)} /></td>
                <td className="text-right"><input type="number" step="0.01" className="input w-32 text-right" value={vals[a.code]?.credit ?? ''} onChange={(e) => set(a.code, 'credit', e.target.value)} /></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
