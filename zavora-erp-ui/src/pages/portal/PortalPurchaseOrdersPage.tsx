import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPortalPurchaseOrders, getPortalPurchaseOrder, getPortalPurchaseOrderPdf, lodgePortalInvoice } from '../../api/portalClient';
import { workToday } from '../../utils/workDate';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import Modal from '../../components/shared/Modal';
import { ShoppingCart, FileUp, Download } from 'lucide-react';

/** Open the LPO PDF (the legal document) in a new tab. */
async function openPoPdf(id: string, number: string) {
  const r = await getPortalPurchaseOrderPdf(id);
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
  id: string; number: string; currency: string; subtotal: string; tax_total: string;
  gross_total: string; status: string; issue_date: string; delivery_date?: string; notes?: string;
}
interface POLine { id: string; description: string; quantity: string; uom: string; unit_price: string; line_total: string; }

const canInvoice = (s: string) => s === 'issued' || s === 'acknowledged' || s === 'partially_invoiced';

export default function PortalPurchaseOrdersPage() {
  const [lodgeFor, setLodgeFor] = useState<PurchaseOrder | null>(null);

  const { data: pos = [], isLoading } = useQuery<PurchaseOrder[]>({
    queryKey: ['portal-purchase-orders'],
    queryFn: () => getPortalPurchaseOrders().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900">Purchase Orders</h1>
        <p className="mt-1 text-sm text-gray-500">Orders awarded to you. Lodge an invoice against a PO to get paid.</p>
      </div>

      {isLoading ? (
        <p className="text-sm text-gray-500 py-12 text-center">Loading…</p>
      ) : pos.length === 0 ? (
        <div className="bg-white rounded-xl border border-gray-200 p-12 text-center">
          <ShoppingCart className="w-10 h-10 text-gray-300 mx-auto mb-3" />
          <p className="text-gray-500">No purchase orders yet. Win a tender to receive one.</p>
        </div>
      ) : (
        <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
                <th className="text-left px-4 py-3">LPO #</th>
                <th className="text-left px-4 py-3">Status</th>
                <th className="text-left px-4 py-3">Issued</th>
                <th className="text-left px-4 py-3">Delivery</th>
                <th className="text-right px-4 py-3">Amount</th>
                <th className="text-right px-4 py-3"></th>
              </tr>
            </thead>
            <tbody>
              {pos.map((po) => (
                <tr key={po.id} className="border-b last:border-b-0">
                  <td className="px-4 py-3 font-medium text-blue-600">{po.number}</td>
                  <td className="px-4 py-3"><span className={statusColor(po.status)}>{po.status.replace('_', ' ')}</span></td>
                  <td className="px-4 py-3 text-gray-600">{formatDate(po.issue_date)}</td>
                  <td className="px-4 py-3 text-gray-600">{po.delivery_date ? formatDate(po.delivery_date) : '—'}</td>
                  <td className="px-4 py-3 text-right font-medium">{formatCurrency(po.gross_total, po.currency)}</td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-2">
                      <button onClick={() => openPoPdf(po.id, po.number)} className="btn-secondary text-xs py-1 px-2" title="Download LPO (PDF)">
                        <Download className="w-3 h-3" /> PDF
                      </button>
                      {canInvoice(po.status) ? (
                        <button onClick={() => setLodgeFor(po)} className="btn-primary text-xs py-1 px-2 bg-emerald-600 hover:bg-emerald-700">
                          <FileUp className="w-3 h-3" /> Lodge invoice
                        </button>
                      ) : (
                        <span className="text-xs text-gray-400">Invoiced</span>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {lodgeFor && <LodgeInvoiceModal po={lodgeFor} onClose={() => setLodgeFor(null)} />}
    </div>
  );
}

function LodgeInvoiceModal({ po, onClose }: { po: PurchaseOrder; onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data } = useQuery({ queryKey: ['portal-po', po.id], queryFn: () => getPortalPurchaseOrder(po.id).then((r) => r.data) });
  const lines: POLine[] = data?.lines ?? [];

  const [invoiceNumber, setInvoiceNumber] = useState('');
  const [issueDate, setIssueDate] = useState(workToday());
  const [notes, setNotes] = useState('');
  const [etimsFile, setEtimsFile] = useState<File | null>(null);
  const [formError, setFormError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => lodgePortalInvoice(po.id, {
      vendor_invoice_number: invoiceNumber.trim(),
      issue_date: issueDate || undefined,
      notes: notes || undefined,
      etims_file: etimsFile as File,
      // The backend bills the LPO lines as-is.
    }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['portal-purchase-orders'] });
      queryClient.invalidateQueries({ queryKey: ['portal-statement'] });
      queryClient.invalidateQueries({ queryKey: ['portal-invoices'] });
      onClose();
    },
    onError: (e: unknown) => {
      const msg = (e as { response?: { data?: { error?: string; message?: string } } })?.response?.data;
      setFormError(msg?.error || msg?.message || 'Could not lodge the invoice. Please try again.');
    },
  });

  const submit = () => {
    setFormError(null);
    if (!invoiceNumber.trim()) { setFormError('The eTIMS invoice number is required.'); return; }
    if (!etimsFile) { setFormError('Attach your eTIMS invoice (PDF, JPG or PNG) — it is mandatory.'); return; }
    mutation.mutate();
  };

  return (
    <Modal open={true} onClose={onClose} title={`Lodge invoice — ${po.number}`} subtitle="This submits an invoice to the buyer for approval and payment." size="lg">
      <form onSubmit={(e) => { e.preventDefault(); submit(); }} className="space-y-5">
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="label">eTIMS invoice number <span className="text-red-500">*</span></label>
            <input className="input" value={invoiceNumber} onChange={(e) => setInvoiceNumber(e.target.value)} placeholder="e.g. 0100…KRA control no." required />
          </div>
          <div>
            <label className="label">Invoice date</label>
            <input type="date" className="input" value={issueDate} onChange={(e) => setIssueDate(e.target.value)} />
          </div>
        </div>

        {/* Mandatory eTIMS invoice attachment. */}
        <div>
          <label className="label">eTIMS tax invoice <span className="text-red-500">*</span></label>
          <input
            type="file"
            accept="application/pdf,image/jpeg,image/png,image/webp"
            onChange={(e) => setEtimsFile(e.target.files?.[0] ?? null)}
            className="block w-full text-sm text-gray-600 file:mr-3 file:py-2 file:px-4 file:rounded-lg file:border-0 file:text-sm file:font-medium file:bg-emerald-50 file:text-emerald-700 hover:file:bg-emerald-100"
            required
          />
          <p className="mt-1 text-xs text-gray-500">
            {etimsFile ? `Selected: ${etimsFile.name}` : 'Attach the eTIMS-generated tax invoice (PDF or a clear photo/scan). Required — the buyer cannot approve payment without it.'}
          </p>
        </div>

        {/* PO lines being invoiced (as-is) */}
        <div className="border rounded-lg overflow-hidden">
          <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
            <div className="col-span-6">Item</div>
            <div className="col-span-2 text-center">Qty</div>
            <div className="col-span-2 text-right">Unit Price</div>
            <div className="col-span-2 text-right">Amount</div>
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

        <div className="flex justify-between items-start">
          <div className="flex-1 mr-4">
            <label className="label">Notes</label>
            <input className="input" value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Optional" />
          </div>
          <div className="text-right">
            <p className="text-xs text-gray-500">PO total</p>
            <p className="text-xl font-bold text-gray-900">{formatCurrency(po.gross_total, po.currency)}</p>
          </div>
        </div>

        {formError && (
          <div className="rounded-lg bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700">{formError}</div>
        )}

        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary bg-emerald-600 hover:bg-emerald-700" disabled={mutation.isPending}>
            {mutation.isPending ? 'Lodging…' : 'Lodge invoice'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
