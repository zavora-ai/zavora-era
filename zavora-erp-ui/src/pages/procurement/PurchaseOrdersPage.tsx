import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getPurchaseOrders, getPurchaseOrder, getVendors } from '../../api/client';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Eye } from 'lucide-react';

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
        subtitle="Local purchase orders raised from awarded tenders. Vendors lodge invoices against these from the portal."
      />
      <DataTable columns={columns} data={pos} loading={isLoading} onRowClick={(r) => setDetailId(r.id)} emptyMessage="No purchase orders yet. Award a tender to raise one." />
      {detailId && <PODetailModal id={detailId} vendorName={vendorName} onClose={() => setDetailId(null)} />}
    </div>
  );
}

function PODetailModal({ id, vendorName, onClose }: { id: string; vendorName: (id: string) => string; onClose: () => void }) {
  const { data } = useQuery({ queryKey: ['purchase-order', id], queryFn: () => getPurchaseOrder(id).then((r) => r.data) });
  const po: PurchaseOrder | undefined = data?.purchase_order;
  const lines: POLine[] = data?.lines ?? [];

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

          <div className="flex justify-end pt-3 border-t">
            <button type="button" onClick={onClose} className="btn-secondary">Close</button>
          </div>
        </div>
      )}
    </Modal>
  );
}
