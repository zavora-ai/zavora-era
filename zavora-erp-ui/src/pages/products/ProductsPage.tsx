import { useState, useEffect, useRef } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getProducts, createProduct, updateProduct, deleteProduct, assignPostingGroups, getSettings, getAccounts } from '../../api/client';
import { PostingGroupFields } from '../../components/shared/PostingGroupFields';
import type { Product, Account } from '../../types';
import { formatCurrency } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, Package, Tag, Pencil, Trash2 } from 'lucide-react';

export default function ProductsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [editing, setEditing] = useState<Product | null>(null);
  const queryClient = useQueryClient();
  const { data: products = [], isLoading } = useQuery<Product[]>({ queryKey: ['products'], queryFn: () => getProducts().then(r => Array.isArray(r.data) ? r.data : []) });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteProduct(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['products'] }),
    onError: (e: any) => alert(e?.response?.data?.error || e?.response?.data?.message || 'Failed to delete product.'),
  });

  const handleDelete = (p: Product) => {
    if (confirm(`Delete "${p.name}"? This can't be undone. (Blocked if it's used on any transaction — deactivate instead.)`)) {
      deleteMutation.mutate(p.id);
    }
  };

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
    { key: 'unit_price', header: 'Price', render: (r) => (r.unit_price != null && Number(r.unit_price) > 0) ? formatCurrency(Number(r.unit_price), r.currency) : <span className="text-gray-400">Variable</span>, className: 'text-right' },
    { key: 'uom', header: 'Unit' },
    { key: 'vat_treatment', header: 'Tax', render: (r) => r.vat_treatment === 'Standard16' ? 'VAT 16%' : r.vat_treatment === 'ZeroRated' ? 'Zero Rated' : 'Exempt' },
    { key: 'sales_account', header: 'Income Acct', render: (r) => <span className="font-mono text-xs">{r.sales_account}</span> },
    { key: 'track_inventory', header: 'Inventory', render: (r) => r.track_inventory ? <span className="badge-success">Tracked</span> : <span className="text-gray-400">—</span> },
    {
      key: 'actions', header: '', className: 'text-right',
      render: (r) => (
        <div className="flex items-center justify-end gap-1">
          <button onClick={(e) => { e.stopPropagation(); setEditing(r); }} className="p-1.5 text-gray-400 hover:text-blue-600 hover:bg-blue-50 rounded" title="Edit"><Pencil className="w-4 h-4" /></button>
          <button onClick={(e) => { e.stopPropagation(); handleDelete(r); }} className="p-1.5 text-gray-400 hover:text-red-600 hover:bg-red-50 rounded" title="Delete" disabled={deleteMutation.isPending}><Trash2 className="w-4 h-4" /></button>
        </div>
      ),
    },
  ];

  return (
    <div>
      <PageHeader
        title="Products & Services"
        subtitle="Items you sell or buy — auto-fill invoices and bills"
        actions={<button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> Add Product or Service</button>}
      />
      <DataTable columns={columns} data={products} loading={isLoading} onRowClick={(r) => setEditing(r)} emptyMessage="No products or services. Add items to auto-fill your invoices." />
      {showCreate && <ProductFormModal onClose={() => setShowCreate(false)} />}
      {editing && <ProductFormModal product={editing} onClose={() => setEditing(null)} />}
    </div>
  );
}

function ProductFormModal({ product, onClose }: { product?: Product; onClose: () => void }) {
  const queryClient = useQueryClient();
  const isEdit = !!product;
  const [form, setForm] = useState({
    name: product?.name ?? '',
    description: product?.description ?? '',
    product_type: (product?.product_type ?? 'Service') as 'Service' | 'Goods' | 'Expense',
    unit_price: product?.unit_price != null ? String(product.unit_price) : '',
    currency: product?.currency ?? 'KES',
    uom: product?.uom ?? 'Each',
    // Accounts
    sales_account: product?.sales_account ?? '5100',
    purchase_account: product?.purchase_account ?? '7350',
    // Tax
    vat_treatment: product?.vat_treatment ?? 'Exempt',
    // Inventory
    track_inventory: product?.track_inventory ?? false,
    sku: '',
    opening_stock: '',
    opening_unit_cost: '',
  });
  const [genGroup, setGenGroup] = useState(product?.general_product_group_id ?? '');
  const [vatGroup, setVatGroup] = useState(product?.vat_product_group_id ?? '');

  // Default the tax treatment from the company's VAT registration: VAT-registered
  // businesses default new items to Standard 16%, others to Exempt (so a company
  // that isn't VAT-registered never accidentally charges output VAT). Respects a
  // manual choice once the user picks a rate.
  const { data: cfg } = useQuery({ queryKey: ['settings'], queryFn: () => getSettings().then((r) => r.data) });
  const vatTouched = useRef(false);
  useEffect(() => {
    // Only auto-default for a NEW product — never override an existing product's
    // saved VAT treatment when editing.
    if (cfg && !isEdit && !vatTouched.current) {
      setForm((f) => ({ ...f, vat_treatment: cfg.tax_config?.vat_registered ? 'Standard16' : 'Exempt' }));
    }
  }, [cfg, isEdit]);

  // Account pickers are driven by the live chart of accounts, not a hardcoded
  // list — so any revenue/expense account (e.g. 5250 Royalty, 7350 Software)
  // is selectable. These are the per-product *fallback* accounts; the primary
  // routing is the posting-group matrix (business × product group).
  const { data: accounts = [] } = useQuery<Account[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then((r) => (Array.isArray(r.data) ? r.data : [])) });
  const incomeAccounts = accounts
    .filter((a) => (a.account_type === 'Revenue' || a.account_type === 'ContraRevenue') && a.is_active && !a.is_control)
    .sort((a, b) => a.code.localeCompare(b.code));
  const expenseAccounts = accounts
    .filter((a) => a.account_type === 'Expense' && a.is_active && !a.is_control)
    .sort((a, b) => a.code.localeCompare(b.code));

  const mutation = useMutation({
    mutationFn: (data: any) => (isEdit ? updateProduct(product!.id, data) : createProduct(data)),
    onSuccess: async (resp: any) => {
      const id = isEdit ? product!.id : (resp?.data?.id ?? resp?.data);
      // Always sync posting groups on edit (allow clearing); on create only if set.
      if (id && (isEdit || genGroup || vatGroup)) {
        try { await assignPostingGroups({ kind: 'product', id, general_group_id: genGroup || undefined, vat_group_id: vatGroup || undefined }); } catch { /* non-fatal */ }
      }
      queryClient.invalidateQueries({ queryKey: ['products'] }); onClose();
    },
    onError: (e: any) => alert(e?.response?.data?.error || e?.response?.data?.message || 'Failed to save product.'),
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
      // Stock master details — the backend creates and links the inventory
      // item (and posts opening stock) when track_inventory is on.
      sku: form.track_inventory ? form.sku || undefined : undefined,
      opening_stock: form.track_inventory && form.opening_stock ? parseFloat(form.opening_stock) : undefined,
      opening_unit_cost: form.track_inventory && form.opening_unit_cost ? parseFloat(form.opening_unit_cost) : undefined,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title={isEdit ? 'Edit Product or Service' : 'Add Product or Service'} size="lg">
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
            <input type="number" step="0.01" className="input" value={form.unit_price} onChange={(e) => setForm({ ...form, unit_price: e.target.value })} placeholder="0.00" />
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
                <input type="radio" name="vat" value={opt.value} checked={form.vat_treatment === opt.value} onChange={(e) => { vatTouched.current = true; setForm({ ...form, vat_treatment: e.target.value }); }} className="sr-only" />
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
              {!incomeAccounts.some((a) => a.code === form.sales_account) && form.sales_account && (
                <option value={form.sales_account}>{form.sales_account} — (not in chart)</option>
              )}
              {incomeAccounts.map((a) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Expense Account <span className="text-gray-400 font-normal">(when purchased)</span></label>
            <select className="input" value={form.purchase_account} onChange={(e) => setForm({ ...form, purchase_account: e.target.value })}>
              {!expenseAccounts.some((a) => a.code === form.purchase_account) && form.purchase_account && (
                <option value={form.purchase_account}>{form.purchase_account} — (not in chart)</option>
              )}
              {expenseAccounts.map((a) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
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
              <div className="mt-3 grid grid-cols-3 gap-3 pl-8">
                <div><label className="label text-xs">SKU {!product?.track_inventory && <span className="text-red-500">*</span>}</label><input className="input text-sm py-1.5 font-mono" value={form.sku} onChange={(e) => setForm({ ...form, sku: e.target.value })} placeholder="e.g. PROD-001" required={!product?.track_inventory} /></div>
                <div><label className="label text-xs">Opening Stock</label><input type="number" className="input text-sm py-1.5" value={form.opening_stock} onChange={(e) => setForm({ ...form, opening_stock: e.target.value })} placeholder="0" /></div>
                <div><label className="label text-xs">Unit Cost{form.opening_stock && parseFloat(form.opening_stock) > 0 ? <span className="text-red-500"> *</span> : null}</label><input type="number" step="0.01" className="input text-sm py-1.5" value={form.opening_unit_cost} onChange={(e) => setForm({ ...form, opening_unit_cost: e.target.value })} placeholder="0.00" required={!!form.opening_stock && parseFloat(form.opening_stock) > 0} /></div>
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
