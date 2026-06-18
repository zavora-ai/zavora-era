import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getCustomReports, getCustomReport, saveCustomReport, deleteCustomReport, runCustomReport, getSettings,
} from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency } from '../../utils/format';
import { Plus, Trash2, Play, Save, Printer } from 'lucide-react';

type Row = {
  key: string;
  kind: 'header' | 'accounts' | 'subtotal';
  label: string;
  from_code?: string;
  to_code?: string;
  sign?: 'debit' | 'credit';
  refs?: string[];
};

const newKey = () => Math.random().toString(36).slice(2, 9);

export default function CustomReportsPage() {
  const qc = useQueryClient();
  const { data: listRes } = useQuery({ queryKey: ['custom-reports'], queryFn: getCustomReports });
  const definitions: { id: string; name: string }[] = listRes?.data ?? [];
  const { data: settingsRes } = useQuery({ queryKey: ['settings'], queryFn: getSettings });
  const branding = settingsRes?.data?.branding ?? {};

  const [id, setId] = useState<string | undefined>(undefined);
  const [name, setName] = useState('');
  const [rows, setRows] = useState<Row[]>([]);
  const today = new Date().toISOString().split('T')[0];
  const [from, setFrom] = useState(`${new Date().getFullYear()}-01-01`);
  const [to, setTo] = useState(today);
  const [runResult, setRunResult] = useState<any>(null);

  const load = useMutation({
    mutationFn: (defId: string) => getCustomReport(defId).then((r) => r.data),
    onSuccess: (d) => { setId(d.id); setName(d.name); setRows(d.definition?.rows ?? []); setRunResult(null); },
  });

  const startNew = () => { setId(undefined); setName('New report'); setRows([]); setRunResult(null); };

  const save = useMutation({
    mutationFn: () => saveCustomReport({ id, name, definition: { rows } }).then((r) => r.data),
    onSuccess: (d) => { setId(d.id); qc.invalidateQueries({ queryKey: ['custom-reports'] }); },
  });

  const remove = useMutation({
    mutationFn: () => deleteCustomReport(id!),
    onSuccess: () => { startNew(); qc.invalidateQueries({ queryKey: ['custom-reports'] }); },
  });

  const run = useMutation({
    mutationFn: () => runCustomReport(id!, from, to).then((r) => r.data),
    onSuccess: (d) => setRunResult(d),
  });

  const addRow = (kind: Row['kind']) =>
    setRows((rs) => [...rs, { key: newKey(), kind, label: '', ...(kind === 'accounts' ? { from_code: '', to_code: '', sign: 'debit' as const } : {}), ...(kind === 'subtotal' ? { refs: [] } : {}) }]);
  const updateRow = (k: string, patch: Partial<Row>) => setRows((rs) => rs.map((r) => (r.key === k ? { ...r, ...patch } : r)));
  const deleteRow = (k: string) => setRows((rs) => rs.filter((r) => r.key !== k));
  const move = (k: string, dir: -1 | 1) => setRows((rs) => {
    const i = rs.findIndex((r) => r.key === k); const j = i + dir;
    if (i < 0 || j < 0 || j >= rs.length) return rs;
    const next = [...rs]; [next[i], next[j]] = [next[j], next[i]]; return next;
  });

  return (
    <div>
      <div className="no-print">
        <PageHeader title="Custom Report Builder" subtitle="Define statement rows (headers, account ranges, subtotals) and run them over any period" />

        <div className="flex gap-2 mb-4 flex-wrap items-center">
          <select className="input min-w-[16rem]" value={id ?? ''} onChange={(e) => e.target.value ? load.mutate(e.target.value) : startNew()}>
            <option value="">— New report —</option>
            {definitions.map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
          </select>
          <button className="btn-secondary" onClick={startNew}><Plus className="w-4 h-4" /> New</button>
          {id && <button className="btn-secondary text-red-600" onClick={() => remove.mutate()}><Trash2 className="w-4 h-4" /> Delete</button>}
        </div>

        <div className="card p-4 mb-4">
          <label className="label">Report name</label>
          <input className="input w-full max-w-md mb-4" value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Management P&L" />

          <table className="w-full text-sm">
            <thead>
              <tr className="text-xs text-gray-500 uppercase border-b">
                <th className="text-left py-1.5 w-24">Type</th>
                <th className="text-left">Label</th>
                <th className="text-left">Detail</th>
                <th className="w-24"></th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.key} className="border-b border-gray-50 align-top">
                  <td className="py-2 capitalize text-gray-500">{r.kind}</td>
                  <td className="py-2"><input className="input w-full" value={r.label} onChange={(e) => updateRow(r.key, { label: e.target.value })} placeholder="Row label" /></td>
                  <td className="py-2">
                    {r.kind === 'accounts' && (
                      <div className="flex items-center gap-2">
                        <input className="input w-20" value={r.from_code ?? ''} onChange={(e) => updateRow(r.key, { from_code: e.target.value })} placeholder="4000" />
                        <span className="text-gray-400">to</span>
                        <input className="input w-20" value={r.to_code ?? ''} onChange={(e) => updateRow(r.key, { to_code: e.target.value })} placeholder="4999" />
                        <select className="input w-28" value={r.sign ?? 'debit'} onChange={(e) => updateRow(r.key, { sign: e.target.value as Row['sign'] })}>
                          <option value="debit">Debit +</option>
                          <option value="credit">Credit +</option>
                        </select>
                      </div>
                    )}
                    {r.kind === 'subtotal' && (
                      <div className="flex flex-wrap gap-2">
                        {rows.filter((o) => o.key !== r.key && o.kind !== 'header').map((o) => (
                          <label key={o.key} className="text-xs flex items-center gap-1">
                            <input type="checkbox" checked={r.refs?.includes(o.key) ?? false}
                              onChange={(e) => updateRow(r.key, { refs: e.target.checked ? [...(r.refs ?? []), o.key] : (r.refs ?? []).filter((x) => x !== o.key) })} />
                            {o.label || '(unlabelled)'}
                          </label>
                        ))}
                      </div>
                    )}
                  </td>
                  <td className="py-2 text-right whitespace-nowrap">
                    <button className="text-gray-400 hover:text-gray-700 px-1" onClick={() => move(r.key, -1)}>↑</button>
                    <button className="text-gray-400 hover:text-gray-700 px-1" onClick={() => move(r.key, 1)}>↓</button>
                    <button className="text-red-500 hover:text-red-700 px-1" onClick={() => deleteRow(r.key)}><Trash2 className="w-3.5 h-3.5 inline" /></button>
                  </td>
                </tr>
              ))}
              {rows.length === 0 && <tr><td colSpan={4} className="py-4 text-center text-gray-400">No rows yet — add a header, account range, or subtotal.</td></tr>}
            </tbody>
          </table>

          <div className="flex gap-2 mt-3">
            <button className="btn-secondary" onClick={() => addRow('header')}><Plus className="w-4 h-4" /> Header</button>
            <button className="btn-secondary" onClick={() => addRow('accounts')}><Plus className="w-4 h-4" /> Account range</button>
            <button className="btn-secondary" onClick={() => addRow('subtotal')}><Plus className="w-4 h-4" /> Subtotal</button>
            <div className="flex-1" />
            <button className="btn-primary" disabled={!name || save.isPending} onClick={() => save.mutate()}><Save className="w-4 h-4" /> {save.isPending ? 'Saving…' : 'Save'}</button>
          </div>
        </div>

        <div className="card p-4 mb-5 flex items-end gap-3">
          <div><label className="label">From</label><input type="date" className="input" value={from} onChange={(e) => setFrom(e.target.value)} /></div>
          <div><label className="label">To</label><input type="date" className="input" value={to} onChange={(e) => setTo(e.target.value)} /></div>
          <button className="btn-primary" disabled={!id || run.isPending} onClick={() => run.mutate()} title={!id ? 'Save the report first' : ''}>
            <Play className="w-4 h-4" /> {run.isPending ? 'Running…' : 'Run'}
          </button>
          {runResult && <button className="btn-secondary" onClick={() => window.print()}><Printer className="w-4 h-4" /> Print</button>}
        </div>
      </div>

      {runResult && (
        <div className="print-area mx-auto max-w-3xl bg-white border border-gray-200 rounded-lg shadow-sm">
          <div className="px-10 pt-10 pb-6 border-b text-center">
            <h1 className="text-xl font-bold text-gray-900">{branding.company_name || 'Your Company'}</h1>
            <h2 className="text-lg font-semibold mt-3">{runResult.name}</h2>
            <p className="text-sm text-gray-500">For the period {runResult.period_from} to {runResult.period_to}</p>
          </div>
          <div className="px-10 py-8">
            <table className="w-full text-sm">
              <tbody>
                {runResult.rows.map((r: any) => (
                  <tr key={r.key} className={`border-b border-gray-50 ${r.bold ? 'font-bold border-t' : ''} ${r.kind === 'header' ? 'bg-gray-50' : ''}`}>
                    <td className="py-1.5">{r.label}</td>
                    <td className="text-right tabular-nums">{r.amount != null ? formatCurrency(Number(r.amount)) : ''}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
