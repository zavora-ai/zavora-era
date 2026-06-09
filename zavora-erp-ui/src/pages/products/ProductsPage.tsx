import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getProducts, createProduct } from '../../api/client';
import type { Product } from '../../types';
import { formatCurrency } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus } from 'lucide-react';

export default function ProductsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const { data: products = [], isLoading } = useQuery<Product[]>({ queryKey: ['products'], queryFn: () => getProducts().then(r => r.data) });

  const columns: Column<Product>[] = [
    { key: 'name', header: 'Name', render: (r) => <span className="font-medium">{r.name}</span> },
    { key: 'product_type', header: 'Type', render: (r) => <span className="badge-info">{r.product_type}</span> },
    { key: 'unit_price', header: 'Price', render: (r) => r.unit_price ? formatCurrency(r.unit_price) : '—', className: 'text-right' },
    { key: 'uom', header: 'UoM' },
    { key: 'vat_treatment', header: 'VAT', render: (r) => r.vat_treatment === 'Standard16' ? '16%' : r.vat_treatment === 'ZeroRated' ? '0%' : r.vat_treatment },
    { key: 'sales_account', header: 'Sales Acct' },
    { key: 'track_inventory', header: 'Inventory', render: (r) => r.track_inventory ? '✓' : '—' },
  ];

  return (
    <div>
      <PageHeader title="Products & Services" subtitle="Catalog items for invoices and bills" actions={<button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> New Product</button>} />
      <DataTable columns={columns} data={products} loading={isLoading} emptyMessage="No products or services." />
      {showCreate && <CreateProductModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateProductModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ name: '', description: '', product_type: 'Service', unit_price: 0, uom: 'Each', vat_treatment: 'Standard16', track_inventory: false });
  const mutation = useMutation({ mutationFn: (data: any) => createProduct(data), onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['products'] }); onClose(); } });

  const handleSubmit = (e: React.FormEvent) => { e.preventDefault(); mutation.mutate(form); };

  return (
    <Modal open={true} onClose={onClose} title="New Product / Service">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div><label className="label">Name *</label><input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} required /></div>
        <div><label className="label">Description</label><input className="input" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} /></div>
        <div className="grid grid-cols-3 gap-4">
          <div><label className="label">Type</label><select className="input" value={form.product_type} onChange={(e) => setForm({ ...form, product_type: e.target.value })}><option value="Service">Service</option><option value="Goods">Goods</option><option value="Expense">Expense</option></select></div>
          <div><label className="label">Unit Price</label><input type="number" className="input" step="0.01" value={form.unit_price} onChange={(e) => setForm({ ...form, unit_price: +e.target.value })} /></div>
          <div><label className="label">Unit of Measure</label><select className="input" value={form.uom} onChange={(e) => setForm({ ...form, uom: e.target.value })}><option>Each</option><option>Hour</option><option>Day</option><option>Month</option><option>Kg</option><option>Litre</option></select></div>
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div><label className="label">VAT Treatment</label><select className="input" value={form.vat_treatment} onChange={(e) => setForm({ ...form, vat_treatment: e.target.value })}><option value="Standard16">Standard (16%)</option><option value="ZeroRated">Zero Rated</option><option value="Exempt">Exempt</option></select></div>
          <div className="flex items-end"><label className="flex items-center gap-2 pb-2"><input type="checkbox" checked={form.track_inventory} onChange={(e) => setForm({ ...form, track_inventory: e.target.checked })} /><span className="text-sm">Track Inventory</span></label></div>
        </div>
        <div className="flex justify-end gap-3 pt-4 border-t"><button type="button" onClick={onClose} className="btn-secondary">Cancel</button><button type="submit" className="btn-primary" disabled={mutation.isPending}>{mutation.isPending ? 'Creating...' : 'Create Product'}</button></div>
      </form>
    </Modal>
  );
}
