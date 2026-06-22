import { useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { getConsolidationEntities, runConsolidatedTrialBalance, getSettings } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency } from '../../utils/format';
import { CheckCircle2, AlertTriangle, Layers, Printer } from 'lucide-react';

export default function ConsolidationPage() {
  const today = new Date().toISOString().split('T')[0];
  const { data: entitiesRes } = useQuery({ queryKey: ['consolidation-entities'], queryFn: getConsolidationEntities });
  const entities: { entity_id: string; name: string; currency: string }[] = entitiesRes?.data ?? [];
  const { data: settingsRes } = useQuery({ queryKey: ['settings'], queryFn: getSettings });
  const branding = settingsRes?.data?.branding ?? {};

  const [selected, setSelected] = useState<string[]>([]);
  const [asAt, setAsAt] = useState(today);

  const run = useMutation({
    mutationFn: () => runConsolidatedTrialBalance({ entity_ids: selected, as_at: asAt }).then((r) => r.data),
  });

  const toggle = (id: string) =>
    setSelected((s) => (s.includes(id) ? s.filter((x) => x !== id) : [...s, id]));

  const r = run.data;

  return (
    <div>
      <div className="no-print">
        <PageHeader title="Multi-Entity Consolidation" subtitle="Consolidated trial balance across the entities you belong to" />

        <div className="card p-4 mb-5">
          <label className="label mb-1">Entities to consolidate</label>
          {entities.length === 0 && <p className="text-sm text-gray-400">You only have access to one entity, or none are available to consolidate.</p>}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-2 mb-4">
            {entities.map((e) => (
              <label key={e.entity_id} className="flex items-center gap-2 text-sm card p-2 cursor-pointer">
                <input type="checkbox" checked={selected.includes(e.entity_id)} onChange={() => toggle(e.entity_id)} />
                <span className="flex-1">{e.name}</span>
                <span className="text-xs text-gray-400">{e.currency}</span>
              </label>
            ))}
          </div>
          <div className="flex items-end gap-3">
            <div><label className="label">As at</label><input type="date" className="input" value={asAt} onChange={(e) => setAsAt(e.target.value)} /></div>
            <button className="btn-primary" disabled={selected.length === 0 || run.isPending} onClick={() => run.mutate()}>
              <Layers className="w-4 h-4" /> {run.isPending ? 'Consolidating…' : 'Consolidate'}
            </button>
            {r && <button className="btn-secondary" onClick={() => window.print()}><Printer className="w-4 h-4" /> Print</button>}
          </div>
        </div>

        {run.isError && <div className="card p-4 mb-5 text-sm text-red-700 bg-red-50 border-red-200">Could not consolidate. Select at least one entity you have access to.</div>}
      </div>

      {r && (
        <div className="print-area mx-auto max-w-3xl bg-white border border-gray-200 rounded-lg shadow-sm">
          <div className="px-10 pt-10 pb-6 border-b">
            <div className="flex items-start justify-between">
              <h1 className="text-xl font-bold text-gray-900">{branding.company_name || 'Group'}</h1>
              {r.is_balanced
                ? <span className="inline-flex items-center gap-1 text-xs font-medium text-green-700 bg-green-50 px-2 py-1 rounded"><CheckCircle2 className="w-3.5 h-3.5" /> Balanced</span>
                : <span className="inline-flex items-center gap-1 text-xs font-medium text-red-700 bg-red-50 px-2 py-1 rounded"><AlertTriangle className="w-3.5 h-3.5" /> Out of balance by {formatCurrency(Math.abs(r.difference))}</span>}
            </div>
            <div className="text-center mt-4">
              <h2 className="text-lg font-semibold">Consolidated Trial Balance</h2>
              <p className="text-sm text-gray-500">As at {r.as_at} · {r.entities.length} entit{r.entities.length === 1 ? 'y' : 'ies'}: {r.entities.map((e: any) => e.name).join(', ')}</p>
              {r.mixed_currency && <p className="text-xs text-amber-700 mt-1">⚠ Entities use different base currencies; amounts are summed without FX translation.</p>}
            </div>
          </div>
          <div className="px-10 py-8">
            <table className="w-full text-sm">
              <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Account</th><th className="text-right">Debit</th><th className="text-right">Credit</th></tr></thead>
              <tbody>
                {r.lines.map((l: any) => (
                  <tr key={l.account_code} className="border-b border-gray-50">
                    <td className="py-1.5"><span className="font-mono text-xs text-gray-400">{l.account_code}</span> {l.account_name}</td>
                    <td className="text-right tabular-nums">{Number(l.closing_debit) ? formatCurrency(Number(l.closing_debit)) : '—'}</td>
                    <td className="text-right tabular-nums">{Number(l.closing_credit) ? formatCurrency(Number(l.closing_credit)) : '—'}</td>
                  </tr>
                ))}
              </tbody>
              <tfoot><tr className="font-bold border-t-2"><td className="py-2">Total</td><td className="text-right tabular-nums">{formatCurrency(Number(r.total_debits))}</td><td className="text-right tabular-nums">{formatCurrency(Number(r.total_credits))}</td></tr></tfoot>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
