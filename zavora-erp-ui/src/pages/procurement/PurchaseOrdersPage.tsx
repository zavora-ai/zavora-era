import { useState } from 'react';
import { useToast } from '../../components/toast/ToastProvider';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPurchaseOrders, getPurchaseOrder, getPurchaseOrderPdf, createPurchaseOrder, getVendors, getPoMatch, getGoodsReceipts, createGoodsReceipt, sendPurchaseOrder } from '../../api/client';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { hasRole, ROLES_CREATE } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Eye, Download, Plus, PackageCheck, Mail } from 'lucide-react';

/** Download the legal LPO document (PDF). */
async function downloadPoPdf(id: string, number: string) {
  const r = await getPurchaseOrderPdf(id);
  const url = URL.createObjectURL(new Blob([r.data], { type: 'application/pdf' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = `${number}.pdf`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

interface PurchaseOrder {
  id: string; number: string; vendor_id: string; currency: string; fx_rate: string;
  subtotal: string; tax_total: string; gross_total: string; status: string;
  issue_date: string; delivery_date?: string; notes?: string;
}
interface POLine {
  id: string; description: string; quantity: string; uom: string; unit_price: string; line_total: string;
}

export default function PurchaseOrdersPage() {
  const [detailId, setDetailId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const { data: pos = [], isLoading } = useQuery<PurchaseOrder[]>({
    queryKey: ['purchase-orders'],
    queryFn: () => getPurchaseOrders().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });
  const { data: vendors = [] } = useQuery<any[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then((r) => (Array.isArray(r.data) ? r.data : [])) });
  const vendorName = (id: string) => vendors.find((v) => v.id === id)?.name ?? `${id.slice(0, 8)}…`;

  const columns: Column<PurchaseOrder>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status.replace('_', ' ')}</span> },
    { key: 'number', header: 'LPO #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'vendor_id', header: 'Vendor', render: (r) => <span className="text-gray-900">{vendorName(r.vendor_id)}</span> },
    { key: 'issue_date', header: 'Issued', render: (r) => formatDate(r.issue_date) },
    { key: 'delivery_date', header: 'Delivery', render: (r) => (r.delivery_date ? formatDate(r.delivery_date) : '—') },
    { key: 'gross_total', header: 'Amount', className: 'text-right', render: (r) => <span className="font-medium">{formatCurrency(r.gross_total, r.currency)}</span> },
    {
      key: 'actions', header: '',
      render: (r) => (
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          <button onClick={() => setDetailId(r.id)} className="btn-secondary text-xs py-1 px-2" title="View LPO"><Eye className="w-3 h-3" /></button>
        </div>
      ),
    },
  ];

  return (
    <div>
      <PageHeader
        title="Purchase Orders (LPO)"
        subtitle="Raised from awarded tenders, or directly against a vendor. Vendors lodge invoices from the portal; for off-portal vendors, staff enter the bill on the AP side."
        actions={hasRole(ROLES_CREATE) ? (
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> New Purchase Order
          </button>
        ) : undefined}
      />
      <DataTable columns={columns} data={pos} loading={isLoading} onRowClick={(r) => setDetailId(r.id)} emptyMessage="No purchase orders yet. Award a tender or raise one directly." />
      {detailId && <PODetailModal id={detailId} vendorName={vendorName} onClose={() => setDetailId(null)} />}
      {showCreate && <CreatePOModal vendors={vendors} onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreatePOModal({ vendors, onClose }: { vendors: any[]; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [vendorId, setVendorId] = useState('');
  const [deliveryDate, setDeliveryDate] = useState('');
  const [notes, setNotes] = useState('');
  const [lines, setLines] = useState([{ description: '', quantity: 1, uom: 'unit', unit_price: 0 }]);
  const [formError, setFormError] = useState<string | null>(null);

  const vendorCurrency = vendors.find((v) => v.id === vendorId)?.currency ?? 'KES';
  const total = lines.reduce((s, l) => s + (Number(l.quantity) || 0) * (Number(l.unit_price) || 0), 0);

  const addLine = () => setLines([...lines, { description: '', quantity: 1, uom: 'unit', unit_price: 0 }]);
  const updateLine = (i: number, field: string, value: any) => {
    const next = [...lines];
    (next[i] as any)[field] = value;
    setLines(next);
  };
  const removeLine = (i: number) => { if (lines.length === 1) return; setLines(lines.filter((_, idx) => idx !== i)); };

  const mutation = useMutation({
    mutationFn: () => createPurchaseOrder({
      vendor_id: vendorId,
      currency: vendorCurrency,
      delivery_date: deliveryDate || undefined,
      notes: notes || undefined,
      lines: lines.filter((l) => l.description.trim()).map((l) => ({
        description: l.description, quantity: Number(l.quantity), uom: l.uom || 'unit', unit_price: Number(l.unit_price),
      })),
    }),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['purchase-orders'] }); onClose(); },
    onError: (e: unknown) => {
      const d = (e as { response?: { data?: { error?: string; message?: string } } })?.response?.data;
      setFormError(d?.error || d?.message || 'Could not create the purchase order.');
    },
  });

  const submit = () => {
    setFormError(null);
    if (!vendorId) { setFormError('Select a vendor.'); return; }
    if (!lines.some((l) => l.description.trim() && Number(l.unit_price) > 0)) {
      setFormError('Add at least one line with a description and unit price.'); return;
    }
    mutation.mutate();
  };

  return (
    <Modal open={true} onClose={onClose} title="New Purchase Order" subtitle="Direct procurement — raise an LPO against any vendor, no tender required." size="lg">
      <form onSubmit={(e) => { e.preventDefault(); submit(); }} className="space-y-5">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <div>
            <label className="label">Vendor *</label>
            <select className="input" value={vendorId} onChange={(e) => setVendorId(e.target.value)} required>
              <option value="">Select a vendor…</option>
              {vendors.map((v) => <option key={v.id} value={v.id}>{v.name}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Delivery date</label>
            <input type="date" className="input" value={deliveryDate} onChange={(e) => setDeliveryDate(e.target.value)} />
          </div>
        </div>

        <div>
          <label className="label">Items</label>
          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-5">Description</div>
              <div className="col-span-2">Qty</div>
              <div className="col-span-2">Unit</div>
              <div className="col-span-2 text-right">Unit Price</div>
              <div className="col-span-1"></div>
            </div>
            {lines.map((line, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-5"><input className="input text-sm py-1.5" placeholder="Item / service" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} /></div>
                <div className="col-span-2"><input className="input text-sm py-1.5 text-center" type="number" min="0" step="0.01" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} /></div>
                <div className="col-span-2"><input className="input text-sm py-1.5" value={line.uom} onChange={(e) => updateLine(i, 'uom', e.target.value)} /></div>
                <div className="col-span-2"><input className="input text-sm py-1.5 text-right" type="number" min="0" step="0.01" value={line.unit_price} onChange={(e) => updateLine(i, 'unit_price', +e.target.value)} /></div>
                <div className="col-span-1 text-center"><button type="button" onClick={() => removeLine(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={lines.length === 1}>×</button></div>
              </div>
            ))}
          </div>
          <button type="button" onClick={addLine} className="mt-2 text-sm font-medium text-blue-600 hover:text-blue-800">+ Add a Line</button>
        </div>

        <div className="flex justify-between items-start">
          <div className="flex-1 mr-4">
            <label className="label">Notes</label>
            <input className="input" value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Delivery terms, reference…" />
          </div>
          <div className="text-right">
            <p className="text-xs text-gray-500">Order total ({vendorCurrency})</p>
            <p className="text-xl font-bold text-gray-900">{formatCurrency(total, vendorCurrency)}</p>
          </div>
        </div>

        {formError && <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">{formError}</div>}

        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Raising…' : 'Raise LPO'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

interface MatchLine { description: string; ordered_qty: string; received_qty: string; billed_qty: string; po_unit_price: string; billed_unit_price: string; status: string; note?: string; }

function matchBadge(status: string) {
  if (status === 'matched') return 'inline-flex px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-100 text-emerald-700';
  if (status === 'over_billed') return 'inline-flex px-2 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-700';
  return 'inline-flex px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700';
}

function PODetailModal({ id, vendorName, onClose }: { id: string; vendorName: (id: string) => string; onClose: () => void }) {
  const { data } = useQuery({ queryKey: ['purchase-order', id], queryFn: () => getPurchaseOrder(id).then((r) => r.data) });
  const po: PurchaseOrder | undefined = data?.purchase_order;
  const lines: POLine[] = data?.lines ?? [];
  const [receiving, setReceiving] = useState(false);
  const toast = useToast();
  const sendMutation = useMutation({
    mutationFn: (email: string) => sendPurchaseOrder(id, email ? { recipient_email: email } : {}),
    onSuccess: (r) => toast.success(r.data?.sent_to ? `LPO emailed to ${r.data.sent_to}` : 'No email on file for this vendor — the send was recorded.'),
    onError: () => toast.error('Could not send the LPO.'),
  });
  const { data: match } = useQuery({ queryKey: ['po-match', id], queryFn: () => getPoMatch(id).then((r) => r.data) });
  const { data: grns = [] } = useQuery<any[]>({ queryKey: ['po-grns', id], queryFn: () => getGoodsReceipts(id).then((r) => (Array.isArray(r.data) ? r.data : [])) });
  const matchLines: MatchLine[] = match?.lines ?? [];

  return (
    <Modal open={true} onClose={onClose} title={po ? `Purchase Order ${po.number}` : 'Purchase Order'} size="lg">
      {!po ? (
        <p className="text-sm text-gray-500 py-8 text-center">Loading…</p>
      ) : (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div><span className="text-gray-500">Vendor</span><p className="font-medium text-gray-900">{vendorName(po.vendor_id)}</p></div>
            <div><span className="text-gray-500">Status</span><p><span className={statusColor(po.status)}>{po.status.replace('_', ' ')}</span></p></div>
            <div><span className="text-gray-500">Issued</span><p>{formatDate(po.issue_date)}</p></div>
            <div><span className="text-gray-500">Delivery</span><p>{po.delivery_date ? formatDate(po.delivery_date) : '—'}</p></div>
          </div>

          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-6">Description</div>
              <div className="col-span-2 text-center">Qty</div>
              <div className="col-span-2 text-right">Unit Price</div>
              <div className="col-span-2 text-right">Total</div>
            </div>
            {lines.map((l) => (
              <div key={l.id} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center text-sm">
                <div className="col-span-6 text-gray-900">{l.description}</div>
                <div className="col-span-2 text-center">{Number(l.quantity)} {l.uom}</div>
                <div className="col-span-2 text-right">{formatCurrency(l.unit_price, po.currency)}</div>
                <div className="col-span-2 text-right font-medium">{formatCurrency(l.line_total, po.currency)}</div>
              </div>
            ))}
          </div>

          <div className="flex justify-end">
            <div className="w-64 bg-gray-50 rounded-lg p-4 space-y-1 text-sm">
              <div className="flex justify-between"><span className="text-gray-600">Subtotal</span><span>{formatCurrency(po.subtotal, po.currency)}</span></div>
              <div className="flex justify-between"><span className="text-gray-600">Tax</span><span>{formatCurrency(po.tax_total, po.currency)}</span></div>
              <div className="border-t pt-1 mt-1 flex justify-between font-bold"><span>Total</span><span>{formatCurrency(po.gross_total, po.currency)}</span></div>
            </div>
          </div>

          {po.notes && <p className="text-sm text-gray-500">Notes: {po.notes}</p>}

          {/* 3-way match: ordered (PO) vs received (GRN) vs billed (invoices) */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <h4 className="text-sm font-semibold text-gray-700">3-way match</h4>
              {match && (
                <span className={match.matched ? 'text-xs font-medium text-emerald-600' : 'text-xs font-medium text-red-600'}>
                  {match.matched ? '✓ Matched' : '⚠ Exceptions'}
                </span>
              )}
            </div>
            <div className="border rounded-lg overflow-hidden">
              <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
                <div className="col-span-5">Description</div>
                <div className="col-span-2 text-center">Ordered</div>
                <div className="col-span-2 text-center">Received</div>
                <div className="col-span-1 text-center">Billed</div>
                <div className="col-span-2 text-right">Status</div>
              </div>
              {matchLines.map((l, i) => (
                <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center text-sm" title={l.note || ''}>
                  <div className="col-span-5 text-gray-900">{l.description}</div>
                  <div className="col-span-2 text-center">{Number(l.ordered_qty)}</div>
                  <div className="col-span-2 text-center font-medium">{Number(l.received_qty)}</div>
                  <div className="col-span-1 text-center">{Number(l.billed_qty)}</div>
                  <div className="col-span-2 text-right"><span className={matchBadge(l.status)}>{l.status.replace('_', ' ')}</span></div>
                </div>
              ))}
            </div>
            {grns.length > 0 && (
              <p className="mt-1 text-xs text-gray-500">Receipts: {grns.map((g) => `${g.number} (${formatDate(g.receipt_date)})`).join(', ')}</p>
            )}
          </div>

          <div className="flex justify-between items-center pt-3 border-t">
            <button type="button" onClick={() => downloadPoPdf(po.id, po.number)} className="btn-secondary">
              <Download className="w-4 h-4" /> Download PDF
            </button>
            <div className="flex gap-2">
              {hasRole(ROLES_CREATE) && (
                <button type="button" disabled={sendMutation.isPending}
                  onClick={() => { const email = window.prompt('Send LPO to which email? (leave blank to use the vendor on file)') ?? ''; if (email !== null) sendMutation.mutate(email); }}
                  className="btn-secondary">
                  <Mail className="w-4 h-4" /> {sendMutation.isPending ? 'Sending…' : 'Email to vendor'}
                </button>
              )}
              {hasRole(ROLES_CREATE) && (
                <button type="button" onClick={() => setReceiving(true)} className="btn-primary bg-emerald-600 hover:bg-emerald-700">
                  <PackageCheck className="w-4 h-4" /> Receive goods
                </button>
              )}
              <button type="button" onClick={onClose} className="btn-secondary">Close</button>
            </div>
          </div>
          {receiving && <ReceiveGoodsModal poId={po.id} poNumber={po.number} lines={lines} onClose={() => setReceiving(false)} />}
        </div>
      )}
    </Modal>
  );
}

function ReceiveGoodsModal({ poId, poNumber, lines, onClose }: { poId: string; poNumber: string; lines: POLine[]; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [recv, setRecv] = useState<Record<string, number>>(() => Object.fromEntries(lines.map((l) => [l.id, Number(l.quantity)])));
  const [error, setError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => createGoodsReceipt(poId, {
      lines: lines.filter((l) => (recv[l.id] ?? 0) > 0).map((l) => ({ po_line_id: l.id, description: l.description, quantity_received: recv[l.id] })),
    }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['po-match', poId] });
      queryClient.invalidateQueries({ queryKey: ['po-grns', poId] });
      onClose();
    },
    onError: (e: unknown) => { const d = (e as any)?.response?.data; setError(d?.error || d?.message || 'Could not record the receipt.'); },
  });

  return (
    <Modal open={true} onClose={onClose} title={`Receive goods — ${poNumber}`} subtitle="Record what actually arrived. This is the receipt leg of the 3-way match." size="lg">
      <div className="space-y-4">
        <div className="border rounded-lg overflow-hidden">
          <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
            <div className="col-span-7">Description</div>
            <div className="col-span-2 text-center">Ordered</div>
            <div className="col-span-3 text-right">Received now</div>
          </div>
          {lines.map((l) => (
            <div key={l.id} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center text-sm">
              <div className="col-span-7 text-gray-900">{l.description}</div>
              <div className="col-span-2 text-center">{Number(l.quantity)} {l.uom}</div>
              <div className="col-span-3">
                <input type="number" min="0" step="0.01" className="input text-sm py-1.5 text-right" value={recv[l.id] ?? 0} onChange={(e) => setRecv({ ...recv, [l.id]: +e.target.value })} />
              </div>
            </div>
          ))}
        </div>
        {error && <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">{error}</div>}
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="button" onClick={() => { setError(null); mutation.mutate(); }} className="btn-primary bg-emerald-600 hover:bg-emerald-700" disabled={mutation.isPending}>
            {mutation.isPending ? 'Recording…' : 'Record receipt'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
