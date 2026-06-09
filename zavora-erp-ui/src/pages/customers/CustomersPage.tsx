import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getCustomers, createCustomer } from '../../api/client';
import type { Customer } from '../../types';
import { formatDate, formatCurrency } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, Mail, Phone, MapPin, FileText } from 'lucide-react';

export default function CustomersPage() {
  const [showCreate, setShowCreate] = useState(false);
  const queryClient = useQueryClient();

  const { data: customers = [], isLoading } = useQuery<Customer[]>({
    queryKey: ['customers'],
    queryFn: () => getCustomers().then(r => r.data),
  });

  const columns: Column<Customer>[] = [
    {
      key: 'name', header: 'Customer',
      render: (r) => (
        <div>
          <p className="font-medium text-gray-900">{r.name}</p>
          {r.email?.[0] && <p className="text-xs text-gray-500">{r.email[0].email}</p>}
        </div>
      )
    },
    { key: 'phone', header: 'Phone', render: (r) => r.phone?.[0]?.number || '—' },
    { key: 'kra_pin', header: 'KRA PIN', render: (r) => r.kra_pin ? <span className="font-mono text-xs">{r.kra_pin}</span> : '—' },
    { key: 'currency', header: 'Currency' },
    { key: 'payment_terms', header: 'Terms', render: (r) => r.payment_terms?.replace('Net', 'Net ') || '—' },
    { key: 'credit_limit', header: 'Credit Limit', render: (r) => r.credit_limit ? formatCurrency(r.credit_limit) : '—', className: 'text-right' },
    { key: 'is_active', header: 'Status', render: (r) => <span className={r.is_active ? 'badge-success' : 'badge-gray'}>{r.is_active ? 'Active' : 'Inactive'}</span> },
    { key: 'created_at', header: 'Added', render: (r) => formatDate(r.created_at) },
  ];

  return (
    <div>
      <PageHeader
        title="Customers"
        subtitle={`${customers.length} customer${customers.length !== 1 ? 's' : ''}`}
        actions={
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> Add a Customer
          </button>
        }
      />
      <DataTable columns={columns} data={customers} loading={isLoading} emptyMessage="No customers yet. Add your first customer to start invoicing." />
      {showCreate && <CreateCustomerModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateCustomerModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();

  const [form, setForm] = useState({
    // Basic info
    name: '',
    first_name: '',
    last_name: '',
    company_name: '',
    account_number: '',
    // Contact
    email: '',
    phone: '',
    mobile: '',
    website: '',
    // Tax
    kra_pin: '',
    vat_number: '',
    // Billing address
    billing_address_1: '',
    billing_address_2: '',
    billing_city: '',
    billing_county: '',
    billing_postal: '',
    billing_country: 'Kenya',
    // Shipping address
    shipping_same: true,
    shipping_address_1: '',
    shipping_address_2: '',
    shipping_city: '',
    shipping_county: '',
    shipping_postal: '',
    shipping_country: 'Kenya',
    // Settings
    currency: 'KES',
    payment_terms: 'Net30',
    credit_limit: '',
    // Notes
    notes: '',
  });

  const mutation = useMutation({
    mutationFn: (data: any) => createCustomer(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['customers'] });
      onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const customerName = form.company_name || `${form.first_name} ${form.last_name}`.trim() || form.name;
    mutation.mutate({
      name: customerName,
      kra_pin: form.kra_pin || undefined,
      vat_number: form.vat_number || undefined,
      email: form.email ? [{ email: form.email, label: 'Main', is_primary: true }] : [],
      phone: [
        ...(form.phone ? [{ number: form.phone, label: 'Office', is_primary: true, whatsapp_enabled: false }] : []),
        ...(form.mobile ? [{ number: form.mobile, label: 'Mobile', is_primary: !form.phone, whatsapp_enabled: true }] : []),
      ],
      address: form.billing_address_1 ? {
        line1: form.billing_address_1,
        line2: form.billing_address_2 || undefined,
        city: form.billing_city,
        county: form.billing_county || undefined,
        postal_code: form.billing_postal || undefined,
        country: form.billing_country,
      } : undefined,
      currency: form.currency,
      payment_terms: form.payment_terms,
      credit_limit: form.credit_limit ? parseFloat(form.credit_limit) : undefined,
      notes: form.notes || undefined,
    });
  };

  const [tab, setTab] = useState<'details' | 'address' | 'settings'>('details');

  return (
    <Modal open={true} onClose={onClose} title="Add a Customer" size="lg">
      <form onSubmit={handleSubmit}>
        {/* Tabs */}
        <div className="flex gap-1 mb-6 border-b">
          {(['details', 'address', 'settings'] as const).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTab(t)}
              className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors ${tab === t ? 'border-blue-600 text-blue-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}
            >
              {t === 'details' ? 'Contact Details' : t === 'address' ? 'Address' : 'Billing Settings'}
            </button>
          ))}
        </div>

        {/* TAB: Contact Details */}
        {tab === 'details' && (
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">First Name</label>
                <input className="input" value={form.first_name} onChange={(e) => setForm({ ...form, first_name: e.target.value })} placeholder="James" />
              </div>
              <div>
                <label className="label">Last Name</label>
                <input className="input" value={form.last_name} onChange={(e) => setForm({ ...form, last_name: e.target.value })} placeholder="Mwangi" />
              </div>
            </div>
            <div>
              <label className="label">Company / Business Name</label>
              <input className="input" value={form.company_name} onChange={(e) => setForm({ ...form, company_name: e.target.value })} placeholder="Acme Ltd" />
              <p className="text-xs text-gray-400 mt-1">If both name and company are provided, company name is used on invoices</p>
            </div>
            <div>
              <label className="label">Account Number</label>
              <input className="input" value={form.account_number} onChange={(e) => setForm({ ...form, account_number: e.target.value })} placeholder="Optional — your internal reference" />
            </div>

            <hr className="my-4" />

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label"><Mail className="inline w-3.5 h-3.5 mr-1" />Email</label>
                <input type="email" className="input" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} placeholder="accounts@company.co.ke" />
                <p className="text-xs text-gray-400 mt-1">Invoices will be sent here</p>
              </div>
              <div>
                <label className="label"><Phone className="inline w-3.5 h-3.5 mr-1" />Phone</label>
                <input className="input" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} placeholder="+254 20 XXX XXXX" />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">Mobile / WhatsApp</label>
                <input className="input" value={form.mobile} onChange={(e) => setForm({ ...form, mobile: e.target.value })} placeholder="+254 7XX XXX XXX" />
              </div>
              <div>
                <label className="label">Website</label>
                <input className="input" value={form.website} onChange={(e) => setForm({ ...form, website: e.target.value })} placeholder="https://..." />
              </div>
            </div>

            <hr className="my-4" />

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">KRA PIN</label>
                <input className="input font-mono" value={form.kra_pin} onChange={(e) => setForm({ ...form, kra_pin: e.target.value.toUpperCase() })} placeholder="P00XXXXXXX" maxLength={11} />
                <p className="text-xs text-gray-400 mt-1">Required for WHT certificates</p>
              </div>
              <div>
                <label className="label">VAT Number</label>
                <input className="input font-mono" value={form.vat_number} onChange={(e) => setForm({ ...form, vat_number: e.target.value })} placeholder="Optional" />
              </div>
            </div>
          </div>
        )}

        {/* TAB: Address */}
        {tab === 'address' && (
          <div className="space-y-4">
            <h4 className="font-medium text-sm text-gray-700 flex items-center gap-2">
              <MapPin className="w-4 h-4" /> Billing Address
            </h4>
            <div>
              <label className="label">Address Line 1</label>
              <input className="input" value={form.billing_address_1} onChange={(e) => setForm({ ...form, billing_address_1: e.target.value })} placeholder="P.O. Box or Street Address" />
            </div>
            <div>
              <label className="label">Address Line 2</label>
              <input className="input" value={form.billing_address_2} onChange={(e) => setForm({ ...form, billing_address_2: e.target.value })} placeholder="Building, Floor, Suite" />
            </div>
            <div className="grid grid-cols-3 gap-3">
              <div>
                <label className="label">City / Town</label>
                <input className="input" value={form.billing_city} onChange={(e) => setForm({ ...form, billing_city: e.target.value })} placeholder="Nairobi" />
              </div>
              <div>
                <label className="label">County</label>
                <input className="input" value={form.billing_county} onChange={(e) => setForm({ ...form, billing_county: e.target.value })} placeholder="Nairobi County" />
              </div>
              <div>
                <label className="label">Postal Code</label>
                <input className="input" value={form.billing_postal} onChange={(e) => setForm({ ...form, billing_postal: e.target.value })} placeholder="00100" />
              </div>
            </div>
            <div>
              <label className="label">Country</label>
              <select className="input" value={form.billing_country} onChange={(e) => setForm({ ...form, billing_country: e.target.value })}>
                <option>Kenya</option><option>Uganda</option><option>Tanzania</option><option>Rwanda</option><option>United Kingdom</option><option>United States</option>
              </select>
            </div>

            <hr className="my-4" />

            <label className="flex items-center gap-2 cursor-pointer">
              <input type="checkbox" checked={form.shipping_same} onChange={(e) => setForm({ ...form, shipping_same: e.target.checked })} className="rounded" />
              <span className="text-sm">Shipping address same as billing</span>
            </label>

            {!form.shipping_same && (
              <div className="space-y-3 mt-3 pl-4 border-l-2 border-gray-200">
                <h4 className="font-medium text-sm text-gray-700">Shipping Address</h4>
                <input className="input" placeholder="Address Line 1" value={form.shipping_address_1} onChange={(e) => setForm({ ...form, shipping_address_1: e.target.value })} />
                <input className="input" placeholder="Address Line 2" value={form.shipping_address_2} onChange={(e) => setForm({ ...form, shipping_address_2: e.target.value })} />
                <div className="grid grid-cols-3 gap-3">
                  <input className="input" placeholder="City" value={form.shipping_city} onChange={(e) => setForm({ ...form, shipping_city: e.target.value })} />
                  <input className="input" placeholder="County" value={form.shipping_county} onChange={(e) => setForm({ ...form, shipping_county: e.target.value })} />
                  <input className="input" placeholder="Postal Code" value={form.shipping_postal} onChange={(e) => setForm({ ...form, shipping_postal: e.target.value })} />
                </div>
              </div>
            )}
          </div>
        )}

        {/* TAB: Billing Settings */}
        {tab === 'settings' && (
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="label">Currency</label>
                <select className="input" value={form.currency} onChange={(e) => setForm({ ...form, currency: e.target.value })}>
                  <option value="KES">KES - Kenya Shilling</option>
                  <option value="USD">USD - US Dollar</option>
                  <option value="EUR">EUR - Euro</option>
                  <option value="GBP">GBP - British Pound</option>
                  <option value="UGX">UGX - Uganda Shilling</option>
                  <option value="TZS">TZS - Tanzania Shilling</option>
                </select>
                <p className="text-xs text-gray-400 mt-1">Default currency for this customer's invoices</p>
              </div>
              <div>
                <label className="label">Payment Terms</label>
                <select className="input" value={form.payment_terms} onChange={(e) => setForm({ ...form, payment_terms: e.target.value })}>
                  <option value="DueOnReceipt">Due on Receipt</option>
                  <option value="Net7">Net 7 days</option>
                  <option value="Net14">Net 14 days</option>
                  <option value="Net30">Net 30 days</option>
                  <option value="Net45">Net 45 days</option>
                  <option value="Net60">Net 60 days</option>
                  <option value="Net90">Net 90 days</option>
                </select>
                <p className="text-xs text-gray-400 mt-1">Automatically sets due date on new invoices</p>
              </div>
            </div>
            <div>
              <label className="label">Credit Limit (KES)</label>
              <input type="number" className="input" value={form.credit_limit} onChange={(e) => setForm({ ...form, credit_limit: e.target.value })} placeholder="Leave blank for no limit" />
              <p className="text-xs text-gray-400 mt-1">You'll be warned when invoicing above this amount</p>
            </div>

            <hr className="my-4" />

            <div>
              <label className="label"><FileText className="inline w-3.5 h-3.5 mr-1" />Internal Notes</label>
              <textarea className="input" rows={3} value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} placeholder="Private notes about this customer (not shown on invoices)" />
            </div>
          </div>
        )}

        {/* Submit */}
        <div className="flex justify-between items-center pt-6 mt-6 border-t">
          <p className="text-xs text-gray-400">
            {form.company_name || form.first_name ? `Creating: ${form.company_name || `${form.first_name} ${form.last_name}`}` : 'Fill in customer details'}
          </p>
          <div className="flex gap-3">
            <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
            <button
              type="submit"
              className="btn-primary"
              disabled={mutation.isPending || (!form.company_name && !form.first_name && !form.name)}
            >
              {mutation.isPending ? 'Saving...' : 'Save Customer'}
            </button>
          </div>
        </div>
      </form>
    </Modal>
  );
}
