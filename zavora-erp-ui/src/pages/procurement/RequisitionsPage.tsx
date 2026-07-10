import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import {
  getRequisitions, getRequisition, createRequisition, submitRequisition,
  approveRequisition, rejectRequisition, convertRequisition, getVendors,
} from '../../api/client';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { usePermissions } from '../../hooks/usePermissions';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import DepartmentSelect from '../../components/shared/DepartmentSelect';
import { Plus, Send, Check, X, ArrowRightLeft } from 'lucide-react';

interface Requisition {
  id: string; number: string; title: string; department?: string; currency: string;
  needed_by?: string; estimated_total: string; status: string;
  converted_to_type?: string; converted_to_id?: string; rejection_reason?: string; created_at: string;
}
interface PRLine { id: string; description: string; quantity: string; uom: string; estimated_unit_price: string; line_total: string; }

export default function RequisitionsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const { can } = usePermissions();
  const [detailId, setDetailId] = useState<string | null>(null);

  const { data: prs = [], isLoading } = useQuery<Requisition[]>({
    queryKey: ['requisitions'],
    queryFn: () => getRequisitions().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });

  const columns: Column<Requisition>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status.replace('_', ' ')}</span> },
    { key: 'number', header: 'PR #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'title', header: 'Title', render: (r) => <span className="text-gray-900">{r.title}</span> },
    { key: 'department', header: 'Department', render: (r) => r.department || '—' },
    { key: 'needed_by', header: 'Needed By', render: (r) => (r.needed_by ? formatDate(r.needed_by) : '—') },
    { key: 'estimated_total', header: 'Est. Total', className: 'text-right', render: (r) => <span className="font-medium">{formatCurrency(r.estimated_total, r.currency)}</span> },
  ];

  return (
    <div>
      <PageHeader
        title="Purchase Requisitions"
        subtitle="Raise an internal request to buy. Once approved, a buyer converts it into a tender or a purchase order."
        actions={can('requisition.create') ? (
          <button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> New Requisition</button>
        ) : undefined}
      />
      <DataTable columns={columns} data={prs} loading={isLoading} onRowClick={(r) => setDetailId(r.id)} emptyMessage="No requisitions yet. Raise one to request a purchase." />
      {showCreate && <CreatePRModal onClose={() => setShowCreate(false)} />}
      {detailId && <PRDetailModal id={detailId} onClose={() => setDetailId(null)} />}
    </div>
  );
}

function CreatePRModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ title: '', justification: '', department: '', needed_by: '' });
  const [lines, setLines] = useState([{ description: '', quantity: 1, uom: 'unit', estimated_unit_price: 0 }]);
  const [error, setError] = useState<string | null>(null);

  const total = lines.reduce((s, l) => s + (Number(l.quantity) || 0) * (Number(l.estimated_unit_price) || 0), 0);
  const addLine = () => setLines([...lines, { description: '', quantity: 1, uom: 'unit', estimated_unit_price: 0 }]);
  const updateLine = (i: number, f: string, v: any) => { const n = [...lines]; (n[i] as any)[f] = v; setLines(n); };
  const removeLine = (i: number) => { if (lines.length === 1) return; setLines(lines.filter((_, idx) => idx !== i)); };

  const mutation = useMutation({
    mutationFn: () => createRequisition({
      title: form.title, justification: form.justification || undefined, department: form.department || undefined,
      needed_by: form.needed_by || undefined,
      lines: lines.filter((l) => l.description.trim()).map((l) => ({
        description: l.description, quantity: Number(l.quantity), uom: l.uom || 'unit', estimated_unit_price: Number(l.estimated_unit_price),
      })),
    }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['requisitions'] }); onClose(); },
    onError: (e: unknown) => { const d = (e as any)?.response?.data; setError(d?.error || d?.message || 'Could not create the requisition.'); },
  });

  const submit = () => {
    setError(null);
    if (!form.title.trim()) { setError('Enter a title.'); return; }
    if (!lines.some((l) => l.description.trim())) { setError('Add at least one line.'); return; }
    mutation.mutate();
  };

  return (
    <Modal open={true} onClose={onClose} title="New Requisition" subtitle="Request a purchase for approval." size="lg">
      <form onSubmit={(e) => { e.preventDefault(); submit(); }} className="space-y-5">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <div className="lg:col-span-2">
            <label className="label">Title *</label>
            <input className="input" value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} placeholder="e.g. Marketing laptops — Q4" required />
          </div>
          <div>
            <label className="label">Department</label>
            <DepartmentSelect byName value={form.department} onChange={(_, name) => setForm({ ...form, department: name })} />
          </div>
          <div>
            <label className="label">Needed by</label>
            <input type="date" className="input" value={form.needed_by} onChange={(e) => setForm({ ...form, needed_by: e.target.value })} />
          </div>
          <div className="lg:col-span-2">
            <label className="label">Justification</label>
            <textarea className="input" rows={2} value={form.justification} onChange={(e) => setForm({ ...form, justification: e.target.value })} placeholder="Business case / why it's needed" />
          </div>
        </div>

        <div>
          <label className="label">Items</label>
          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-5">Description</div>
              <div className="col-span-2">Qty</div>
              <div className="col-span-2">Unit</div>
              <div className="col-span-2 text-right">Est. Price</div>
              <div className="col-span-1"></div>
            </div>
            {lines.map((line, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-5"><input className="input text-sm py-1.5" placeholder="Item / service" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} /></div>
                <div className="col-span-2"><input className="input text-sm py-1.5 text-center" type="number" min="0" step="0.01" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} /></div>
                <div className="col-span-2"><input className="input text-sm py-1.5" value={line.uom} onChange={(e) => updateLine(i, 'uom', e.target.value)} /></div>
                <div className="col-span-2"><input className="input text-sm py-1.5 text-right" type="number" min="0" step="0.01" value={line.estimated_unit_price} onChange={(e) => updateLine(i, 'estimated_unit_price', +e.target.value)} /></div>
                <div className="col-span-1 text-center"><button type="button" onClick={() => removeLine(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={lines.length === 1}>×</button></div>
              </div>
            ))}
          </div>
          <div className="flex justify-between items-center mt-2">
            <button type="button" onClick={addLine} className="text-sm font-medium text-blue-600 hover:text-blue-800">+ Add a Line</button>
            <div className="text-right"><span className="text-xs text-gray-500 mr-2">Estimated total</span><span className="font-bold text-gray-900">{formatCurrency(total, 'KES')}</span></div>
          </div>
        </div>

        {error && <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">{error}</div>}
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>{mutation.isPending ? 'Saving…' : 'Create Requisition'}</button>
        </div>
      </form>
    </Modal>
  );
}

function PRDetailModal({ id, onClose }: { id: string; onClose: () => void }) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const { can } = usePermissions();
  const { data } = useQuery({ queryKey: ['requisition', id], queryFn: () => getRequisition(id).then((r) => r.data) });
  const pr: Requisition | undefined = data?.requisition;
  const lines: PRLine[] = data?.lines ?? [];
  const [converting, setConverting] = useState(false);

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: ['requisitions'] });
    queryClient.invalidateQueries({ queryKey: ['requisition', id] });
  };
  const act = useMutation({
    mutationFn: (fn: () => Promise<any>) => fn(),
    onSuccess: invalidate,
  });

  if (!pr) return <Modal open={true} onClose={onClose} title="Requisition"><p className="text-sm text-gray-500 py-8 text-center">Loading…</p></Modal>;

  return (
    <Modal open={true} onClose={onClose} title={`${pr.number} — ${pr.title}`} size="lg">
      <div className="space-y-4">
        <div className="grid grid-cols-3 gap-4 text-sm">
          <div><span className="text-gray-500">Status</span><p><span className={statusColor(pr.status)}>{pr.status.replace('_', ' ')}</span></p></div>
          <div><span className="text-gray-500">Department</span><p className="font-medium">{pr.department || '—'}</p></div>
          <div><span className="text-gray-500">Needed by</span><p>{pr.needed_by ? formatDate(pr.needed_by) : '—'}</p></div>
        </div>
        {pr.status === 'rejected' && pr.rejection_reason && (
          <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">Rejected: {pr.rejection_reason}</div>
        )}
        {pr.status === 'converted' && (
          <div className="rounded-lg bg-emerald-50 border border-emerald-200 px-3 py-2 text-sm text-emerald-700">
            Converted to {pr.converted_to_type === 'tender' ? 'a tender' : 'a purchase order'}.{' '}
            {pr.converted_to_type === 'tender'
              ? <button className="underline font-medium" onClick={() => { onClose(); navigate('/tenders'); }}>View tenders</button>
              : <button className="underline font-medium" onClick={() => { onClose(); navigate('/purchase-orders'); }}>View purchase orders</button>}
          </div>
        )}

        <div className="border rounded-lg overflow-hidden">
          <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
            <div className="col-span-6">Description</div><div className="col-span-2 text-center">Qty</div>
            <div className="col-span-2 text-right">Est. Price</div><div className="col-span-2 text-right">Total</div>
          </div>
          {lines.map((l) => (
            <div key={l.id} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center text-sm">
              <div className="col-span-6 text-gray-900">{l.description}</div>
              <div className="col-span-2 text-center">{Number(l.quantity)} {l.uom}</div>
              <div className="col-span-2 text-right">{formatCurrency(l.estimated_unit_price, pr.currency)}</div>
              <div className="col-span-2 text-right font-medium">{formatCurrency(l.line_total, pr.currency)}</div>
            </div>
          ))}
        </div>
        <div className="flex justify-end"><div className="text-right"><span className="text-xs text-gray-500 mr-2">Estimated total</span><span className="text-lg font-bold">{formatCurrency(pr.estimated_total, pr.currency)}</span></div></div>

        {/* Lifecycle actions */}
        <div className="flex items-center justify-between pt-3 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Close</button>
          <div className="flex gap-2">
            {pr.status === 'draft' && can('requisition.create') && (
              <button className="btn-primary" disabled={act.isPending} onClick={() => act.mutate(() => submitRequisition(id))}>
                <Send className="w-4 h-4" /> Submit for approval
              </button>
            )}
            {pr.status === 'submitted' && can('requisition.approve') && (
              <>
                <button className="btn-secondary text-red-600" disabled={act.isPending} onClick={() => { const reason = window.prompt('Reason for rejection?') ?? undefined; act.mutate(() => rejectRequisition(id, reason)); }}>
                  <X className="w-4 h-4" /> Reject
                </button>
                <button className="btn-primary bg-emerald-600 hover:bg-emerald-700" disabled={act.isPending} onClick={() => act.mutate(() => approveRequisition(id))}>
                  <Check className="w-4 h-4" /> Approve
                </button>
              </>
            )}
            {pr.status === 'approved' && can('requisition.convert') && (
              <button className="btn-primary" onClick={() => setConverting(true)}><ArrowRightLeft className="w-4 h-4" /> Convert</button>
            )}
          </div>
        </div>
      </div>
      {converting && <ConvertModal pr={pr} onDone={() => { setConverting(false); invalidate(); onClose(); }} onCancel={() => setConverting(false)} />}
    </Modal>
  );
}

function ConvertModal({ pr, onDone, onCancel }: { pr: Requisition; onDone: () => void; onCancel: () => void }) {
  const [target, setTarget] = useState<'purchase_order' | 'tender'>('purchase_order');
  const [vendorId, setVendorId] = useState('');
  const [deliveryDate, setDeliveryDate] = useState('');
  const [closingDate, setClosingDate] = useState('');
  const [error, setError] = useState<string | null>(null);
  const { data: vendors = [] } = useQuery<any[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then((r) => (Array.isArray(r.data) ? r.data : [])) });

  const mutation = useMutation({
    mutationFn: () => convertRequisition(pr.id, {
      target,
      vendor_id: target === 'purchase_order' ? vendorId : undefined,
      delivery_date: target === 'purchase_order' ? (deliveryDate || undefined) : undefined,
      closing_date: target === 'tender' ? (closingDate || undefined) : undefined,
    }),
    onSuccess: onDone,
    onError: (e: unknown) => { const d = (e as any)?.response?.data; setError(d?.error || d?.message || 'Conversion failed.'); },
  });

  const submit = () => {
    setError(null);
    if (target === 'purchase_order' && !vendorId) { setError('Select a vendor for the purchase order.'); return; }
    mutation.mutate();
  };

  return (
    <Modal open={true} onClose={onCancel} title={`Convert ${pr.number}`} subtitle="Turn this approved requisition into a sourcing document." size="md">
      <div className="space-y-4">
        <div className="grid grid-cols-2 gap-2">
          <button type="button" onClick={() => setTarget('purchase_order')} className={`rounded-lg border px-3 py-3 text-sm text-left ${target === 'purchase_order' ? 'border-blue-500 bg-blue-50' : 'border-gray-200'}`}>
            <p className="font-medium">Direct Purchase Order</p><p className="text-gray-500 text-xs mt-0.5">Buy now from a chosen vendor</p>
          </button>
          <button type="button" onClick={() => setTarget('tender')} className={`rounded-lg border px-3 py-3 text-sm text-left ${target === 'tender' ? 'border-blue-500 bg-blue-50' : 'border-gray-200'}`}>
            <p className="font-medium">Tender / RFQ</p><p className="text-gray-500 text-xs mt-0.5">Invite competitive bids</p>
          </button>
        </div>

        {target === 'purchase_order' ? (
          <>
            <div>
              <label className="label">Vendor *</label>
              <select className="input" value={vendorId} onChange={(e) => setVendorId(e.target.value)} required>
                <option value="">Select a vendor…</option>
                {vendors.map((v) => <option key={v.id} value={v.id}>{v.name}</option>)}
              </select>
            </div>
            <div><label className="label">Delivery date</label><input type="date" className="input" value={deliveryDate} onChange={(e) => setDeliveryDate(e.target.value)} /></div>
          </>
        ) : (
          <div><label className="label">Closing date</label><input type="date" className="input" value={closingDate} onChange={(e) => setClosingDate(e.target.value)} /></div>
        )}

        {error && <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">{error}</div>}
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onCancel} className="btn-secondary">Cancel</button>
          <button type="button" onClick={submit} className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Converting…' : target === 'purchase_order' ? 'Raise LPO' : 'Create Tender'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
