import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getProducts, createProduct, assignPostingGroups } from '../../api/client';
import { PostingGroupFields } from '../../components/shared/PostingGroupFields';
import type { Product } from '../../types';
import { formatCurrency } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, Package, Tag } from 'lucide-react';

export default function ProductsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const { data: products = [], isLoading } = useQuery<Product[]>({ queryKey: ['products'], queryFn: () => getProducts().then(r => Array.isArray(r.data) ? r.data : []) });

  const columns: Column<Product>[] = [
    {
      key: 'name', header: 'Product / Service',
      render: (r) => (
        <div className="flex items-center gap-3">
          <div className={`w-8 h-8 rounded-lg flex items-center justify-center ${r.product_type === 'Service' ? 'bg-purple-100 text-purple-600' : r.product_type === 'Goods' ? 'bg-blue-100 text-blue-600' : 'bg-orange-100 text-orange-600'}`}>
            {r.product_type === 'Service' ? <Tag className="w-4 h-4" /> : <Package className="w-4 h-4" />}
          </div>
          <div>
            <p className="font-medium text-gray-900">{r.name}</p>
            {r.description && <p className="text-xs text-gray-500 truncate max-w-[200px]">{r.description}</p>}
          </div>
        </div>
      )
    },
    { key: 'product_type', header: 'Type', render: (r) => <span className={r.product_type === 'Service' ? 'badge-info' : r.product_type === 'Goods' ? 'badge-success' : 'badge-warning'}>{r.product_type}</span> },
    { key: 'unit_price', header: 'Price', render: (r) => r.unit_price ? formatCurrency(r.unit_price) : <span className="text-gray-400">Variable</span>, className: 'text-right' },
    { key: 'uom', header: 'Unit' },
    { key: 'vat_treatment', header: 'Tax', render: (r) => r.vat_treatment === 'Standard16' ? 'VAT 16%' : r.vat_treatment === 'ZeroRated' ? 'Zero Rated' : 'Exempt' },
    { key: 'sales_account', header: 'Income Acct', render: (r) => <span className="font-mono text-xs">{r.sales_account}</span> },
    { key: 'track_inventory', header: 'Inventory', render: (r) => r.track_inventory ? <span className="badge-success">Tracked</span> : <span className="text-gray-400">—</span> },
  ];

  return (
    <div>
      <PageHeader
        title="Products & Services"
        subtitle="Items you sell or buy — auto-fill invoices and bills"
        actions={<button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> Add Product or Service</button>}
      />
      <DataTable columns={columns} data={products} loading={isLoading} emptyMessage="No products or services. Add items to auto-fill your invoices." />
      {showCreate && <CreateProductModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateProductModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    name: '',
    description: '',
    product_type: 'Service' as 'Service' | 'Goods' | 'Expense',
    unit_price: '',
    currency: 'KES',
    uom: 'Each',
    // Accounts
    sales_account: '5100',
    purchase_account: '6000',
    // Tax
    vat_treatment: 'Standard16',
    // Inventory
    track_inventory: false,
    sku: '',
    opening_stock: '',
  });
  const [genGroup, setGenGroup] = useState('');
  const [vatGroup, setVatGroup] = useState('');

  const mutation = useMutation({
    mutationFn: (data: any) => createProduct(data),
    onSuccess: async (resp: any) => {
      const id = resp?.data?.id ?? resp?.data;
      if (id && (genGroup || vatGroup)) {
        try { await assignPostingGroups({ kind: 'product', id, general_group_id: genGroup || undefined, vat_group_id: vatGroup || undefined }); } catch { /* non-fatal */ }
      }
      queryClient.invalidateQueries({ queryKey: ['products'] }); onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      name: form.name,
      description: form.description || undefined,
      product_type: form.product_type,
      unit_price: form.unit_price ? parseFloat(form.unit_price) : undefined,
      currency: form.currency,
      uom: form.uom,
      sales_account: form.sales_account,
      purchase_account: form.purchase_account,
      vat_treatment: form.vat_treatment,
      track_inventory: form.track_inventory,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Add Product or Service" size="lg">
      <form onSubmit={handleSubmit} className="space-y-5">
        {/* Type selection */}
        <div>
          <label className="label">What are you adding?</label>
          <div className="grid grid-cols-3 gap-3 mt-1">
            {[
              { type: 'Service', label: 'Service', desc: 'Consulting, design, labour', icon: '💼' },
              { type: 'Goods', label: 'Product', desc: 'Physical items you sell', icon: '📦' },
              { type: 'Expense', label: 'Expense Item', desc: 'Things you buy/consume', icon: '🧾' },
            ].map(opt => (
              <button
                key={opt.type}
                type="button"
                onClick={() => setForm({ ...form, product_type: opt.type as any, sales_account: opt.type === 'Service' ? '5100' : '5000', purchase_account: opt.type === 'Expense' ? '7900' : '6000' })}
                className={`p-3 rounded-lg border-2 text-left transition-colors ${form.product_type === opt.type ? 'border-blue-500 bg-blue-50' : 'border-gray-200 hover:border-gray-300'}`}
              >
                <span className="text-xl">{opt.icon}</span>
                <p className="font-medium text-sm mt-1">{opt.label}</p>
                <p className="text-xs text-gray-500">{opt.desc}</p>
              </button>
            ))}
          </div>
        </div>

        {/* Name & Description */}
        <div>
          <label className="label">Name *</label>
          <input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="e.g. Web Design Services, Office Chair, Taxi Fare" required />
        </div>
        <div>
          <label className="label">Description <span className="text-gray-400 font-normal">(auto-fills on invoices)</span></label>
          <textarea className="input" rows={2} value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="Detailed description shown on invoices and bills" />
        </div>

        {/* Price & Units */}
        <div className="grid grid-cols-3 gap-4">
          <div>
            <label className="label">Default Price</label>
            <div className="relative">
              <span className="absolute left-3 top-1/2 -translate-y-1/2 text-sm text-gray-400">{form.currency}</span>
              <input type="number" step="0.01" className="input pl-12" value={form.unit_price} onChange={(e) => setForm({ ...form, unit_price: e.target.value })} placeholder="0.00" />
            </div>
            <p className="text-xs text-gray-400 mt-1">Can be changed per invoice</p>
          </div>
          <div>
            <label className="label">Unit of Measure</label>
            <select className="input" value={form.uom} onChange={(e) => setForm({ ...form, uom: e.target.value })}>
              <option value="Each">Each</option>
              <option value="Hour">Hour</option>
              <option value="Day">Day</option>
              <option value="Week">Week</option>
              <option value="Month">Month</option>
              <option value="Kg">Kilogram (Kg)</option>
              <option value="Litre">Litre</option>
              <option value="Metre">Metre</option>
              <option value="Box">Box</option>
              <option value="Pack">Pack</option>
            </select>
          </div>
          <div>
            <label className="label">Currency</label>
            <select className="input" value={form.currency} onChange={(e) => setForm({ ...form, currency: e.target.value })}>
              <option value="KES">KES</option><option value="USD">USD</option><option value="EUR">EUR</option><option value="GBP">GBP</option>
            </select>
          </div>
        </div>

        {/* Tax */}
        <div>
          <label className="label">VAT Treatment</label>
          <div className="grid grid-cols-3 gap-3">
            {[
              { value: 'Standard16', label: 'Standard Rate (16%)', desc: 'Most goods & services' },
              { value: 'ZeroRated', label: 'Zero Rated (0%)', desc: 'Exports, basic food' },
              { value: 'Exempt', label: 'Exempt', desc: 'Financial services, land' },
            ].map(opt => (
              <label key={opt.value} className={`p-3 rounded-lg border cursor-pointer transition-colors ${form.vat_treatment === opt.value ? 'border-blue-500 bg-blue-50' : 'border-gray-200 hover:border-gray-300'}`}>
                <input type="radio" name="vat" value={opt.value} checked={form.vat_treatment === opt.value} onChange={(e) => setForm({ ...form, vat_treatment: e.target.value })} className="sr-only" />
                <p className="text-sm font-medium">{opt.label}</p>
                <p className="text-xs text-gray-500">{opt.desc}</p>
              </label>
            ))}
          </div>
        </div>

        <PostingGroupFields scope="product" generalId={genGroup} vatId={vatGroup} onGeneral={setGenGroup} onVat={setVatGroup} />

        {/* Accounts (fallback when no posting-group match) */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Income Account <span className="text-gray-400 font-normal">(fallback when sold)</span></label>
            <select className="input" value={form.sales_account} onChange={(e) => setForm({ ...form, sales_account: e.target.value })}>
              <option value="5000">5000 — Sales Revenue</option>
              <option value="5100">5100 — Service Revenue</option>
              <option value="5200">5200 — Other Income</option>
            </select>
          </div>
          <div>
            <label className="label">Expense Account <span className="text-gray-400 font-normal">(when purchased)</span></label>
            <select className="input" value={form.purchase_account} onChange={(e) => setForm({ ...form, purchase_account: e.target.value })}>
              <option value="6000">6000 — Cost of Goods Sold</option>
              <option value="6100">6100 — Direct Materials</option>
              <option value="7300">7300 — Office Supplies</option>
              <option value="7900">7900 — Miscellaneous Expenses</option>
            </select>
          </div>
        </div>

        {/* Inventory tracking */}
        {form.product_type === 'Goods' && (
          <div className="bg-gray-50 rounded-lg p-4">
            <label className="flex items-center gap-3 cursor-pointer">
              <input type="checkbox" checked={form.track_inventory} onChange={(e) => setForm({ ...form, track_inventory: e.target.checked })} className="rounded" />
              <div>
                <p className="text-sm font-medium">Track inventory for this product</p>
                <p className="text-xs text-gray-500">Monitor stock levels, get alerts on low stock, FIFO/Weighted Average costing</p>
              </div>
            </label>
            {form.track_inventory && (
              <div className="mt-3 grid grid-cols-2 gap-3 pl-8">
                <div><label className="label text-xs">SKU</label><input className="input text-sm py-1.5 font-mono" value={form.sku} onChange={(e) => setForm({ ...form, sku: e.target.value })} placeholder="e.g. PROD-001" /></div>
                <div><label className="label text-xs">Opening Stock</label><input type="number" className="input text-sm py-1.5" value={form.opening_stock} onChange={(e) => setForm({ ...form, opening_stock: e.target.value })} placeholder="0" /></div>
              </div>
            )}
          </div>
        )}

        {/* Submit */}
        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.name}>
            {mutation.isPending ? 'Saving...' : 'Save'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
