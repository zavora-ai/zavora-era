import { useEffect, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPortalTenders, getPortalTender, submitPortalBid } from '../../api/portalClient';
import { formatCurrency, formatDate } from '../../utils/format';
import Modal from '../../components/shared/Modal';
import { Gavel, Send } from 'lucide-react';

interface Tender {
  id: string; number: string; title: string; description?: string; category?: string;
  closing_date?: string; status: string;
}
interface TenderLine { id: string; description: string; quantity: string; uom: string; }

export default function PortalTendersPage() {
  const [bidFor, setBidFor] = useState<Tender | null>(null);

  const { data: tenders = [], isLoading } = useQuery<Tender[]>({
    queryKey: ['portal-tenders'],
    queryFn: () => getPortalTenders().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-gray-900">Open Tenders</h1>
        <p className="mt-1 text-sm text-gray-500">Requests for quotation you can bid on. Submit your best price before the closing date.</p>
      </div>

      {isLoading ? (
        <p className="text-sm text-gray-500 py-12 text-center">Loading tenders…</p>
      ) : tenders.length === 0 ? (
        <div className="bg-white rounded-xl border border-gray-200 p-12 text-center">
          <Gavel className="w-10 h-10 text-gray-300 mx-auto mb-3" />
          <p className="text-gray-500">No open tenders right now. Check back soon.</p>
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2">
          {tenders.map((t) => (
            <div key={t.id} className="bg-white rounded-xl border border-gray-200 p-5 hover:shadow-md transition-shadow">
              <div className="flex items-start justify-between mb-2">
                <span className="text-xs font-semibold text-blue-600">{t.number}</span>
                {t.category && <span className="text-xs px-2 py-0.5 bg-gray-100 rounded-full text-gray-600">{t.category}</span>}
              </div>
              <h3 className="font-semibold text-gray-900 mb-1">{t.title}</h3>
              {t.description && <p className="text-sm text-gray-500 line-clamp-2 mb-3">{t.description}</p>}
              <div className="flex items-center justify-between mt-4">
                <span className="text-xs text-gray-400">
                  {t.closing_date ? `Closes ${formatDate(t.closing_date)}` : 'No closing date'}
                </span>
                <button onClick={() => setBidFor(t)} className="btn-primary text-sm bg-emerald-600 hover:bg-emerald-700">
                  <Send className="w-3.5 h-3.5" /> Submit bid
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {bidFor && <BidModal tender={bidFor} onClose={() => setBidFor(null)} />}
    </div>
  );
}

function BidModal({ tender, onClose }: { tender: Tender; onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data } = useQuery({ queryKey: ['portal-tender', tender.id], queryFn: () => getPortalTender(tender.id).then((r) => r.data) });
  const tenderLines: TenderLine[] = data?.lines ?? [];
  const existingBid = data?.my_bid;

  const [lines, setLines] = useState<{ tender_line_id?: string; description: string; quantity: number; unit_price: number }[]>([]);
  const [notes, setNotes] = useState('');

  // Seed the bid lines from the tender's requested items once loaded.
  useEffect(() => {
    if (lines.length === 0 && tenderLines.length > 0) {
      setLines(tenderLines.map((l) => ({ tender_line_id: l.id, description: l.description, quantity: Number(l.quantity), unit_price: 0 })));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tenderLines]);

  const mutation = useMutation({
    mutationFn: () => submitPortalBid(tender.id, { currency: 'KES', notes: notes || undefined, lines }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['portal-tenders'] });
      queryClient.invalidateQueries({ queryKey: ['portal-bids'] });
      onClose();
    },
  });

  const updateLine = (i: number, field: string, value: any) => {
    const next = [...lines];
    (next[i] as any)[field] = value;
    setLines(next);
  };
  const total = lines.reduce((s, l) => s + l.quantity * l.unit_price, 0);

  return (
    <Modal open={true} onClose={onClose} title={`Bid on ${tender.number}`} subtitle={tender.title} size="lg">
      <form onSubmit={(e) => { e.preventDefault(); mutation.mutate(); }} className="space-y-5">
        {existingBid && (
          <div className="p-3 rounded-lg bg-amber-50 text-amber-700 text-sm">
            You already have a {existingBid.status} bid of {formatCurrency(existingBid.total_amount, existingBid.currency)}. Submitting again replaces it.
          </div>
        )}

        <div className="border rounded-lg overflow-hidden">
          <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
            <div className="col-span-6">Item</div>
            <div className="col-span-2 text-center">Qty</div>
            <div className="col-span-2 text-right">Unit Price</div>
            <div className="col-span-2 text-right">Amount</div>
          </div>
          {lines.map((line, i) => (
            <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
              <div className="col-span-6">
                <input className="input text-sm py-1.5" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} required />
              </div>
              <div className="col-span-2">
                <input className="input text-sm py-1.5 text-center" type="number" min="0" step="0.01" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} />
              </div>
              <div className="col-span-2">
                <input className="input text-sm py-1.5 text-right" type="number" min="0" step="0.01" value={line.unit_price} onChange={(e) => updateLine(i, 'unit_price', +e.target.value)} required />
              </div>
              <div className="col-span-2 text-right text-sm font-medium">{formatCurrency(line.quantity * line.unit_price)}</div>
            </div>
          ))}
        </div>

        <div className="flex justify-between items-center">
          <div className="flex-1 mr-4">
            <label className="label">Notes to the buyer</label>
            <input className="input" value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Lead time, warranty, terms…" />
          </div>
          <div className="text-right">
            <p className="text-xs text-gray-500">Total bid</p>
            <p className="text-xl font-bold text-gray-900">{formatCurrency(total)}</p>
          </div>
        </div>

        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary bg-emerald-600 hover:bg-emerald-700" disabled={mutation.isPending || total <= 0}>
            {mutation.isPending ? 'Submitting…' : 'Submit bid'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
