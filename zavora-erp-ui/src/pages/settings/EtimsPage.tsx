import { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getEtimsConfig, saveEtimsConfig, initializeEtims } from '../../api/client';
import { Receipt, CheckCircle2, AlertTriangle, RefreshCw } from 'lucide-react';

interface EtimsDevice {
  enabled: boolean; environment: string; pin?: string | null; bhf_id: string;
  dvc_srl_no?: string | null; sdc_id?: string | null; mrc_no?: string | null;
  initialized: boolean; initialized_at?: string | null; last_invc_no: number; last_error?: string | null;
}

export default function EtimsPage() {
  const qc = useQueryClient();
  const { data: dev, isLoading } = useQuery<EtimsDevice>({ queryKey: ['etims-config'], queryFn: () => getEtimsConfig().then((r) => r.data) });

  const [form, setForm] = useState({ enabled: false, environment: 'sandbox', pin: '', bhf_id: '00', dvc_srl_no: '' });
  useEffect(() => {
    if (dev) setForm({ enabled: dev.enabled, environment: dev.environment || 'sandbox', pin: dev.pin || '', bhf_id: dev.bhf_id || '00', dvc_srl_no: dev.dvc_srl_no || '' });
  }, [dev]);

  const save = useMutation({
    mutationFn: () => saveEtimsConfig(form),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['etims-config'] }),
    onError: (e: any) => window.alert(e?.response?.data?.error || 'Could not save.'),
  });
  const init = useMutation({
    mutationFn: () => initializeEtims(),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['etims-config'] }); window.alert('Device initialised with KRA.'); },
    onError: (e: any) => window.alert(e?.response?.data?.error || 'Initialisation failed.'),
  });

  if (isLoading) return <p className="text-sm text-gray-500 py-12 text-center">Loading…</p>;
  const set = (k: string, v: any) => setForm((f) => ({ ...f, [k]: v }));

  return (
    <div className="max-w-3xl mx-auto space-y-6">
      <div>
        <h1 className="text-xl font-bold flex items-center gap-2"><Receipt className="w-5 h-5 text-indigo-600" /> KRA eTIMS</h1>
        <p className="text-sm text-gray-500 mt-1">Transmit tax invoices to KRA in real time via the eTIMS OSCU/VSCU. Posted invoices and POS sales are sent automatically once the device is enabled and initialised.</p>
      </div>

      {/* Status */}
      <div className="card p-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            {dev?.initialized
              ? <CheckCircle2 className="w-5 h-5 text-emerald-600" />
              : <AlertTriangle className="w-5 h-5 text-amber-500" />}
            <span className="font-semibold">{dev?.initialized ? 'Device initialised' : 'Not initialised'}</span>
            {dev?.enabled && <span className="text-xs px-2 py-0.5 rounded-full bg-emerald-50 text-emerald-700">Enabled</span>}
            <span className="text-xs px-2 py-0.5 rounded-full bg-slate-100 text-slate-600 uppercase">{dev?.environment}</span>
          </div>
          <button onClick={() => init.mutate()} disabled={init.isPending || !form.enabled} className="btn-secondary text-sm">
            <RefreshCw className={`w-4 h-4 ${init.isPending ? 'animate-spin' : ''}`} /> {dev?.initialized ? 'Re-initialise' : 'Initialise device'}
          </button>
        </div>
        {dev?.initialized && (
          <div className="grid grid-cols-2 sm:grid-cols-3 gap-4 mt-4 text-sm">
            <div><div className="text-gray-500 text-xs">SCU ID</div><div className="font-medium">{dev.sdc_id || '—'}</div></div>
            <div><div className="text-gray-500 text-xs">MRC No</div><div className="font-medium">{dev.mrc_no || '—'}</div></div>
            <div><div className="text-gray-500 text-xs">Last invoice #</div><div className="font-medium">{dev.last_invc_no}</div></div>
          </div>
        )}
        {dev?.last_error && <div className="mt-3 text-xs text-red-600 bg-red-50 border border-red-100 rounded p-2 break-words">Last error: {dev.last_error}</div>}
      </div>

      {/* Config */}
      <div className="card p-5 space-y-4">
        <h2 className="font-semibold">Device credentials</h2>
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={form.enabled} onChange={(e) => set('enabled', e.target.checked)} />
          Enable eTIMS transmission
        </label>
        <div className="grid sm:grid-cols-2 gap-4">
          <div>
            <label className="label">Environment</label>
            <select className="input" value={form.environment} onChange={(e) => set('environment', e.target.value)}>
              <option value="sandbox">Sandbox (testing)</option>
              <option value="production">Production (live KRA)</option>
            </select>
          </div>
          <div>
            <label className="label">Branch ID</label>
            <input className="input" value={form.bhf_id} onChange={(e) => set('bhf_id', e.target.value)} placeholder="00" />
          </div>
          <div>
            <label className="label">KRA PIN</label>
            <input className="input" value={form.pin} onChange={(e) => set('pin', e.target.value.toUpperCase())} placeholder="P051234567X" />
          </div>
          <div>
            <label className="label">Device serial number</label>
            <input className="input" value={form.dvc_srl_no} onChange={(e) => set('dvc_srl_no', e.target.value)} placeholder="From KRA eTIMS onboarding" />
          </div>
        </div>
        <div className="flex justify-end">
          <button onClick={() => save.mutate()} disabled={save.isPending} className="btn-primary">{save.isPending ? 'Saving…' : 'Save'}</button>
        </div>
        <p className="text-xs text-gray-400">The device serial number and PIN come from your KRA eTIMS onboarding. Save the credentials, then initialise the device to register it with KRA.</p>
      </div>
    </div>
  );
}
