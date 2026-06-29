import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getSupplierCreditNotes,
  createSupplierCreditNote,
  getVendors,
  getBills,
  getAccounts,
} from '../../api/client';
import type { Vendor, Bill, Account, SupplierCreditNote } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { workToday } from '../../utils/workDate';
import { hasRole, ROLES_CREATE } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, AlertCircle, FileMinus } from 'lucide-react';

export default function SupplierCreditNotesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const { data: notes = [], isLoading } = useQuery<SupplierCreditNote[]>({
    queryKey: ['supplier-credit-notes'],
    queryFn: () => getSupplierCreditNotes().then(r => Array.isArray(r.data) ? r.data : []),
  });
  const { data: vendors = [] } = useQuery<Vendor[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then(r => Array.isArray(r.data) ? r.data : []) });

  const vendorName = (id: string) => vendors.find(v => v.id === id)?.name ?? `${id.slice(0, 8)}...`;

  const columns: Column<SupplierCreditNote>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status}</span> },
    { key: 'credit_note_number', header: 'CN #', render: (r) => <span className="font-medium text-blue-600">{r.credit_note_number}</span> },
    { key: 'vendor_id', header: 'Vendor', render: (r) => <span className="text-gray-900">{vendorName(r.vendor_id)}</span> },
    { key: 'credit_note_date', header: 'Date', render: (r) => formatDate(r.credit_note_date) },
    { key: 'gross_total', header: 'Total', render: (r) => <span className="font-medium">{formatCurrency(r.gross_total)}</span>, className: 'text-right' },
  ];

  return (
    <div>
      <PageHeader
        title="Supplier Credit Notes"
        subtitle="Record credits issued by suppliers against bills (reverses AP and input VAT)"
        actions={
          hasRole(ROLES_CREATE) ? (
            <button onClick={() => setShowCreate(true)} className="btn-primary">
              <Plus className="w-4 h-4" /> New Supplier Credit Note
            </button>
          ) : undefined
        }
      />
      <DataTable
        columns={columns}
        data={notes}
        loading={isLoading}
        emptyMessage="No supplier credit notes yet. Record one when a vendor credits you for a returned or over-billed purchase."
      />
      {showCreate && <CreateModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

interface LineForm {
  description: string;
  quantity: number;
  unit_price: number;
  tax_rate: number;
  account_code: string;
}

function CreateModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data: vendors = [] } = useQuery<Vendor[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then(r => Array.isArray(r.data) ? r.data : []) });
  const { data: bills = [] } = useQuery<Bill[]>({ queryKey: ['bills', 'all'], queryFn: () => getBills({ limit: 500 }).then(r => r.data.data ?? r.data) });
  const { data: accounts = [] } = useQuery<Account[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then(r => Array.isArray(r.data) ? r.data : []) });
  const [error, setError] = useState<string | null>(null);

  const today = workToday();
  const [form, setForm] = useState({
    vendor_id: '',
    applies_to_bill: '',
    credit_note_date: today,
    reason: '',
    lines: [emptyLine()] as LineForm[],
  });

  function emptyLine(): LineForm {
    return { description: '', quantity: 1, unit_price: 0, tax_rate: 16, account_code: '' };
  }

  const mutation = useMutation({
    mutationFn: (data: any) => createSupplierCreditNote(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['supplier-credit-notes'] });
      onClose();
    },
    onError: (e: any) => {
      setError(e?.response?.data?.error || e?.response?.data?.message || 'Failed to create supplier credit note.');
    },
  });

  const vendorBills = bills.filter(b => !form.vendor_id || b.vendor_id === form.vendor_id);

  const addLine = () => setForm({ ...form, lines: [...form.lines, emptyLine()] });
  const updateLine = (i: number, field: keyof LineForm, value: any) => {
    const lines = [...form.lines];
    (lines[i] as any)[field] = value;
    setForm({ ...form, lines });
  };
  const removeLine = (i: number) => {
    if (form.lines.length === 1) return;
    setForm({ ...form, lines: form.lines.filter((_, idx) => idx !== i) });
  };

  const subtotal = form.lines.reduce((s, l) => s + l.quantity * l.unit_price, 0);
  const totalTax = form.lines.reduce((s, l) => s + (l.quantity * l.unit_price * l.tax_rate) / 100, 0);
  const grandTotal = subtotal + totalTax;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    mutation.mutate({
      vendor_id: form.vendor_id,
      applies_to_bill: form.applies_to_bill || undefined,
      credit_note_date: form.credit_note_date,
      reason: form.reason,
      lines: form.lines.map(l => ({
        description: l.description,
        quantity: l.quantity,
        unit_price: l.unit_price,
        account_code: l.account_code || undefined,
        vat_treatment: l.tax_rate === 16 ? 'Standard16' : l.tax_rate === 0 ? 'ZeroRated' : 'Exempt',
      })),
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="New Supplier Credit Note" subtitle="Reverses accounts payable and input VAT for this vendor" size="xl">
      <form onSubmit={handleSubmit} className="space-y-6">
        {error && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 text-red-700 text-sm">
            <AlertCircle className="w-4 h-4 shrink-0" /><span>{error}</span>
          </div>
        )}

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
          <div>
            <label className="label">Vendor *</label>
            <select className="input" value={form.vendor_id} onChange={(e) => setForm({ ...form, vendor_id: e.target.value, applies_to_bill: '' })} required>
              <option value="">Choose a vendor...</option>
              {vendors.map(v => <option key={v.id} value={v.id}>{v.name}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Applies to Bill <span className="text-gray-400 font-normal">(optional)</span></label>
            <select className="input" value={form.applies_to_bill} onChange={(e) => setForm({ ...form, applies_to_bill: e.target.value })}>
              <option value="">None</option>
              {vendorBills.map(b => <option key={b.id} value={b.id}>{b.number}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Date</label>
            <input type="date" className="input" value={form.credit_note_date} onChange={(e) => setForm({ ...form, credit_note_date: e.target.value })} />
          </div>
        </div>

        <div>
          <label className="label">Reason *</label>
          <input className="input" value={form.reason} onChange={(e) => setForm({ ...form, reason: e.target.value })} placeholder="e.g. Returned goods, price adjustment" required />
        </div>

        {/* Lines */}
        <div>
          <label className="label mb-0">Items</label>
          <div className="border rounded-lg overflow-hidden mt-2">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-4">Description</div>
              <div className="col-span-1">Qty</div>
              <div className="col-span-2">Price</div>
              <div className="col-span-1">Tax</div>
              <div className="col-span-3">GL Account</div>
              <div className="col-span-1"></div>
            </div>
            {form.lines.map((line, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-4">
                  <input className="input text-sm py-1.5" placeholder="Description" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} required />
                </div>
                <div className="col-span-1">
                  <input className="input text-sm py-1.5 text-center" type="number" min="0" step="0.01" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} />
                </div>
                <div className="col-span-2">
                  <input className="input text-sm py-1.5" type="number" min="0" step="0.01" value={line.unit_price} onChange={(e) => updateLine(i, 'unit_price', +e.target.value)} />
                </div>
                <div className="col-span-1">
                  <select className="input text-sm py-1.5" value={line.tax_rate} onChange={(e) => updateLine(i, 'tax_rate', +e.target.value)}>
                    <option value={16}>16%</option>
                    <option value={0}>0%</option>
                  </select>
                </div>
                <div className="col-span-3">
                  <select className="input text-sm py-1.5" value={line.account_code} onChange={(e) => updateLine(i, 'account_code', e.target.value)}>
                    <option value="">Default</option>
                    {accounts.map(a => <option key={a.id} value={a.code}>{a.code} — {a.name}</option>)}
                  </select>
                </div>
                <div className="col-span-1 text-center">
                  <button type="button" onClick={() => removeLine(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={form.lines.length === 1}>×</button>
                </div>
              </div>
            ))}
          </div>
          <button type="button" onClick={addLine} className="mt-2 text-sm font-medium text-blue-600 hover:text-blue-800">+ Add a Line</button>
        </div>

        <div className="flex justify-end">
          <div className="bg-gray-50 rounded-lg p-4 space-y-2 w-64">
            <div className="flex justify-between text-sm"><span className="text-gray-600">Subtotal</span><span className="font-medium">{formatCurrency(subtotal)}</span></div>
            <div className="flex justify-between text-sm"><span className="text-gray-600">VAT</span><span>{formatCurrency(totalTax)}</span></div>
            <div className="border-t pt-2 flex justify-between text-base font-bold"><span>Total</span><span>{formatCurrency(grandTotal)}</span></div>
          </div>
        </div>

        <div className="flex items-center justify-between pt-4 border-t gap-3">
          <p className="text-xs text-gray-400 flex items-center gap-1"><FileMinus className="w-3.5 h-3.5" /> Posts a reversing journal entry on save.</p>
          <div className="flex gap-3">
            <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
            <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.vendor_id || !form.reason}>
              {mutation.isPending ? 'Saving...' : 'Create Credit Note'}
            </button>
          </div>
        </div>
      </form>
    </Modal>
  );
}
