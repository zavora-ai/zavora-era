import { useState, useEffect } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getSettings, updateSettings } from '../../api/client';
import type { ErpConfig, DocumentSequences } from '../../types';
import PageHeader from '../../components/shared/PageHeader';
import PostingAccountsTab from './PostingAccountsTab';
import PostingGroupsTab from './PostingGroupsTab';
import NotificationPrefsTab from './NotificationPrefsTab';
import NotificationProvidersTab from './NotificationProvidersTab';
import SubscriptionTab from './SubscriptionTab';
import { SkeletonLines } from '../../components/shared/Skeleton';
import ErrorRetry from '../../components/shared/ErrorRetry';
import { Save, CheckCircle, AlertCircle } from 'lucide-react';

export default function SettingsPage() {
  const queryClient = useQueryClient();
  const { data: config, isLoading, isError, refetch } = useQuery<ErpConfig>({ queryKey: ['settings'], queryFn: () => getSettings().then(r => r.data) });

  const [searchParams] = useSearchParams();
  const TAB_KEYS = ['company', 'tax', 'payments', 'sequences', 'posting', 'posting-groups', 'notifications', 'providers', 'subscription'] as const;
  const urlTab = searchParams.get('tab');
  const [tab, setTab] = useState<'company' | 'tax' | 'payments' | 'sequences' | 'posting' | 'posting-groups' | 'notifications' | 'providers' | 'subscription'>(
    (TAB_KEYS as readonly string[]).includes(urlTab ?? '') ? (urlTab as any) : 'company'
  );
  const [toast, setToast] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  // Controlled form state for each tab
  const [company, setCompany] = useState({ company_name: '', registration_number: '', kra_pin: '', vat_number: '', address: '', phone: '', primary_color: '#1a56db', base_currency: 'KES', fiscal_year_end: '12-31' });
  const [tax, setTax] = useState({ vat_registered: false, wht_enabled: false, paye_enabled: false });
  const [payments, setPayments] = useState({ mpesa_enabled: false, mpesa_paybill: '', paystack_enabled: false, paystack_public_key: '', bank_transfer_enabled: false });
  const [seq, setSeq] = useState<DocumentSequences | null>(null);

  // Initialize form state from fetched config
  useEffect(() => {
    if (!config) return;
    setCompany({
      company_name: config.branding?.company_name ?? '',
      registration_number: config.branding?.registration_number ?? '',
      kra_pin: config.branding?.kra_pin ?? '',
      vat_number: config.branding?.vat_number ?? '',
      address: config.branding?.address ?? '',
      phone: config.branding?.phone ?? '',
      primary_color: config.branding?.primary_color ?? '#1a56db',
      base_currency: config.base_currency ?? 'KES',
      fiscal_year_end: config.fiscal_year_end ? `${config.fiscal_year_end.month}-${config.fiscal_year_end.day}` : '12-31',
    });
    setTax({
      vat_registered: config.tax_config?.vat_registered ?? false,
      wht_enabled: config.tax_config?.wht_enabled ?? false,
      paye_enabled: config.tax_config?.paye_enabled ?? false,
    });
    setPayments({
      mpesa_enabled: config.payment_config?.mpesa_enabled ?? false,
      mpesa_paybill: config.payment_config?.mpesa_paybill ?? '',
      paystack_enabled: config.payment_config?.paystack_enabled ?? false,
      paystack_public_key: config.payment_config?.paystack_public_key ?? '',
      bank_transfer_enabled: config.payment_config?.bank_transfer_enabled ?? false,
    });
    if (config.sequences) setSeq(config.sequences);
  }, [config]);

  const mutation = useMutation({
    mutationFn: (data: any) => updateSettings(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['settings'] });
      setToast({ type: 'success', message: 'Settings saved successfully' });
      setTimeout(() => setToast(null), 4000);
    },
    onError: (err: any) => {
      setToast({ type: 'error', message: err?.response?.data?.error || 'Failed to save settings' });
      setTimeout(() => setToast(null), 6000);
    },
  });

  const handleSave = () => {
    const patch: any = {};
    if (tab === 'company') {
      const [m, d] = company.fiscal_year_end.split('-').map(Number);
      patch.branding = {
        ...config?.branding,
        company_name: company.company_name,
        registration_number: company.registration_number || undefined,
        kra_pin: company.kra_pin || undefined,
        vat_number: company.vat_number || undefined,
        address: company.address || undefined,
        phone: company.phone || undefined,
        primary_color: company.primary_color,
      };
      patch.base_currency = company.base_currency;
      patch.fiscal_year_end = { month: m, day: d };
    } else if (tab === 'tax') {
      patch.tax_config = tax;
    } else if (tab === 'payments') {
      patch.payment_config = payments;
    } else if (tab === 'sequences') {
      if (!seq) return;
      patch.sequences = seq;
    }
    mutation.mutate(patch);
  };

  const tabs = [
    { key: 'company', label: 'Company' },
    { key: 'tax', label: 'Tax & VAT' },
    { key: 'payments', label: 'Payment Methods' },
    { key: 'sequences', label: 'Document Numbers' },
    { key: 'posting', label: 'Posting Accounts' },
    { key: 'posting-groups', label: 'Posting Groups' },
    { key: 'notifications', label: 'Notifications' },
    { key: 'providers', label: 'Providers' },
    { key: 'subscription', label: 'Subscription' },
  ];

  return (
    <div>
      <PageHeader title="Settings" subtitle="Configure your Zavora ERP instance" />

      {toast && (
        <div className={`mb-4 flex items-center gap-2 p-3 rounded-lg text-sm ${toast.type === 'success' ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'}`}>
          {toast.type === 'success' ? <CheckCircle className="w-4 h-4 shrink-0" /> : <AlertCircle className="w-4 h-4 shrink-0" />}
          <span>{toast.message}</span>
        </div>
      )}

      <div className="flex gap-1 mb-6 bg-gray-100 p-1 rounded-lg w-fit">
        {tabs.map((t) => (
          <button key={t.key} onClick={() => setTab(t.key as any)} className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${tab === t.key ? 'bg-white shadow-sm text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}>{t.label}</button>
        ))}
      </div>

      {isLoading ? (
        <div className="card p-6"><SkeletonLines lines={6} /></div>
      ) : isError ? (
        <ErrorRetry message="Couldn't load your settings." onRetry={() => refetch()} />
      ) : (
      <div className="card p-6">
        {tab === 'company' && (
          <div className="space-y-4 max-w-xl">
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Company Name</label><input className="input" value={company.company_name} onChange={(e) => setCompany({ ...company, company_name: e.target.value })} /></div>
              <div><label className="label">Registration Number</label><input className="input" value={company.registration_number} onChange={(e) => setCompany({ ...company, registration_number: e.target.value })} placeholder="e.g. PVT-XXXXXXX" /></div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">Base Currency</label>
                <select className="input" value={company.base_currency} onChange={(e) => setCompany({ ...company, base_currency: e.target.value })}>
                  <option value="KES">KES — Kenyan Shilling</option>
                  <option value="USD">USD — US Dollar</option>
                  <option value="EUR">EUR — Euro</option>
                  <option value="GBP">GBP — Pound Sterling</option>
                  <option value="TZS">TZS — Tanzanian Shilling</option>
                  <option value="UGX">UGX — Ugandan Shilling</option>
                </select>
              </div>
              <div>
                <label className="label">Fiscal Year End</label>
                <select className="input" value={company.fiscal_year_end} onChange={(e) => setCompany({ ...company, fiscal_year_end: e.target.value })}>
                  <option value="12-31">December 31</option>
                  <option value="6-30">June 30</option>
                  <option value="3-31">March 31</option>
                  <option value="9-30">September 30</option>
                </select>
              </div>
            </div>
            <p className="text-xs text-amber-600">Set the base currency and year-end before posting transactions — changing them later does not restate existing entries.</p>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">KRA PIN</label><input className="input" value={company.kra_pin} onChange={(e) => setCompany({ ...company, kra_pin: e.target.value })} placeholder="P00XXXXXXX" /></div>
              <div><label className="label">VAT Number</label><input className="input" value={company.vat_number} onChange={(e) => setCompany({ ...company, vat_number: e.target.value })} placeholder="Only if VAT-registered" /></div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Registered Address</label><input className="input" value={company.address} onChange={(e) => setCompany({ ...company, address: e.target.value })} placeholder="P.O. Box / street, town" /></div>
              <div><label className="label">Phone</label><input className="input" value={company.phone} onChange={(e) => setCompany({ ...company, phone: e.target.value })} placeholder="+254..." /></div>
            </div>
            <div><label className="label">Primary Color</label><input type="color" value={company.primary_color} onChange={(e) => setCompany({ ...company, primary_color: e.target.value })} className="h-10 w-20 rounded border" /></div>
          </div>
        )}

        {tab === 'tax' && (
          <div className="space-y-4 max-w-xl">
            <div className="flex items-center gap-3"><input type="checkbox" checked={tax.vat_registered} onChange={(e) => setTax({ ...tax, vat_registered: e.target.checked })} /><label className="text-sm font-medium">VAT Registered</label></div>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Standard VAT Rate</label><input className="input" defaultValue="16%" disabled /></div>
              <div><label className="label">VAT Period</label><select className="input"><option>Monthly</option><option>Quarterly</option></select></div>
            </div>
            <div className="flex items-center gap-3"><input type="checkbox" checked={tax.wht_enabled} onChange={(e) => setTax({ ...tax, wht_enabled: e.target.checked })} /><label className="text-sm font-medium">Withholding Tax Enabled</label></div>
            <div className="flex items-center gap-3"><input type="checkbox" checked={tax.paye_enabled} onChange={(e) => setTax({ ...tax, paye_enabled: e.target.checked })} /><label className="text-sm font-medium">PAYE Payroll Enabled</label></div>
            <p className="text-xs text-gray-500">Kenya statutory rates: PAYE progressive bands, NSSF 6% (Tier I+II), SHA 2.75%, Housing Levy 1.5%</p>
          </div>
        )}

        {tab === 'payments' && (
          <div className="space-y-4 max-w-xl">
            <div className="p-4 bg-green-50 rounded-lg border border-green-200">
              <div className="flex items-center gap-3 mb-2"><input type="checkbox" checked={payments.mpesa_enabled} onChange={(e) => setPayments({ ...payments, mpesa_enabled: e.target.checked })} /><label className="text-sm font-medium text-green-900">M-Pesa (Daraja) Integration</label></div>
              <div className="grid grid-cols-2 gap-3 ml-6">
                <div><label className="label text-xs">Paybill Number</label><input className="input text-sm" value={payments.mpesa_paybill} onChange={(e) => setPayments({ ...payments, mpesa_paybill: e.target.value })} placeholder="174379" /></div>
                <div><label className="label text-xs">Till Number</label><input className="input text-sm" placeholder="Optional" /></div>
              </div>
            </div>
            <div className="p-4 bg-purple-50 rounded-lg border border-purple-200">
              <div className="flex items-center gap-3 mb-2"><input type="checkbox" checked={payments.paystack_enabled} onChange={(e) => setPayments({ ...payments, paystack_enabled: e.target.checked })} /><label className="text-sm font-medium text-purple-900">Paystack (Card Payments)</label></div>
              {payments.paystack_enabled && (
                <div className="pl-6">
                  <label className="label text-xs">Public Key</label>
                  <input className="input text-sm font-mono" value={payments.paystack_public_key} onChange={(e) => setPayments({ ...payments, paystack_public_key: e.target.value })} placeholder="pk_live_…" />
                  <p className="text-xs text-gray-500 mt-1">The secret key is set on the server (PAYSTACK_SECRET_KEY env), never here.</p>
                </div>
              )}
            </div>
            <div className="p-4 bg-gray-50 rounded-lg border border-gray-200">
              <div className="flex items-center gap-3"><input type="checkbox" checked={payments.bank_transfer_enabled} onChange={(e) => setPayments({ ...payments, bank_transfer_enabled: e.target.checked })} /><label className="text-sm font-medium">Bank Transfer</label></div>
            </div>
          </div>
        )}

        {tab === 'sequences' && seq && (
          <div className="space-y-4 max-w-xl">
            <p className="text-sm text-gray-500 mb-4">Configure document numbering prefixes and starting numbers.</p>
            {([
              { label: 'Invoice', pk: 'invoice_prefix', nk: 'invoice_next' },
              { label: 'Estimate', pk: 'estimate_prefix', nk: 'estimate_next' },
              { label: 'Credit Note', pk: 'credit_note_prefix', nk: 'credit_note_next' },
              { label: 'Bill', pk: 'bill_prefix', nk: 'bill_next' },
              { label: 'Journal', pk: 'journal_prefix', nk: 'journal_next' },
              { label: 'Payment', pk: 'payment_prefix', nk: 'payment_next' },
            ] as const).map((row) => {
              const prefix = (seq as any)[row.pk] as string;
              const next = (seq as any)[row.nk] as number;
              return (
                <div key={row.label} className="grid grid-cols-3 gap-3 items-end">
                  <div>
                    <label className="label">{row.label} Prefix</label>
                    <input className="input font-mono" value={prefix} onChange={(e) => setSeq({ ...seq, [row.pk]: e.target.value })} />
                  </div>
                  <div>
                    <label className="label">Next Number</label>
                    <input type="number" min={1} className="input" value={next} onChange={(e) => setSeq({ ...seq, [row.nk]: Math.max(1, parseInt(e.target.value) || 1) })} />
                  </div>
                  <div className="text-sm text-gray-500 pb-2">e.g. {prefix}-2026-{String(next).padStart(4, '0')}</div>
                </div>
              );
            })}
            <div className="flex items-center gap-2 mt-4">
              <input type="checkbox" checked={seq.year_reset} onChange={(e) => setSeq({ ...seq, year_reset: e.target.checked })} />
              <label className="text-sm">Reset numbering on new fiscal year</label>
            </div>
          </div>
        )}

        {tab === 'posting' && <PostingAccountsTab />}
        {tab === 'posting-groups' && <PostingGroupsTab />}
        {tab === 'notifications' && <NotificationPrefsTab />}
        {tab === 'providers' && <NotificationProvidersTab />}
        {tab === 'subscription' && <SubscriptionTab />}

        {tab !== 'posting' && tab !== 'posting-groups' && tab !== 'notifications' && tab !== 'providers' && (
          <div className="mt-6 pt-4 border-t flex justify-end">
            <button onClick={handleSave} disabled={mutation.isPending} className="btn-primary">
              {mutation.isPending ? (
                <><div className="animate-spin w-4 h-4 border-2 border-white border-t-transparent rounded-full" /> Saving...</>
              ) : (
                <><Save className="w-4 h-4" /> Save Changes</>
              )}
            </button>
          </div>
        )}
      </div>
      )}
    </div>
  );
}
