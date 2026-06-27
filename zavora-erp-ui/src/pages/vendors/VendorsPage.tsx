import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getVendors, createVendor } from '../../api/client';
import type { Vendor } from '../../types';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, AlertTriangle } from 'lucide-react';

export default function VendorsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const navigate = useNavigate();
  const { data: vendors = [], isLoading } = useQuery<Vendor[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then(r => Array.isArray(r.data) ? r.data : []) });

  const columns: Column<Vendor>[] = [
    { key: 'name', header: 'Vendor', render: (r) => <span className="font-medium text-gray-900">{r.name}</span> },
    { key: 'kra_pin', header: 'KRA PIN', render: (r) => r.kra_pin ? <span className="font-mono text-xs">{r.kra_pin}</span> : <span className="text-gray-400">—</span> },
    { key: 'wht_category', header: 'WHT Category', render: (r) => r.wht_category ? <span className="badge-warning">{r.wht_category}</span> : <span className="text-gray-400">None</span> },
    { key: 'resident', header: 'Resident', render: (r) => r.resident ? <span className="badge-success">Yes</span> : <span className="badge-danger">Non-resident</span> },
    { key: 'payment_terms', header: 'Terms', render: (r) => r.payment_terms?.replace('Net', 'Net ') },
    { key: 'is_active', header: 'Status', render: (r) => <span className={r.is_active ? 'badge-success' : 'badge-gray'}>{r.is_active ? 'Active' : 'Inactive'}</span> },
  ];

  return (
    <div>
      <PageHeader
        title="Vendors"
        subtitle={`${vendors.length} supplier${vendors.length !== 1 ? 's' : ''} — withholding tax is auto-calculated on bills`}
        actions={<button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> Add a Vendor</button>}
      />
      <DataTable columns={columns} data={vendors} loading={isLoading} onRowClick={(r) => navigate(`/vendors/${r.id}`)} emptyMessage="No vendors yet. Add suppliers to start tracking bills and payments." />
      {showCreate && <CreateVendorModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateVendorModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'details' | 'tax' | 'bank'>('details');

  const [form, setForm] = useState({
    name: '',
    contact_person: '',
    email: '',
    phone: '',
    // Address
    address_1: '',
    address_2: '',
    city: '',
    county: '',
    postal: '',
    country: 'Kenya',
    // Tax & WHT
    kra_pin: '',
    vat_number: '',
    wht_category: '',
    resident: true,
    // Payment
    currency: 'KES',
    payment_terms: 'Net30',
    default_expense_account: '7900',
    // Bank
    bank_name: '',
    bank_branch: '',
    bank_account_name: '',
    bank_account_number: '',
    bank_swift: '',
    // Notes
    notes: '',
  });

  const mutation = useMutation({
    mutationFn: (data: any) => createVendor(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['vendors'] }); onClose(); },
  });

  const whtRates: Record<string, { resident: string; nonResident: string }> = {
    Consultancy: { resident: '5%', nonResident: '20%' },
    ManagementFees: { resident: '5%', nonResident: '20%' },
    Rent: { resident: '10%', nonResident: '30%' },
    Contractual: { resident: '3%', nonResident: '20%' },
    Royalties: { resident: '5%', nonResident: '20%' },
    Interest: { resident: '15%', nonResident: '15%' },
    Dividends: { resident: '5%', nonResident: '15%' },
    Transport: { resident: '2%', nonResident: '20%' },
  };

  const selectedRate = form.wht_category ? whtRates[form.wht_category] : null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      name: form.name,
      kra_pin: form.kra_pin || undefined,
      vat_number: form.vat_number || undefined,
      wht_category: form.wht_category || undefined,
      resident: form.resident,
      email: form.email ? [{ email: form.email, is_primary: true }] : [],
      phone: form.phone ? [{ number: form.phone, is_primary: true, whatsapp_enabled: false }] : [],
      address: form.address_1 ? {
        line1: form.address_1, line2: form.address_2 || undefined,
        city: form.city, county: form.county || undefined,
        postal_code: form.postal || undefined, country: form.country,
      } : undefined,
      currency: form.currency,
      payment_terms: form.payment_terms,
      default_expense_account: form.default_expense_account || undefined,
      bank_details: form.bank_account_number ? {
        bank_name: form.bank_name, branch: form.bank_branch || undefined,
        account_name: form.bank_account_name, account_number: form.bank_account_number,
        swift_code: form.bank_swift || undefined,
      } : undefined,
      notes: form.notes || undefined,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Add a Vendor" size="lg">
      <form onSubmit={handleSubmit}>
        <div className="flex gap-1 mb-6 border-b">
          {(['details', 'tax', 'bank'] as const).map((t) => (
            <button key={t} type="button" onClick={() => setTab(t)} className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px ${tab === t ? 'border-blue-600 text-blue-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}>
              {t === 'details' ? 'Vendor Details' : t === 'tax' ? 'Tax & WHT' : 'Bank Details'}
            </button>
          ))}
        </div>

        {tab === 'details' && (
          <div className="space-y-4">
            <div>
              <label className="label">Vendor / Company Name *</label>
              <input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="e.g. Kenya Power & Lighting" required />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Contact Person</label><input className="input" value={form.contact_person} onChange={(e) => setForm({ ...form, contact_person: e.target.value })} placeholder="Optional" /></div>
              <div><label className="label">Email</label><input type="email" className="input" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} placeholder="accounts@vendor.co.ke" /></div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Phone</label><input className="input" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} placeholder="+254..." /></div>
              <div>
                <label className="label">Payment Terms</label>
                <select className="input" value={form.payment_terms} onChange={(e) => setForm({ ...form, payment_terms: e.target.value })}>
                  <option value="DueOnReceipt">Due on Receipt</option><option value="Net7">Net 7</option><option value="Net14">Net 14</option><option value="Net30">Net 30</option><option value="Net45">Net 45</option><option value="Net60">Net 60</option>
                </select>
              </div>
            </div>
            <hr />
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Address</label><input className="input" value={form.address_1} onChange={(e) => setForm({ ...form, address_1: e.target.value })} placeholder="Street or P.O. Box" /></div>
              <div><label className="label">City</label><input className="input" value={form.city} onChange={(e) => setForm({ ...form, city: e.target.value })} placeholder="Nairobi" /></div>
            </div>
            <div>
              <label className="label">Notes</label>
              <textarea className="input" rows={2} value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} placeholder="Internal notes..." />
            </div>
          </div>
        )}

        {tab === 'tax' && (
          <div className="space-y-4">
            <div className="bg-amber-50 border border-amber-200 rounded-lg p-4 text-sm">
              <div className="flex items-start gap-2">
                <AlertTriangle className="w-5 h-5 text-amber-600 shrink-0 mt-0.5" />
                <div>
                  <p className="font-medium text-amber-900">Withholding Tax (WHT)</p>
                  <p className="text-amber-700 mt-1">When a WHT category is set, the system automatically deducts and accounts for withholding tax when you post bills for this vendor. No manual calculation needed.</p>
                </div>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">KRA PIN *</label>
                <input className="input font-mono" value={form.kra_pin} onChange={(e) => setForm({ ...form, kra_pin: e.target.value.toUpperCase() })} placeholder="P00XXXXXXX" maxLength={11} />
                <p className="text-xs text-gray-400 mt-1">Required for WHT certificate generation</p>
              </div>
              <div>
                <label className="label">VAT Number</label>
                <input className="input font-mono" value={form.vat_number} onChange={(e) => setForm({ ...form, vat_number: e.target.value })} placeholder="If VAT registered" />
              </div>
            </div>

            <div>
              <label className="label">WHT Category</label>
              <select className="input" value={form.wht_category} onChange={(e) => setForm({ ...form, wht_category: e.target.value })}>
                <option value="">No withholding tax</option>
                <option value="Consultancy">Consultancy / Professional fees</option>
                <option value="ManagementFees">Management fees</option>
                <option value="Rent">Rent (land & buildings)</option>
                <option value="Contractual">Contractual (construction)</option>
                <option value="Royalties">Royalties</option>
                <option value="Interest">Interest (non-bank)</option>
                <option value="Dividends">Dividends</option>
                <option value="Transport">Transport</option>
              </select>
            </div>

            {selectedRate && (
              <div className="bg-blue-50 border border-blue-200 rounded-lg p-3">
                <p className="text-sm font-medium text-blue-900">Applicable rate:</p>
                <p className="text-sm text-blue-700 mt-1">
                  Resident: <strong>{selectedRate.resident}</strong> · Non-resident: <strong>{selectedRate.nonResident}</strong>
                </p>
              </div>
            )}

            <div className="flex items-center gap-3">
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="radio" name="resident" checked={form.resident} onChange={() => setForm({ ...form, resident: true })} />
                <span className="text-sm">Kenyan Resident</span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="radio" name="resident" checked={!form.resident} onChange={() => setForm({ ...form, resident: false })} />
                <span className="text-sm">Non-Resident</span>
              </label>
            </div>

            <div>
              <label className="label">Default Expense Account</label>
              <select className="input" value={form.default_expense_account} onChange={(e) => setForm({ ...form, default_expense_account: e.target.value })}>
                <option value="7100">7100 — Rent Expense</option>
                <option value="7200">7200 — Utilities</option>
                <option value="7300">7300 — Office Supplies</option>
                <option value="7400">7400 — Insurance</option>
                <option value="7500">7500 — Professional Fees</option>
                <option value="7700">7700 — Advertising & Marketing</option>
                <option value="7800">7800 — Travel & Transport</option>
                <option value="7900">7900 — Miscellaneous Expenses</option>
              </select>
              <p className="text-xs text-gray-400 mt-1">Pre-selected when creating bills for this vendor</p>
            </div>
          </div>
        )}

        {tab === 'bank' && (
          <div className="space-y-4">
            <p className="text-sm text-gray-500">Bank details for payment runs and EFT transfers.</p>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Bank Name</label><input className="input" value={form.bank_name} onChange={(e) => setForm({ ...form, bank_name: e.target.value })} placeholder="e.g. KCB Bank" /></div>
              <div><label className="label">Branch</label><input className="input" value={form.bank_branch} onChange={(e) => setForm({ ...form, bank_branch: e.target.value })} placeholder="e.g. Kenyatta Avenue" /></div>
            </div>
            <div>
              <label className="label">Account Name</label>
              <input className="input" value={form.bank_account_name} onChange={(e) => setForm({ ...form, bank_account_name: e.target.value })} placeholder="Name on bank account" />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div><label className="label">Account Number</label><input className="input font-mono" value={form.bank_account_number} onChange={(e) => setForm({ ...form, bank_account_number: e.target.value })} placeholder="Account number" /></div>
              <div><label className="label">SWIFT / BIC Code</label><input className="input font-mono" value={form.bank_swift} onChange={(e) => setForm({ ...form, bank_swift: e.target.value })} placeholder="Optional — for international" /></div>
            </div>
          </div>
        )}

        <div className="flex justify-end gap-3 pt-6 mt-6 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.name}>
            {mutation.isPending ? 'Saving...' : 'Save Vendor'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
