import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getBills, createBill, approveBill, getVendors } from '../../api/client';
import type { Bill, Vendor } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, CheckCircle } from 'lucide-react';

export default function BillsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const queryClient = useQueryClient();
  const { data: bills = [], isLoading } = useQuery<Bill[]>({ queryKey: ['bills'], queryFn: () => getBills().then(r => r.data) });
  const approveMut = useMutation({ mutationFn: (id: string) => approveBill(id), onSuccess: () => queryClient.invalidateQueries({ queryKey: ['bills'] }) });

  const columns: Column<Bill>[] = [
    { key: 'number', header: 'Number', render: (r) => <span className="font-medium">{r.number}</span> },
    { key: 'vendor_id', header: 'Vendor', render: (r) => r.vendor_id?.slice(0, 8) + '...' },
    { key: 'issue_date', header: 'Date', render: (r) => formatDate(r.issue_date) },
    { key: 'due_date', header: 'Due', render: (r) => formatDate(r.due_date) },
    { key: 'gross_total', header: 'Amount', render: (r) => formatCurrency(r.gross_total), className: 'text-right' },
    { key: 'wht_amount', header: 'WHT', render: (r) => r.wht_amount > 0 ? formatCurrency(r.wht_amount) : '—', className: 'text-right' },
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status.replace('_', ' ')}</span> },
    { key: 'actions', header: '', render: (r) => r.status === 'pending_approval' ? (<button onClick={(e) => { e.stopPropagation(); approveMut.mutate(r.id); }} className="btn-success text-xs py-1 px-2"><CheckCircle className="w-3 h-3" /> Approve</button>) : null },
  ];

  return (
    <div>
      <PageHeader title="Bills" subtitle="Accounts payable — vendor invoices" actions={<button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> New Bill</button>} />
      <DataTable columns={columns} data={bills} loading={isLoading} emptyMessage="No bills yet." />
      {showCreate && <CreateBillModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateBillModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data: vendors = [] } = useQuery<Vendor[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then(r => r.data) });
  const [form, setForm] = useState({ vendor_id: '', issue_date: new Date().toISOString().split('T')[0], vendor_invoice_number: '', lines: [{ description: '', quantity: 1, unit_price: 0 }] });
  const mutation = useMutation({ mutationFn: (data: any) => createBill(data), onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['bills'] }); onClose(); } });

  const addLine = () => setForm({ ...form, lines: [...form.lines, { description: '', quantity: 1, unit_price: 0 }] });
  const updateLine = (i: number, f: string, v: any) => { const lines = [...form.lines]; (lines[i] as any)[f] = v; setForm({ ...form, lines }); };

  const handleSubmit = (e: React.FormEvent) => { e.preventDefault(); mutation.mutate({ vendor_id: form.vendor_id, issue_date: form.issue_date, vendor_invoice_number: form.vendor_invoice_number || undefined, lines: form.lines.map(l => ({ description: l.description, quantity: l.quantity, unit_price: l.unit_price })) }); };

  return (
    <Modal open={true} onClose={onClose} title="New Bill" size="lg">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-3 gap-4">
          <div><label className="label">Vendor *</label><select className="input" value={form.vendor_id} onChange={(e) => setForm({ ...form, vendor_id: e.target.value })} required><option value="">Select...</option>{vendors.map(v => <option key={v.id} value={v.id}>{v.name}</option>)}</select></div>
          <div><label className="label">Date</label><input type="date" className="input" value={form.issue_date} onChange={(e) => setForm({ ...form, issue_date: e.target.value })} /></div>
          <div><label className="label">Vendor Invoice #</label><input className="input" value={form.vendor_invoice_number} onChange={(e) => setForm({ ...form, vendor_invoice_number: e.target.value })} /></div>
        </div>
        <div className="space-y-2">
          {form.lines.map((line, i) => (
            <div key={i} className="grid grid-cols-12 gap-2">
              <input className="input col-span-6" placeholder="Description" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} required />
              <input className="input col-span-2" type="number" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} />
              <input className="input col-span-3" type="number" step="0.01" value={line.unit_price} onChange={(e) => updateLine(i, 'unit_price', +e.target.value)} />
              <button type="button" onClick={() => setForm({ ...form, lines: form.lines.filter((_, idx) => idx !== i) })} className="col-span-1 text-red-500">×</button>
            </div>
          ))}
          <button type="button" onClick={addLine} className="text-sm text-blue-600">+ Add line</button>
        </div>
        <div className="flex justify-end gap-3 pt-4 border-t"><button type="button" onClick={onClose} className="btn-secondary">Cancel</button><button type="submit" className="btn-primary" disabled={mutation.isPending}>{mutation.isPending ? 'Creating...' : 'Create Bill'}</button></div>
      </form>
    </Modal>
  );
}
