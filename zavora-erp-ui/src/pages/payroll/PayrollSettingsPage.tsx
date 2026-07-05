import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  listEarningTypes, createEarningType, setEarningTypeActive,
  listDeductionTypes, createDeductionType, setDeductionTypeActive,
  listDepartments, createDepartment, listStatutoryConfig, upsertStatutoryConfig,
} from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { Plus } from 'lucide-react';

type Tab = 'earnings' | 'deductions' | 'departments' | 'statutory';

export default function PayrollSettingsPage() {
  const [tab, setTab] = useState<Tab>('earnings');
  const tabs: [Tab, string][] = [['earnings', 'Earning Types'], ['deductions', 'Deduction Types'], ['departments', 'Departments'], ['statutory', 'Statutory Rates']];
  return (
    <div>
      <PageHeader title="Payroll Settings" subtitle="Define earning & deduction types, departments, and statutory rates." />
      <div className="flex gap-1 mb-5 border-b flex-wrap">
        {tabs.map(([t, label]) => (
          <button key={t} onClick={() => setTab(t)} className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px ${tab === t ? 'border-indigo-600 text-indigo-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}>{label}</button>
        ))}
      </div>
      {tab === 'earnings' && <EarningTypes />}
      {tab === 'deductions' && <DeductionTypes />}
      {tab === 'departments' && <Departments />}
      {tab === 'statutory' && <Statutory />}
    </div>
  );
}

function EarningTypes() {
  const qc = useQueryClient();
  const { data = [] } = useQuery<any[]>({ queryKey: ['earning-types'], queryFn: () => listEarningTypes().then(r => r.data) });
  const [f, setF] = useState({ code: '', name: '', taxable: true, pensionable: true, affects_shif: true });
  const [err, setErr] = useState('');
  const inv = () => qc.invalidateQueries({ queryKey: ['earning-types'] });
  const add = useMutation({ mutationFn: () => createEarningType(f), onSuccess: () => { setF({ code: '', name: '', taxable: true, pensionable: true, affects_shif: true }); inv(); }, onError: (e: any) => setErr(e?.response?.data?.error ?? 'Failed') });
  const toggle = useMutation({ mutationFn: (v: any) => setEarningTypeActive(v.id, v.active), onSuccess: inv });
  return (
    <div className="space-y-4">
      {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
      <div className="card p-3 flex flex-wrap items-end gap-2">
        <div><label className="label">Code</label><input className="input py-1 text-sm font-mono w-28" value={f.code} onChange={e => setF({ ...f, code: e.target.value.toUpperCase() })} /></div>
        <div><label className="label">Name</label><input className="input py-1 text-sm w-52" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} /></div>
        <label className="flex items-center gap-1 text-xs"><input type="checkbox" checked={f.taxable} onChange={e => setF({ ...f, taxable: e.target.checked })} /> Taxable</label>
        <label className="flex items-center gap-1 text-xs"><input type="checkbox" checked={f.pensionable} onChange={e => setF({ ...f, pensionable: e.target.checked })} /> Pensionable</label>
        <label className="flex items-center gap-1 text-xs"><input type="checkbox" checked={f.affects_shif} onChange={e => setF({ ...f, affects_shif: e.target.checked })} /> SHIF/Housing base</label>
        <button className="btn-primary py-1" disabled={!f.code || !f.name || add.isPending} onClick={() => { setErr(''); add.mutate(); }}><Plus className="w-4 h-4" /> Add</button>
      </div>
      <TypeTable rows={data} extraCols={[['Pensionable', 'pensionable'], ['SHIF base', 'affects_shif']]} onToggle={(id, active) => toggle.mutate({ id, active })} />
    </div>
  );
}

function DeductionTypes() {
  const qc = useQueryClient();
  const { data = [] } = useQuery<any[]>({ queryKey: ['deduction-types'], queryFn: () => listDeductionTypes().then(r => r.data) });
  const [f, setF] = useState({ code: '', name: '', category: 'voluntary', pre_tax: false });
  const [err, setErr] = useState('');
  const inv = () => qc.invalidateQueries({ queryKey: ['deduction-types'] });
  const add = useMutation({ mutationFn: () => createDeductionType(f), onSuccess: () => { setF({ code: '', name: '', category: 'voluntary', pre_tax: false }); inv(); }, onError: (e: any) => setErr(e?.response?.data?.error ?? 'Failed') });
  const toggle = useMutation({ mutationFn: (v: any) => setDeductionTypeActive(v.id, v.active), onSuccess: inv });
  return (
    <div className="space-y-4">
      {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
      <div className="card p-3 flex flex-wrap items-end gap-2">
        <div><label className="label">Code</label><input className="input py-1 text-sm font-mono w-28" value={f.code} onChange={e => setF({ ...f, code: e.target.value.toUpperCase() })} /></div>
        <div><label className="label">Name</label><input className="input py-1 text-sm w-52" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} /></div>
        <div><label className="label">Category</label>
          <select className="input py-1 text-sm" value={f.category} onChange={e => setF({ ...f, category: e.target.value })}>
            <option value="voluntary">Voluntary</option><option value="welfare">Welfare</option><option value="loan">Loan</option><option value="statutory">Statutory</option>
          </select>
        </div>
        <label className="flex items-center gap-1 text-xs"><input type="checkbox" checked={f.pre_tax} onChange={e => setF({ ...f, pre_tax: e.target.checked })} /> Pre-tax</label>
        <button className="btn-primary py-1" disabled={!f.code || !f.name || add.isPending} onClick={() => { setErr(''); add.mutate(); }}><Plus className="w-4 h-4" /> Add</button>
      </div>
      <TypeTable rows={data} extraCols={[['Category', 'category'], ['Pre-tax', 'pre_tax']]} onToggle={(id, active) => toggle.mutate({ id, active })} />
    </div>
  );
}

function TypeTable({ rows, extraCols, onToggle }: { rows: any[]; extraCols: [string, string][]; onToggle: (id: string, active: boolean) => void }) {
  return (
    <div className="card overflow-x-auto">
      <table className="w-full text-sm">
        <thead><tr className="border-b text-left text-xs text-gray-500 uppercase">
          <th className="px-3 py-2">Code</th><th className="px-3 py-2">Name</th><th className="px-3 py-2">Taxable</th>
          {extraCols.map(([h]) => <th key={h} className="px-3 py-2">{h}</th>)}
          <th className="px-3 py-2">Active</th>
        </tr></thead>
        <tbody>
          {rows.map(r => (
            <tr key={r.id} className="border-b">
              <td className="px-3 py-2 font-mono">{r.code}</td>
              <td className="px-3 py-2">{r.name}{r.is_system && <span className="ml-2 text-[10px] text-gray-400">system</span>}</td>
              <td className="px-3 py-2">{'taxable' in r ? (r.taxable ? 'Yes' : 'No') : '—'}</td>
              {extraCols.map(([, k]) => <td key={k} className="px-3 py-2">{typeof r[k] === 'boolean' ? (r[k] ? 'Yes' : 'No') : r[k]}</td>)}
              <td className="px-3 py-2"><button onClick={() => onToggle(r.id, !r.active)} className={r.active ? 'badge-success' : 'badge-gray'}>{r.active ? 'Active' : 'Inactive'}</button></td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Departments() {
  const qc = useQueryClient();
  const { data = [] } = useQuery<any[]>({ queryKey: ['departments'], queryFn: () => listDepartments().then(r => r.data) });
  const [f, setF] = useState({ code: '', name: '', cost_center: '' });
  const [err, setErr] = useState('');
  const add = useMutation({ mutationFn: () => createDepartment({ ...f, cost_center: f.cost_center || undefined }), onSuccess: () => { setF({ code: '', name: '', cost_center: '' }); qc.invalidateQueries({ queryKey: ['departments'] }); }, onError: (e: any) => setErr(e?.response?.data?.error ?? 'Failed') });
  return (
    <div className="space-y-4">
      {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded">{err}</div>}
      <div className="card p-3 flex flex-wrap items-end gap-2">
        <div><label className="label">Code</label><input className="input py-1 text-sm font-mono w-28" value={f.code} onChange={e => setF({ ...f, code: e.target.value.toUpperCase() })} /></div>
        <div><label className="label">Name</label><input className="input py-1 text-sm w-52" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} /></div>
        <div><label className="label">Cost centre</label><input className="input py-1 text-sm w-36" value={f.cost_center} onChange={e => setF({ ...f, cost_center: e.target.value })} /></div>
        <button className="btn-primary py-1" disabled={!f.code || !f.name || add.isPending} onClick={() => { setErr(''); add.mutate(); }}><Plus className="w-4 h-4" /> Add</button>
      </div>
      <div className="card overflow-x-auto">
        <table className="w-full text-sm">
          <thead><tr className="border-b text-left text-xs text-gray-500 uppercase"><th className="px-3 py-2">Code</th><th className="px-3 py-2">Name</th><th className="px-3 py-2">Cost centre</th></tr></thead>
          <tbody>{data.map(d => <tr key={d.id} className="border-b"><td className="px-3 py-2 font-mono">{d.code}</td><td className="px-3 py-2">{d.name}</td><td className="px-3 py-2 text-gray-500">{d.cost_center || '—'}</td></tr>)}</tbody>
        </table>
      </div>
    </div>
  );
}

function Statutory() {
  const qc = useQueryClient();
  const { data = [] } = useQuery<any[]>({ queryKey: ['statutory-config'], queryFn: () => listStatutoryConfig().then(r => r.data) });
  const [editing, setEditing] = useState<any | null>(null); // config row, or {} for new
  const latest = data[0];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-sm text-gray-500 max-w-2xl">Effective-dated statutory rules applied to each pay run (PAYE bands, NSSF, SHA, Housing Levy, reliefs). Rates change — add a <b>new version</b> with a future effective date and the run uses whichever is in force on its pay date. Past runs stay reproducible.</p>
        <button className="btn-primary shrink-0" onClick={() => setEditing({ __new: true, base: latest })}><Plus className="w-4 h-4" /> New rate version</button>
      </div>
      {editing && <StatutoryEditor row={editing} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); qc.invalidateQueries({ queryKey: ['statutory-config'] }); }} />}
      {data.map(c => {
        const cfg = c.config || {};
        return (
          <div key={c.id} className="card p-4">
            <div className="flex items-center justify-between mb-2">
              <h3 className="font-medium">{c.name}</h3>
              <div className="flex items-center gap-3">
                <span className="text-xs text-gray-500">Effective {c.effective_from}</span>
                <button className="text-indigo-600 text-xs hover:underline" onClick={() => setEditing(c)}>Edit</button>
              </div>
            </div>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-sm">
              <Fact label="Personal relief" v={cfg.personal_relief} />
              <Fact label="NSSF rate" v={cfg.nssf_rate} pct />
              <Fact label="NSSF cap" v={cfg.nssf_tier2_limit} />
              <Fact label="SHA rate" v={cfg.sha_rate} pct />
              <Fact label="Housing rate" v={cfg.housing_rate} pct />
              <Fact label="Insurance relief cap" v={cfg.insurance_relief_cap} />
              <Fact label="Disability exemption" v={cfg.disability_exemption} />
              <Fact label="PAYE bands" v={(cfg.paye_bands || []).length} />
            </div>
          </div>
        );
      })}
    </div>
  );
}

function StatutoryEditor({ row, onClose, onSaved }: { row: any; onClose: () => void; onSaved: () => void }) {
  const base = (row.__new ? row.base?.config : row.config) || {};
  const pct = (d: any) => d == null ? '' : String(+(Number(d) * 100).toFixed(4));
  const [f, setF] = useState<any>({
    effective_from: row.__new ? '' : row.effective_from,
    name: row.__new ? '' : (row.name ?? base.name ?? ''),
    personal_relief: base.personal_relief ?? 2400,
    insurance_relief_cap: base.insurance_relief_cap ?? 5000,
    disability_exemption: base.disability_exemption ?? 150000,
    nssf_tier1_limit: base.nssf_tier1_limit ?? 7000,
    nssf_tier2_limit: base.nssf_tier2_limit ?? 36000,
    nssf_rate: pct(base.nssf_rate ?? 0.06),
    sha_rate: pct(base.sha_rate ?? 0.0275),
    sha_minimum: base.sha_minimum ?? 0,
    housing_rate: pct(base.housing_rate ?? 0.015),
    nita_per_employee: base.nita_per_employee ?? 0,
  });
  const [bands, setBands] = useState<any[]>(
    (base.paye_bands ?? [
      { upper: 24000, rate: 0.10 }, { upper: 32333, rate: 0.25 }, { upper: 500000, rate: 0.30 },
      { upper: 800000, rate: 0.325 }, { upper: null, rate: 0.35 },
    ]).map((b: any) => ({ upper: b.upper == null ? '' : String(b.upper), rate: String(+(Number(b.rate) * 100).toFixed(4)) }))
  );
  const [err, setErr] = useState('');

  const save = useMutation({
    mutationFn: () => upsertStatutoryConfig({
      effective_from: f.effective_from,
      config: {
        name: f.name,
        paye_bands: bands.map(b => ({ upper: b.upper === '' ? null : Number(b.upper), rate: Number(b.rate) / 100 })),
        personal_relief: Number(f.personal_relief),
        insurance_relief_cap: Number(f.insurance_relief_cap),
        disability_exemption: Number(f.disability_exemption),
        nssf_tier1_limit: Number(f.nssf_tier1_limit),
        nssf_tier2_limit: Number(f.nssf_tier2_limit),
        nssf_rate: Number(f.nssf_rate) / 100,
        sha_rate: Number(f.sha_rate) / 100,
        sha_minimum: Number(f.sha_minimum),
        housing_rate: Number(f.housing_rate) / 100,
        nita_per_employee: Number(f.nita_per_employee),
      },
    }),
    onSuccess: onSaved,
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Save failed'),
  });

  const num = (k: string, label: string) => (
    <div><label className="label">{label}</label><input type="number" className="input py-1 text-sm" value={f[k]} onChange={e => setF({ ...f, [k]: e.target.value })} /></div>
  );

  return (
    <div className="card p-4 border-indigo-200 ring-1 ring-indigo-100">
      <div className="flex items-center justify-between mb-3">
        <h3 className="font-medium">{row.__new ? 'New statutory version' : `Edit — ${row.name}`}</h3>
        <button className="text-gray-400 hover:text-gray-700 text-sm" onClick={onClose}>Cancel</button>
      </div>
      {err && <div className="bg-red-50 text-red-700 text-sm px-3 py-2 rounded mb-3">{err}</div>}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div><label className="label">Effective from *</label><input type="date" className="input py-1 text-sm" value={f.effective_from} onChange={e => setF({ ...f, effective_from: e.target.value })} disabled={!row.__new} /></div>
        <div><label className="label">Name *</label><input className="input py-1 text-sm" value={f.name} onChange={e => setF({ ...f, name: e.target.value })} placeholder="e.g. Finance Act 2025" /></div>
        {num('personal_relief', 'Personal relief (KES)')}
        {num('insurance_relief_cap', 'Insurance relief cap')}
        {num('nssf_rate', 'NSSF rate (%)')}
        {num('nssf_tier2_limit', 'NSSF cap (KES)')}
        {num('sha_rate', 'SHA rate (%)')}
        {num('sha_minimum', 'SHA minimum (KES)')}
        {num('housing_rate', 'Housing rate (%)')}
        {num('disability_exemption', 'Disability exemption')}
        {num('nita_per_employee', 'NITA / employee')}
      </div>
      <div className="mt-4">
        <div className="flex items-center justify-between mb-1">
          <h4 className="text-sm font-medium text-gray-700">PAYE bands (monthly)</h4>
          <button className="text-indigo-600 text-xs hover:underline" onClick={() => setBands([...bands, { upper: '', rate: '' }])}>+ band</button>
        </div>
        <table className="w-full text-sm">
          <thead><tr className="text-left text-xs text-gray-500"><th className="py-1">Up to (KES, blank = no limit)</th><th className="py-1">Rate (%)</th><th></th></tr></thead>
          <tbody>
            {bands.map((b, i) => (
              <tr key={i}>
                <td className="py-1 pr-2"><input type="number" className="input py-1 text-sm" value={b.upper} placeholder="∞" onChange={e => setBands(bands.map((x, j) => j === i ? { ...x, upper: e.target.value } : x))} /></td>
                <td className="py-1 pr-2"><input type="number" className="input py-1 text-sm w-24" value={b.rate} onChange={e => setBands(bands.map((x, j) => j === i ? { ...x, rate: e.target.value } : x))} /></td>
                <td className="py-1"><button className="text-gray-400 hover:text-red-600 text-xs" onClick={() => setBands(bands.filter((_, j) => j !== i))}>remove</button></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="flex justify-end gap-2 mt-4">
        <button className="btn-secondary" onClick={onClose}>Cancel</button>
        <button className="btn-primary" disabled={!f.effective_from || !f.name || save.isPending} onClick={() => { setErr(''); save.mutate(); }}>{save.isPending ? 'Saving…' : 'Save version'}</button>
      </div>
    </div>
  );
}

function Fact({ label, v, pct }: { label: string; v: any; pct?: boolean }) {
  const val = v == null ? '—' : pct ? `${(Number(v) * 100).toFixed(2)}%` : String(v);
  return <div><p className="text-xs text-gray-400">{label}</p><p className="font-medium">{val}</p></div>;
}
