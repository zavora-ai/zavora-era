import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getDebitNotes, createDebitNote, getVendors } from '../../api/client';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { usePermissions } from '../../hooks/usePermissions';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus } from 'lucide-react';

interface DebitNote { id: string; number: string; vendor_id: string; reason?: string; currency: string; gross_total: string; status: string; debit_note_date: string; }

export default function DebitNotesPage() {
  const { can } = usePermissions();
  const [showCreate, setShowCreate] = useState(false);
  const { data: notes = [], isLoading } = useQuery<DebitNote[]>({ queryKey: ['debit-notes'], queryFn: () => getDebitNotes().then((r) => (Array.isArray(r.data) ? r.data : [])) });
  const { data: vendors = [] } = useQuery<any[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then((r) => (Array.isArray(r.data) ? r.data : [])) });
  const vName = (id: string) => vendors.find((v) => v.id === id)?.name ?? `${id.slice(0, 8)}…`;

  const columns: Column<DebitNote>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status}</span> },
    { key: 'number', header: 'DN #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'vendor_id', header: 'Vendor', render: (r) => vName(r.vendor_id) },
    { key: 'reason', header: 'Reason', render: (r) => r.reason || '—' },
    { key: 'debit_note_date', header: 'Date', render: (r) => formatDate(r.debit_note_date) },
    { key: 'gross_total', header: 'Amount', className: 'text-right', render: (r) => <span className="font-medium">{formatCurrency(r.gross_total, r.currency)}</span> },
  ];

  return (
    <div>
      <PageHeader title="Debit Notes" subtitle="Supplier returns and overcharge claims. Issuing a debit note reduces what you owe the vendor."
        actions={can('debit_note.create') ? <button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> New Debit Note</button> : undefined} />
      <DataTable columns={columns} data={notes} loading={isLoading} emptyMessage="No debit notes yet." />
      {showCreate && <CreateDNModal vendors={vendors} onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateDNModal({ vendors, onClose }: { vendors: any[]; onClose: () => void }) {
  const qc = useQueryClient();
  const [vendorId, setVendorId] = useState('');
  const [reason, setReason] = useState('');
  const [lines, setLines] = useState([{ description: '', quantity: 1, unit_price: 0, account_code: '' }]);
  const [error, setError] = useState<string | null>(null);
  const total = lines.reduce((s, l) => s + (Number(l.quantity) || 0) * (Number(l.unit_price) || 0), 0);
  const add = () => setLines([...lines, { description: '', quantity: 1, unit_price: 0, account_code: '' }]);
  const upd = (i: number, f: string, v: any) => { const n = [...lines]; (n[i] as any)[f] = v; setLines(n); };
  const rm = (i: number) => { if (lines.length === 1) return; setLines(lines.filter((_, idx) => idx !== i)); };

  const mut = useMutation({
    mutationFn: () => createDebitNote({ vendor_id: vendorId, reason: reason || undefined, lines: lines.filter((l) => l.description.trim()).map((l) => ({ description: l.description, quantity: Number(l.quantity), unit_price: Number(l.unit_price), account_code: l.account_code || undefined })) }),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['debit-notes'] }); onClose(); },
    onError: (e: any) => setError(e?.response?.data?.error || 'Could not create the debit note.'),
  });
  const submit = () => { setError(null); if (!vendorId) return setError('Select a vendor.'); if (!lines.some((l) => l.description.trim() && Number(l.unit_price) > 0)) return setError('Add at least one line with an amount.'); mut.mutate(); };

  return (
    <Modal open={true} onClose={onClose} title="New Debit Note" subtitle="Reduce the payable to a vendor for a return or overcharge." size="lg">
      <form onSubmit={(e) => { e.preventDefault(); submit(); }} className="space-y-5">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <div><label className="label">Vendor *</label><select className="input" value={vendorId} onChange={(e) => setVendorId(e.target.value)} required><option value="">Select a vendor…</option>{vendors.map((v) => <option key={v.id} value={v.id}>{v.name}</option>)}</select></div>
          <div><label className="label">Reason</label><input className="input" value={reason} onChange={(e) => setReason(e.target.value)} placeholder="e.g. Returned faulty goods" /></div>
        </div>
        <div>
          <label className="label">Items</label>
          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase"><div className="col-span-5">Description</div><div className="col-span-2">Qty</div><div className="col-span-2">Account</div><div className="col-span-2 text-right">Unit Price</div></div>
            {lines.map((l, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-5"><input className="input text-sm py-1.5" placeholder="Item returned" value={l.description} onChange={(e) => upd(i, 'description', e.target.value)} /></div>
                <div className="col-span-2"><input type="number" min="0" step="0.01" className="input text-sm py-1.5 text-center" value={l.quantity} onChange={(e) => upd(i, 'quantity', +e.target.value)} /></div>
                <div className="col-span-2"><input className="input text-sm py-1.5" placeholder="acct" value={l.account_code} onChange={(e) => upd(i, 'account_code', e.target.value)} /></div>
                <div className="col-span-2"><input type="number" min="0" step="0.01" className="input text-sm py-1.5 text-right" value={l.unit_price} onChange={(e) => upd(i, 'unit_price', +e.target.value)} /></div>
                <div className="col-span-1 text-center"><button type="button" onClick={() => rm(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={lines.length === 1}>×</button></div>
              </div>
            ))}
          </div>
          <div className="flex justify-between items-center mt-2"><button type="button" onClick={add} className="text-sm font-medium text-blue-600 hover:text-blue-800">+ Add a Line</button><span className="font-bold">{formatCurrency(total, 'KES')}</span></div>
        </div>
        {error && <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">{error}</div>}
        <div className="flex justify-end pt-4 border-t gap-3"><button type="button" onClick={onClose} className="btn-secondary">Cancel</button><button type="submit" className="btn-primary" disabled={mut.isPending}>{mut.isPending ? 'Issuing…' : 'Issue Debit Note'}</button></div>
      </form>
    </Modal>
  );
}
