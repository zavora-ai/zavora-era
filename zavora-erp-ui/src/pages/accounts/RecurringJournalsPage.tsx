import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getRecurringJournals, saveRecurringJournal, deleteRecurringJournal, runRecurringJournals, getAccounts } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency } from '../../utils/format';
import { Plus, Trash2, Save } from 'lucide-react';

type Line = { account_code: string; debit: string; credit: string; description: string };
const emptyLine = (): Line => ({ account_code: '', debit: '', credit: '', description: '' });

export default function RecurringJournalsPage() {
  const qc = useQueryClient();
  const { data: listRes } = useQuery({ queryKey: ['recurring-journals'], queryFn: getRecurringJournals });
  const templates: any[] = listRes?.data ?? [];
  const { data: accountsRes } = useQuery({ queryKey: ['accounts'], queryFn: getAccounts });
  const accounts: any[] = accountsRes?.data ?? [];

  const today = new Date().toISOString().split('T')[0];
  const [id, setId] = useState<string | undefined>(undefined);
  const [name, setName] = useState('');
  const [cadence, setCadence] = useState('monthly');
  const [autoReverse, setAutoReverse] = useState(false);
  const [nextRun, setNextRun] = useState(today);
  const [lines, setLines] = useState<Line[]>([emptyLine(), emptyLine()]);

  const reset = () => { setId(undefined); setName(''); setCadence('monthly'); setAutoReverse(false); setNextRun(today); setLines([emptyLine(), emptyLine()]); };
  const load = (t: any) => {
    setId(t.id); setName(t.name); setCadence(t.cadence); setAutoReverse(t.auto_reverse); setNextRun(t.next_run_date);
    setLines((t.lines ?? []).map((l: any) => ({ account_code: l.account_code ?? '', debit: l.debit ? String(l.debit) : '', credit: l.credit ? String(l.credit) : '', description: l.description ?? '' })));
  };

  const totalDr = lines.reduce((s, l) => s + (Number(l.debit) || 0), 0);
  const totalCr = lines.reduce((s, l) => s + (Number(l.credit) || 0), 0);
  const balanced = Math.abs(totalDr - totalCr) < 0.01 && totalDr > 0;

  const save = useMutation({
    mutationFn: () => saveRecurringJournal({
      id, name, cadence, auto_reverse: autoReverse, next_run_date: nextRun,
      lines: lines.filter((l) => l.account_code && (Number(l.debit) || Number(l.credit)))
        .map((l) => ({ account_code: l.account_code, debit: Number(l.debit) || undefined, credit: Number(l.credit) || undefined, description: l.description || undefined })),
    }),
    onSuccess: (r) => { setId(r.data.id); qc.invalidateQueries({ queryKey: ['recurring-journals'] }); },
  });
  const remove = useMutation({ mutationFn: () => deleteRecurringJournal(id!), onSuccess: () => { reset(); qc.invalidateQueries({ queryKey: ['recurring-journals'] }); } });
  const runNow = useMutation({ mutationFn: () => runRecurringJournals(), onSuccess: () => qc.invalidateQueries({ queryKey: ['recurring-journals'] }) });

  const upd = (i: number, k: keyof Line, v: string) => setLines((ls) => ls.map((l, idx) => idx === i ? { ...l, [k]: v } : l));

  return (
    <div>
      <PageHeader title="Recurring Journals" subtitle="Templates the scheduler posts automatically — monthly accruals (auto-reverse next month) and prepayment amortisation." />

      <div className="flex gap-2 mb-4 flex-wrap items-center">
        <select className="input min-w-[16rem]" value={id ?? ''} onChange={(e) => e.target.value ? load(templates.find((t) => t.id === e.target.value)) : reset()}>
          <option value="">— New template —</option>
          {templates.map((t) => <option key={t.id} value={t.id}>{t.name} ({t.cadence}{t.auto_reverse ? ', auto-reverse' : ''})</option>)}
        </select>
        <button className="btn-secondary" onClick={reset}><Plus className="w-4 h-4" /> New</button>
        {id && <button className="btn-secondary text-red-600" onClick={() => remove.mutate()}><Trash2 className="w-4 h-4" /> Delete</button>}
        <div className="flex-1" />
        <button className="btn-secondary" disabled={runNow.isPending} onClick={() => runNow.mutate()} title="Post any templates due today">
          {runNow.isPending ? 'Running…' : 'Run due now'}
        </button>
        {runNow.isSuccess && <span className="text-xs text-green-700">Posted {runNow.data?.data?.posted} entr{runNow.data?.data?.posted === 1 ? 'y' : 'ies'}</span>}
      </div>

      <div className="card p-4 mb-4 flex flex-wrap items-end gap-4">
        <div><label className="label">Name</label><input className="input w-56" value={name} onChange={(e) => setName(e.target.value)} placeholder="Monthly rent accrual" /></div>
        <div><label className="label">Cadence</label><select className="input" value={cadence} onChange={(e) => setCadence(e.target.value)}><option value="weekly">Weekly</option><option value="monthly">Monthly</option><option value="quarterly">Quarterly</option></select></div>
        <div><label className="label">Next run</label><input type="date" className="input" value={nextRun} onChange={(e) => setNextRun(e.target.value)} /></div>
        <label className="flex items-center gap-2 text-sm text-gray-600 pb-2"><input type="checkbox" checked={autoReverse} onChange={(e) => setAutoReverse(e.target.checked)} /> Auto-reverse (accrual)</label>
      </div>

      <div className="card p-5 mb-4">
        <table className="w-full text-sm">
          <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Account</th><th className="text-left">Description</th><th className="text-right">Debit</th><th className="text-right">Credit</th><th></th></tr></thead>
          <tbody>
            {lines.map((l, i) => (
              <tr key={i} className="border-b border-gray-50">
                <td className="py-1.5">
                  <select className="input w-full" value={l.account_code} onChange={(e) => upd(i, 'account_code', e.target.value)}>
                    <option value="">Select…</option>
                    {accounts.map((a) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
                  </select>
                </td>
                <td><input className="input w-full" value={l.description} onChange={(e) => upd(i, 'description', e.target.value)} placeholder="Narration" /></td>
                <td className="text-right"><input type="number" step="0.01" className="input w-28 text-right" value={l.debit} onChange={(e) => upd(i, 'debit', e.target.value)} /></td>
                <td className="text-right"><input type="number" step="0.01" className="input w-28 text-right" value={l.credit} onChange={(e) => upd(i, 'credit', e.target.value)} /></td>
                <td className="text-right"><button className="text-red-500 px-1" onClick={() => setLines((ls) => ls.length > 2 ? ls.filter((_, idx) => idx !== i) : ls)}><Trash2 className="w-3.5 h-3.5" /></button></td>
              </tr>
            ))}
          </tbody>
          <tfoot>
            <tr className="font-medium border-t"><td colSpan={2} className="py-2">Total</td><td className="text-right">{formatCurrency(totalDr)}</td><td className="text-right">{formatCurrency(totalCr)}</td><td /></tr>
          </tfoot>
        </table>
        <div className="flex items-center gap-3 mt-3">
          <button className="btn-secondary" onClick={() => setLines((ls) => [...ls, emptyLine()])}><Plus className="w-4 h-4" /> Add line</button>
          <span className={`text-xs ${balanced ? 'text-green-700' : 'text-amber-700'}`}>{balanced ? 'Balanced' : `Out of balance by ${formatCurrency(Math.abs(totalDr - totalCr))}`}</span>
          <div className="flex-1" />
          <button className="btn-primary" disabled={!name || !balanced || save.isPending} onClick={() => save.mutate()}><Save className="w-4 h-4" /> {save.isPending ? 'Saving…' : 'Save template'}</button>
        </div>
      </div>
    </div>
  );
}
