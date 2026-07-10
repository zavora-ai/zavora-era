import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getTenders, createTender, publishTender, getTenderBids, awardTender, getVendors,
} from '../../api/client';
import { workToday } from '../../utils/workDate';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { usePermissions } from '../../hooks/usePermissions';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, Send, Gavel, Award } from 'lucide-react';

interface Tender {
  id: string; number: string; title: string; description?: string; category?: string;
  closing_date?: string; status: string; created_at: string;
}
interface Bid {
  id: string; tender_id: string; vendor_id: string; currency: string;
  total_amount: string; notes?: string; status: string; submitted_at: string;
}

export default function TendersPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [bidsFor, setBidsFor] = useState<Tender | null>(null);
  const queryClient = useQueryClient();
  const { can } = usePermissions();

  const { data: tenders = [], isLoading } = useQuery<Tender[]>({
    queryKey: ['tenders'],
    queryFn: () => getTenders().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });

  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['tenders'] });
  const publishMut = useMutation({ mutationFn: (id: string) => publishTender(id), onSuccess: invalidate });

  const columns: Column<Tender>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status}</span> },
    { key: 'number', header: 'RFQ #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'title', header: 'Title', render: (r) => <span className="text-gray-900">{r.title}</span> },
    { key: 'category', header: 'Category', render: (r) => r.category || '—' },
    { key: 'closing_date', header: 'Closes', render: (r) => (r.closing_date ? formatDate(r.closing_date) : '—') },
    {
      key: 'actions', header: '',
      render: (r) => (
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          {r.status === 'draft' && can('tender.create') && (
            <button onClick={() => publishMut.mutate(r.id)} disabled={publishMut.isPending} className="btn-primary text-xs py-1 px-2" title="Open for bids">
              <Send className="w-3 h-3" /> Publish
            </button>
          )}
          {(r.status === 'open' || r.status === 'awarded' || r.status === 'closed') && (
            <button onClick={() => setBidsFor(r)} className="btn-secondary text-xs py-1 px-2" title="View bids">
              <Gavel className="w-3 h-3" /> Bids
            </button>
          )}
        </div>
      ),
    },
  ];

  return (
    <div>
      <PageHeader
        title="Tenders / RFQs"
        subtitle="Publish a request for quotation, collect vendor bids, and award the winner — which raises the LPO automatically."
        actions={can('tender.create') ? (
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> New Tender
          </button>
        ) : undefined}
      />
      <DataTable columns={columns} data={tenders} loading={isLoading} onRowClick={(r) => setBidsFor(r)} emptyMessage="No tenders yet. Create one to invite vendor bids." />

      {showCreate && <CreateTenderModal onClose={() => setShowCreate(false)} />}
      {bidsFor && <BidsModal tender={bidsFor} onClose={() => setBidsFor(null)} />}
    </div>
  );
}

function CreateTenderModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    title: '', description: '', category: '', closing_date: '',
    lines: [{ description: '', quantity: 1, uom: 'unit' }],
  });

  const mutation = useMutation({
    mutationFn: (data: any) => createTender(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['tenders'] }); onClose(); },
  });

  const addLine = () => setForm({ ...form, lines: [...form.lines, { description: '', quantity: 1, uom: 'unit' }] });
  const updateLine = (i: number, field: string, value: any) => {
    const lines = [...form.lines];
    (lines[i] as any)[field] = value;
    setForm({ ...form, lines });
  };
  const removeLine = (i: number) => { if (form.lines.length === 1) return; setForm({ ...form, lines: form.lines.filter((_, idx) => idx !== i) }); };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      title: form.title,
      description: form.description || undefined,
      category: form.category || undefined,
      closing_date: form.closing_date || undefined,
      lines: form.lines.filter((l) => l.description.trim()).map((l) => ({ description: l.description, quantity: l.quantity, uom: l.uom })),
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="New Tender" size="lg">
      <form onSubmit={handleSubmit} className="space-y-5">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <div className="lg:col-span-2">
            <label className="label">Title *</label>
            <input className="input" value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} placeholder="e.g. Office laptops — Q3" required />
          </div>
          <div>
            <label className="label">Category</label>
            <input className="input" value={form.category} onChange={(e) => setForm({ ...form, category: e.target.value })} placeholder="e.g. IT Equipment" />
          </div>
          <div>
            <label className="label">Closing Date</label>
            <input type="date" className="input" min={workToday()} value={form.closing_date} onChange={(e) => setForm({ ...form, closing_date: e.target.value })} />
          </div>
          <div className="lg:col-span-2">
            <label className="label">Description</label>
            <textarea className="input" rows={2} value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="Scope, delivery terms, evaluation criteria…" />
          </div>
        </div>

        <div>
          <label className="label">Requested Items</label>
          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-7">Description</div>
              <div className="col-span-2">Qty</div>
              <div className="col-span-2">Unit</div>
              <div className="col-span-1"></div>
            </div>
            {form.lines.map((line, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-7">
                  <input className="input text-sm py-1.5" placeholder="Item description" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} />
                </div>
                <div className="col-span-2">
                  <input className="input text-sm py-1.5 text-center" type="number" min="1" step="0.01" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} />
                </div>
                <div className="col-span-2">
                  <input className="input text-sm py-1.5" value={line.uom} onChange={(e) => updateLine(i, 'uom', e.target.value)} />
                </div>
                <div className="col-span-1 text-center">
                  <button type="button" onClick={() => removeLine(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={form.lines.length === 1}>×</button>
                </div>
              </div>
            ))}
          </div>
          <button type="button" onClick={addLine} className="mt-2 text-sm font-medium text-blue-600 hover:text-blue-800">+ Add a Line</button>
        </div>

        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.title.trim()}>
            {mutation.isPending ? 'Saving…' : 'Create Tender'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function BidsModal({ tender, onClose }: { tender: Tender; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [awarding, setAwarding] = useState<Bid | null>(null);

  const { data: bids = [], isLoading } = useQuery<Bid[]>({
    queryKey: ['tender-bids', tender.id],
    queryFn: () => getTenderBids(tender.id).then((r) => (Array.isArray(r.data) ? r.data : [])),
  });
  const { data: vendors = [] } = useQuery<any[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then((r) => (Array.isArray(r.data) ? r.data : [])) });
  const vendorName = (id: string) => vendors.find((v) => v.id === id)?.name ?? `${id.slice(0, 8)}…`;

  const { can } = usePermissions();
  const canAward = can('tender.award') && tender.status === 'open';
  const lowest = bids.length ? Math.min(...bids.map((b) => Number(b.total_amount))) : null;

  return (
    <Modal open={true} onClose={onClose} title={`Bids — ${tender.number}`} subtitle={tender.title} size="lg">
      <div className="space-y-4">
        {isLoading ? (
          <p className="text-sm text-gray-500 py-8 text-center">Loading bids…</p>
        ) : bids.length === 0 ? (
          <p className="text-sm text-gray-500 py-8 text-center">No bids submitted yet.</p>
        ) : (
          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-4">Vendor</div>
              <div className="col-span-3 text-right">Bid Amount</div>
              <div className="col-span-2">Status</div>
              <div className="col-span-3 text-right">Action</div>
            </div>
            {bids.map((b) => (
              <div key={b.id} className="grid grid-cols-12 gap-2 px-3 py-2.5 border-b last:border-b-0 items-center text-sm">
                <div className="col-span-4 font-medium text-gray-900">{vendorName(b.vendor_id)}</div>
                <div className="col-span-3 text-right">
                  <span className="font-medium">{formatCurrency(b.total_amount, b.currency)}</span>
                  {lowest != null && Number(b.total_amount) === lowest && (
                    <span className="ml-1 text-[10px] font-semibold text-emerald-600 uppercase">lowest</span>
                  )}
                </div>
                <div className="col-span-2"><span className={statusColor(b.status)}>{b.status}</span></div>
                <div className="col-span-3 text-right">
                  {canAward && b.status !== 'rejected' && (
                    <button onClick={() => setAwarding(b)} className="btn-primary text-xs py-1 px-2" title="Award & raise the LPO">
                      <Award className="w-3 h-3" /> Award
                    </button>
                  )}
                  {b.status === 'awarded' && <span className="text-xs text-emerald-600 font-medium">Awarded</span>}
                </div>
              </div>
            ))}
          </div>
        )}
        <div className="flex justify-end pt-3 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Close</button>
        </div>
      </div>

      {awarding && (
        <AwardModal
          tender={tender}
          bid={awarding}
          vendorName={vendorName(awarding.vendor_id)}
          onClose={() => setAwarding(null)}
          onAwarded={() => {
            setAwarding(null);
            onClose();
            queryClient.invalidateQueries({ queryKey: ['tenders'] });
            queryClient.invalidateQueries({ queryKey: ['purchase-orders'] });
          }}
        />
      )}
    </Modal>
  );
}

function AwardModal({ tender, bid, vendorName, onClose, onAwarded }: { tender: Tender; bid: Bid; vendorName: string; onClose: () => void; onAwarded: () => void }) {
  const [deliveryDate, setDeliveryDate] = useState('');
  const [notes, setNotes] = useState('');
  const mutation = useMutation({
    mutationFn: () => awardTender(tender.id, { bid_id: bid.id, delivery_date: deliveryDate || undefined, notes: notes || undefined }),
    onSuccess: onAwarded,
  });

  return (
    <Modal open={true} onClose={onClose} title={`Award to ${vendorName}`} subtitle={`This raises an LPO for ${formatCurrency(bid.total_amount, bid.currency)} and rejects other bids.`} size="sm">
      <form onSubmit={(e) => { e.preventDefault(); mutation.mutate(); }} className="space-y-4">
        <div className="flex items-start gap-2 p-3 rounded-lg bg-indigo-50 text-indigo-700 text-sm">
          <Award className="w-4 h-4 shrink-0 mt-0.5" />
          <span>Awarding closes {tender.number}, rejects all other bids, and issues a purchase order the vendor can invoice against.</span>
        </div>
        <div>
          <label className="label">Delivery Date</label>
          <input type="date" className="input" value={deliveryDate} onChange={(e) => setDeliveryDate(e.target.value)} />
        </div>
        <div>
          <label className="label">Notes to vendor</label>
          <textarea className="input" rows={2} value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Delivery instructions, terms…" />
        </div>
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Awarding…' : 'Award & Raise LPO'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
