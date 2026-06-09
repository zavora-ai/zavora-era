import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getInvoices, createInvoice, postInvoice, getCustomers } from '../../api/client';
import type { Invoice, Customer } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, Send, CheckCircle } from 'lucide-react';

export default function InvoicesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const queryClient = useQueryClient();

  const { data: invoices = [], isLoading } = useQuery<Invoice[]>({
    queryKey: ['invoices'],
    queryFn: () => getInvoices().then(r => r.data),
  });

  const postMutation = useMutation({
    mutationFn: (id: string) => postInvoice(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['invoices'] }),
  });

  const columns: Column<Invoice>[] = [
    { key: 'number', header: 'Number', render: (r) => <span className="font-medium">{r.number}</span> },
    { key: 'customer_id', header: 'Customer', render: (r) => r.customer_id?.slice(0, 8) + '...' },
    { key: 'issue_date', header: 'Date', render: (r) => formatDate(r.issue_date) },
    { key: 'due_date', header: 'Due', render: (r) => formatDate(r.due_date) },
    { key: 'gross_total', header: 'Amount', render: (r) => formatCurrency(r.gross_total), className: 'text-right' },
    { key: 'balance_due', header: 'Balance', render: (r) => formatCurrency(r.balance_due), className: 'text-right' },
    {
      key: 'status', header: 'Status',
      render: (r) => <span className={statusColor(r.status)}>{r.status}</span>
    },
    {
      key: 'actions', header: '',
      render: (r) => r.status === 'draft' ? (
        <button onClick={(e) => { e.stopPropagation(); postMutation.mutate(r.id); }} className="btn-primary text-xs py-1 px-2">
          <CheckCircle className="w-3 h-3" /> Post
        </button>
      ) : null
    },
  ];

  return (
    <div>
      <PageHeader
        title="Invoices"
        subtitle="Manage sales invoices and credit notes"
        actions={
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> New Invoice
          </button>
        }
      />

      <DataTable columns={columns} data={invoices} loading={isLoading} emptyMessage="No invoices yet. Create your first invoice." />

      {showCreate && <CreateInvoiceModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateInvoiceModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data: customers = [] } = useQuery<Customer[]>({
    queryKey: ['customers'],
    queryFn: () => getCustomers().then(r => r.data),
  });

  const [form, setForm] = useState({
    customer_id: '',
    issue_date: new Date().toISOString().split('T')[0],
    lines: [{ description: '', quantity: 1, unit_price: 0, account_code: '5000' }],
    notes: '',
  });

  const mutation = useMutation({
    mutationFn: (data: any) => createInvoice(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['invoices'] });
      onClose();
    },
  });

  const addLine = () => {
    setForm({ ...form, lines: [...form.lines, { description: '', quantity: 1, unit_price: 0, account_code: '5000' }] });
  };

  const updateLine = (i: number, field: string, value: any) => {
    const lines = [...form.lines];
    (lines[i] as any)[field] = value;
    setForm({ ...form, lines });
  };

  const removeLine = (i: number) => {
    setForm({ ...form, lines: form.lines.filter((_, idx) => idx !== i) });
  };

  const subtotal = form.lines.reduce((sum, l) => sum + l.quantity * l.unit_price, 0);
  const tax = subtotal * 0.16;
  const total = subtotal + tax;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      customer_id: form.customer_id,
      issue_date: form.issue_date,
      lines: form.lines.map(l => ({
        description: l.description,
        quantity: l.quantity,
        unit_price: l.unit_price,
        account_code: l.account_code,
      })),
      notes: form.notes || undefined,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Create Invoice" size="xl">
      <form onSubmit={handleSubmit} className="space-y-6">
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Customer</label>
            <select className="input" value={form.customer_id} onChange={(e) => setForm({ ...form, customer_id: e.target.value })} required>
              <option value="">Select customer...</option>
              {customers.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Issue Date</label>
            <input type="date" className="input" value={form.issue_date} onChange={(e) => setForm({ ...form, issue_date: e.target.value })} />
          </div>
        </div>

        {/* Line items */}
        <div>
          <label className="label">Line Items</label>
          <div className="space-y-2">
            <div className="grid grid-cols-12 gap-2 text-xs font-medium text-gray-500 px-1">
              <div className="col-span-5">Description</div>
              <div className="col-span-2">Qty</div>
              <div className="col-span-2">Price</div>
              <div className="col-span-2">Total</div>
              <div className="col-span-1"></div>
            </div>
            {form.lines.map((line, i) => (
              <div key={i} className="grid grid-cols-12 gap-2">
                <input className="input col-span-5" placeholder="Description" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} required />
                <input className="input col-span-2" type="number" min="1" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} />
                <input className="input col-span-2" type="number" min="0" step="0.01" value={line.unit_price} onChange={(e) => updateLine(i, 'unit_price', +e.target.value)} />
                <div className="col-span-2 flex items-center text-sm font-medium">{formatCurrency(line.quantity * line.unit_price)}</div>
                <button type="button" onClick={() => removeLine(i)} className="col-span-1 text-red-500 hover:text-red-700 text-sm">×</button>
              </div>
            ))}
            <button type="button" onClick={addLine} className="text-sm text-blue-600 hover:text-blue-800">+ Add line</button>
          </div>
        </div>

        {/* Totals */}
        <div className="flex justify-end">
          <div className="w-64 space-y-2 text-sm">
            <div className="flex justify-between"><span className="text-gray-500">Subtotal</span><span>{formatCurrency(subtotal)}</span></div>
            <div className="flex justify-between"><span className="text-gray-500">VAT (16%)</span><span>{formatCurrency(tax)}</span></div>
            <div className="flex justify-between font-bold text-base border-t pt-2"><span>Total</span><span>{formatCurrency(total)}</span></div>
          </div>
        </div>

        <div>
          <label className="label">Notes</label>
          <textarea className="input" rows={2} value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} placeholder="Optional notes for the customer" />
        </div>

        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Creating...' : 'Create Invoice'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
