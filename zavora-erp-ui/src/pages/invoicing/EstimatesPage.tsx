import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getEstimates, createEstimate, convertEstimate, getCustomers, getProducts } from '../../api/client';
import type { Estimate, Customer, Product } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, ArrowRight, FileText, Send } from 'lucide-react';

export default function EstimatesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [filter, setFilter] = useState<string>('all');
  const queryClient = useQueryClient();

  const { data: estimates = [], isLoading } = useQuery<Estimate[]>({
    queryKey: ['estimates'],
    queryFn: () => getEstimates().then(r => r.data),
  });

  const convertMutation = useMutation({
    mutationFn: (id: string) => convertEstimate(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['estimates'] }),
  });

  const filtered = filter === 'all' ? estimates : estimates.filter(e => e.status === filter);

  const statusCounts = {
    all: estimates.length,
    draft: estimates.filter(e => e.status === 'draft').length,
    sent: estimates.filter(e => e.status === 'sent' || e.status === 'viewed').length,
    accepted: estimates.filter(e => e.status === 'accepted').length,
    expired: estimates.filter(e => e.status === 'expired' || e.status === 'rejected').length,
    converted: estimates.filter(e => e.status === 'converted').length,
  };

  const columns: Column<Estimate>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status}</span> },
    { key: 'number', header: 'Estimate #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'customer_id', header: 'Customer', render: (r) => <span className="text-gray-900">{r.customer_id?.slice(0, 8)}...</span> },
    { key: 'issue_date', header: 'Issued', render: (r) => formatDate(r.issue_date) },
    { key: 'expiry_date', header: 'Expires', render: (r) => formatDate(r.expiry_date) },
    { key: 'gross_total', header: 'Total', render: (r) => <span className="font-medium">{formatCurrency(r.gross_total)}</span>, className: 'text-right' },
    {
      key: 'actions', header: '',
      render: (r) => (
        <div className="flex items-center gap-1">
          {(r.status === 'accepted' || r.status === 'sent' || r.status === 'draft') && r.status !== 'converted' && (
            <button
              onClick={(e) => { e.stopPropagation(); convertMutation.mutate(r.id); }}
              className="btn-success text-xs py-1 px-2"
              title="Convert to Invoice"
              disabled={convertMutation.isPending}
            >
              <ArrowRight className="w-3 h-3" /> Convert
            </button>
          )}
        </div>
      )
    },
  ];

  return (
    <div>
      <PageHeader
        title="Estimates & Quotes"
        subtitle="Create estimates and convert to invoices with one click"
        actions={
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> New Estimate
          </button>
        }
      />

      {/* Status filter tabs */}
      <div className="flex gap-1 mb-4 bg-gray-100 p-1 rounded-lg w-fit">
        {(['all', 'draft', 'sent', 'accepted', 'expired', 'converted'] as const).map((s) => (
          <button
            key={s}
            onClick={() => setFilter(s)}
            className={`px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${filter === s ? 'bg-white shadow-sm text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
          >
            {s.charAt(0).toUpperCase() + s.slice(1)} ({statusCounts[s]})
          </button>
        ))}
      </div>

      <DataTable columns={columns} data={filtered} loading={isLoading} emptyMessage="No estimates yet. Create your first estimate to send quotes to customers." />

      {showCreate && <CreateEstimateModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

// ============================================================
// Full-featured Estimate Creation
// ============================================================
function CreateEstimateModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data: customers = [] } = useQuery<Customer[]>({ queryKey: ['customers'], queryFn: () => getCustomers().then(r => r.data) });
  const { data: products = [] } = useQuery<Product[]>({ queryKey: ['products'], queryFn: () => getProducts().then(r => r.data) });

  const today = new Date().toISOString().split('T')[0];
  const defaultExpiry = new Date(Date.now() + 30 * 86400000).toISOString().split('T')[0];

  const [form, setForm] = useState({
    customer_id: '',
    issue_date: today,
    expiry_date: defaultExpiry,
    lines: [emptyLine()],
    notes: '',
    currency: 'KES',
  });

  function emptyLine() {
    return { product_id: '', description: '', quantity: 1, unit_price: 0, tax_rate: 16, account_code: '4000' };
  }

  const mutation = useMutation({
    mutationFn: (data: any) => createEstimate(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['estimates'] });
      onClose();
    },
  });

  const addLine = () => setForm({ ...form, lines: [...form.lines, emptyLine()] });

  const updateLine = (i: number, field: string, value: any) => {
    const lines = [...form.lines];
    (lines[i] as any)[field] = value;

    if (field === 'product_id' && value) {
      const product = products.find(p => p.id === value);
      if (product) {
        lines[i].description = product.description || product.name;
        lines[i].unit_price = product.unit_price || 0;
        lines[i].account_code = product.sales_account;
        lines[i].tax_rate = product.vat_treatment === 'Standard16' ? 16 : product.vat_treatment === 'ZeroRated' ? 0 : 0;
      }
    }
    setForm({ ...form, lines });
  };

  const removeLine = (i: number) => {
    if (form.lines.length === 1) return;
    setForm({ ...form, lines: form.lines.filter((_, idx) => idx !== i) });
  };

  // Calculations
  const subtotal = form.lines.reduce((sum, l) => sum + l.quantity * l.unit_price, 0);
  const taxByRate: Record<number, number> = {};
  form.lines.forEach(l => {
    const lineTotal = l.quantity * l.unit_price;
    const tax = lineTotal * l.tax_rate / 100;
    taxByRate[l.tax_rate] = (taxByRate[l.tax_rate] || 0) + tax;
  });
  const totalTax = Object.values(taxByRate).reduce((a, b) => a + b, 0);
  const grandTotal = subtotal + totalTax;

  const selectedCustomer = customers.find(c => c.id === form.customer_id);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      customer_id: form.customer_id,
      issue_date: form.issue_date,
      expiry_date: form.expiry_date,
      lines: form.lines.map(l => ({
        product_id: l.product_id || undefined,
        description: l.description,
        quantity: l.quantity,
        unit_price: l.unit_price,
        account_code: l.account_code,
        vat_treatment: l.tax_rate === 16 ? 'Standard16' : l.tax_rate === 0 ? 'ZeroRated' : 'Exempt',
      })),
      notes: form.notes || undefined,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Create Estimate" size="xl">
      <form onSubmit={handleSubmit} className="space-y-6">
        {/* Header — Customer + Dates */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Left — Customer */}
          <div className="space-y-4">
            <div>
              <label className="label">Customer *</label>
              <select
                className="input"
                value={form.customer_id}
                onChange={(e) => setForm({ ...form, customer_id: e.target.value })}
                required
              >
                <option value="">Choose a customer...</option>
                {customers.map(c => (
                  <option key={c.id} value={c.id}>{c.name}</option>
                ))}
              </select>
            </div>
            {selectedCustomer && (
              <div className="bg-gray-50 rounded-lg p-3 text-sm text-gray-600">
                <p className="font-medium text-gray-900">{selectedCustomer.name}</p>
                {selectedCustomer.email?.[0] && <p>{selectedCustomer.email[0].email}</p>}
                {selectedCustomer.kra_pin && <p>PIN: {selectedCustomer.kra_pin}</p>}
              </div>
            )}
          </div>

          {/* Right — Estimate details */}
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="label">Issue Date</label>
                <input type="date" className="input" value={form.issue_date} onChange={(e) => setForm({ ...form, issue_date: e.target.value })} />
              </div>
              <div>
                <label className="label">Expiry Date</label>
                <input type="date" className="input" value={form.expiry_date} onChange={(e) => setForm({ ...form, expiry_date: e.target.value })} />
              </div>
            </div>
            <div>
              <label className="label">Currency</label>
              <select className="input" value={form.currency} onChange={(e) => setForm({ ...form, currency: e.target.value })}>
                <option value="KES">KES - Kenya Shilling</option>
                <option value="USD">USD - US Dollar</option>
                <option value="EUR">EUR - Euro</option>
                <option value="GBP">GBP - British Pound</option>
              </select>
            </div>
          </div>
        </div>

        {/* Line Items */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="label mb-0">Items</label>
          </div>
          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-3">Product / Service</div>
              <div className="col-span-3">Description</div>
              <div className="col-span-1">Qty</div>
              <div className="col-span-2">Price</div>
              <div className="col-span-1">Tax</div>
              <div className="col-span-1 text-right">Amount</div>
              <div className="col-span-1"></div>
            </div>
            {form.lines.map((line, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-3">
                  <select className="input text-sm py-1.5" value={line.product_id} onChange={(e) => updateLine(i, 'product_id', e.target.value)}>
                    <option value="">Select item...</option>
                    {products.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                </div>
                <div className="col-span-3">
                  <input className="input text-sm py-1.5" placeholder="Description" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} />
                </div>
                <div className="col-span-1">
                  <input className="input text-sm py-1.5 text-center" type="number" min="1" step="0.01" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} />
                </div>
                <div className="col-span-2">
                  <input className="input text-sm py-1.5" type="number" min="0" step="0.01" value={line.unit_price} onChange={(e) => updateLine(i, 'unit_price', +e.target.value)} />
                </div>
                <div className="col-span-1">
                  <select className="input text-sm py-1.5" value={line.tax_rate} onChange={(e) => updateLine(i, 'tax_rate', +e.target.value)}>
                    <option value={16}>16%</option>
                    <option value={8}>8%</option>
                    <option value={0}>0%</option>
                  </select>
                </div>
                <div className="col-span-1 text-right text-sm font-medium">
                  {formatCurrency(line.quantity * line.unit_price)}
                </div>
                <div className="col-span-1 text-center">
                  <button type="button" onClick={() => removeLine(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={form.lines.length === 1}>×</button>
                </div>
              </div>
            ))}
          </div>
          <button type="button" onClick={addLine} className="mt-2 text-sm font-medium text-blue-600 hover:text-blue-800">
            + Add a Line
          </button>
        </div>

        {/* Notes + Totals */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <div>
            <label className="label">Notes <span className="text-gray-400 font-normal">(visible to customer)</span></label>
            <textarea className="input" rows={3} value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} placeholder="Add terms, scope of work, or special conditions..." />
          </div>

          <div className="bg-gray-50 rounded-lg p-4 space-y-2">
            <div className="flex justify-between text-sm">
              <span className="text-gray-600">Subtotal</span>
              <span className="font-medium">{formatCurrency(subtotal)}</span>
            </div>
            {Object.entries(taxByRate).map(([rate, amount]) => (
              <div key={rate} className="flex justify-between text-sm">
                <span className="text-gray-600">VAT ({rate}%)</span>
                <span>{formatCurrency(amount)}</span>
              </div>
            ))}
            <div className="border-t pt-2 mt-2 flex justify-between text-base font-bold">
              <span>Total ({form.currency})</span>
              <span>{formatCurrency(grandTotal)}</span>
            </div>
          </div>
        </div>

        {/* Footer actions */}
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.customer_id}>
            {mutation.isPending ? 'Saving...' : 'Save Estimate'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
