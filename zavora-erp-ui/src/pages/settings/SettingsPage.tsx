import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getSettings, updateSettings } from '../../api/client';
import type { ErpConfig } from '../../types';
import PageHeader from '../../components/shared/PageHeader';
import { Save } from 'lucide-react';

export default function SettingsPage() {
  const { data: config } = useQuery<ErpConfig>({ queryKey: ['settings'], queryFn: () => getSettings().then(r => r.data) });
  const queryClient = useQueryClient();
  const mutation = useMutation({ mutationFn: (data: any) => updateSettings(data), onSuccess: () => queryClient.invalidateQueries({ queryKey: ['settings'] }) });

  const [tab, setTab] = useState<'company' | 'tax' | 'payments' | 'sequences'>('company');

  const tabs = [
    { key: 'company', label: 'Company' },
    { key: 'tax', label: 'Tax & VAT' },
    { key: 'payments', label: 'Payment Methods' },
    { key: 'sequences', label: 'Document Numbers' },
  ];

  return (
    <div>
      <PageHeader title="Settings" subtitle="Configure your Zavora ERA instance" />

      <div className="flex gap-1 mb-6 bg-gray-100 p-1 rounded-lg w-fit">
        {tabs.map((t) => (
          <button key={t.key} onClick={() => setTab(t.key as any)} className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${tab === t.key ? 'bg-white shadow-sm text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}>{t.label}</button>
        ))}
      </div>

      <div className="card p-6">
        {tab === 'company' && (
          <div className="space-y-4 max-w-xl">
            <div><label className="label">Company Name</label><input className="input" defaultValue={config?.branding?.company_name} /></div>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Base Currency</label><input className="input" defaultValue={config?.base_currency || 'KES'} disabled /></div>
              <div><label className="label">Fiscal Year End</label><input className="input" defaultValue="December 31" disabled /></div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">KRA PIN</label><input className="input" defaultValue={config?.branding?.kra_pin} placeholder="P00XXXXXXX" /></div>
              <div><label className="label">VAT Number</label><input className="input" defaultValue={config?.branding?.vat_number} /></div>
            </div>
            <div><label className="label">Primary Color</label><input type="color" defaultValue={config?.branding?.primary_color || '#1a56db'} className="h-10 w-20 rounded border" /></div>
          </div>
        )}

        {tab === 'tax' && (
          <div className="space-y-4 max-w-xl">
            <div className="flex items-center gap-3"><input type="checkbox" defaultChecked={config?.tax_config?.vat_registered} /><label className="text-sm font-medium">VAT Registered</label></div>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Standard VAT Rate</label><input className="input" defaultValue="16%" disabled /></div>
              <div><label className="label">VAT Period</label><select className="input"><option>Monthly</option><option>Quarterly</option></select></div>
            </div>
            <div className="flex items-center gap-3"><input type="checkbox" defaultChecked={config?.tax_config?.wht_enabled} /><label className="text-sm font-medium">Withholding Tax Enabled</label></div>
            <div className="flex items-center gap-3"><input type="checkbox" defaultChecked={config?.tax_config?.paye_enabled} /><label className="text-sm font-medium">PAYE Payroll Enabled</label></div>
            <p className="text-xs text-gray-500">Kenya statutory rates: PAYE progressive bands, NSSF 6% (Tier I+II), SHA 2.75%, Housing Levy 1.5%</p>
          </div>
        )}

        {tab === 'payments' && (
          <div className="space-y-4 max-w-xl">
            <div className="p-4 bg-green-50 rounded-lg border border-green-200">
              <div className="flex items-center gap-3 mb-2"><input type="checkbox" defaultChecked={config?.payment_config?.mpesa_enabled} /><label className="text-sm font-medium text-green-900">M-Pesa (Daraja) Integration</label></div>
              <div className="grid grid-cols-2 gap-3 ml-6">
                <div><label className="label text-xs">Paybill Number</label><input className="input text-sm" defaultValue={config?.payment_config?.mpesa_paybill} placeholder="174379" /></div>
                <div><label className="label text-xs">Till Number</label><input className="input text-sm" placeholder="Optional" /></div>
              </div>
            </div>
            <div className="p-4 bg-purple-50 rounded-lg border border-purple-200">
              <div className="flex items-center gap-3"><input type="checkbox" defaultChecked={config?.payment_config?.flutterwave_enabled} /><label className="text-sm font-medium text-purple-900">Flutterwave (Card Payments)</label></div>
            </div>
            <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
              <div className="flex items-center gap-3"><input type="checkbox" defaultChecked={config?.payment_config?.bank_transfer_enabled} /><label className="text-sm font-medium">Bank Transfer</label></div>
            </div>
          </div>
        )}

        {tab === 'sequences' && (
          <div className="space-y-4 max-w-xl">
            <p className="text-sm text-gray-500 mb-4">Configure document numbering prefixes and starting numbers.</p>
            {[
              { label: 'Invoice', prefix: config?.sequences?.invoice_prefix || 'INV', next: config?.sequences?.invoice_next || 1 },
              { label: 'Estimate', prefix: config?.sequences?.estimate_prefix || 'EST', next: config?.sequences?.estimate_next || 1 },
              { label: 'Bill', prefix: config?.sequences?.bill_prefix || 'BILL', next: config?.sequences?.bill_next || 1 },
            ].map((seq) => (
              <div key={seq.label} className="grid grid-cols-3 gap-3 items-end">
                <div><label className="label">{seq.label} Prefix</label><input className="input font-mono" defaultValue={seq.prefix} /></div>
                <div><label className="label">Next Number</label><input type="number" className="input" defaultValue={seq.next} /></div>
                <div className="text-sm text-gray-500 pb-2">e.g. {seq.prefix}-2026-{String(seq.next).padStart(4, '0')}</div>
              </div>
            ))}
            <div className="flex items-center gap-2 mt-4"><input type="checkbox" defaultChecked={config?.sequences?.year_reset} /><label className="text-sm">Reset numbering on new fiscal year</label></div>
          </div>
        )}

        <div className="mt-6 pt-4 border-t flex justify-end">
          <button className="btn-primary"><Save className="w-4 h-4" /> Save Changes</button>
        </div>
      </div>
    </div>
  );
}
