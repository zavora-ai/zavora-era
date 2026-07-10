import { useState, useMemo } from 'react';
import { useToast } from '../../components/toast/ToastProvider';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPosSession, openPosSession, completePosSale, getProducts, getPosReceipt } from '../../api/client';
import { formatCurrency } from '../../utils/format';
import Modal from '../../components/shared/Modal';
import { ShoppingCart, Plus, Minus, Trash2, Banknote, Smartphone, Search, Printer } from 'lucide-react';

/** Fetch the ETR/eTIMS thermal receipt and open it in a print window. */
async function printReceipt(invoiceId: string, tendered?: number) {
  try {
    const r = await getPosReceipt(invoiceId, tendered);
    const w = window.open('', '_blank', 'width=380,height=640');
    if (!w) { window.alert('Allow pop-ups to print the receipt.'); return; }
    w.document.open(); w.document.write(r.data as unknown as string); w.document.close();
  } catch { window.alert('Could not load the receipt.'); }
}

interface Product { id: string; name: string; unit_price?: string | number | null; sku?: string; }
interface CartLine { product: Product; quantity: number; unit_price: number; }

const priceOf = (p: Product) => Number(p.unit_price ?? 0);

export default function PosSellPage() {
  const qc = useQueryClient();
  const { data: session, isLoading } = useQuery({ queryKey: ['pos-session'], queryFn: () => getPosSession().then((r) => r.data) });

  if (isLoading) return <p className="text-sm text-gray-500 py-12 text-center">Loading…</p>;
  if (!session) return <OpenTill onOpened={() => qc.invalidateQueries({ queryKey: ['pos-session'] })} />;
  return <Register session={session} />;
}

function OpenTill({ onOpened }: { onOpened: () => void }) {
  const [float, setFloat] = useState(0);
  const [name, setName] = useState('Main Till');
  const mut = useMutation({ mutationFn: () => openPosSession({ register_name: name, opening_float: Number(float) }), onSuccess: onOpened });
  return (
    <div className="max-w-md mx-auto mt-10 bg-white rounded-2xl border border-gray-200 p-6">
      <div className="text-center mb-5"><ShoppingCart className="w-10 h-10 mx-auto text-indigo-500 mb-2" /><h1 className="text-xl font-bold">Open the till</h1><p className="text-sm text-gray-500">Start a shift to begin selling.</p></div>
      <label className="label">Register name</label>
      <input className="input mb-3" value={name} onChange={(e) => setName(e.target.value)} />
      <label className="label">Opening float (cash in drawer)</label>
      <input type="number" min="0" className="input mb-4 text-lg" value={float} onChange={(e) => setFloat(+e.target.value)} />
      <button onClick={() => mut.mutate()} disabled={mut.isPending} className="btn-primary w-full justify-center text-base py-3">{mut.isPending ? 'Opening…' : 'Open till'}</button>
    </div>
  );
}

function Register({ session }: { session: any }) {
  const { data: products = [] } = useQuery<Product[]>({ queryKey: ['products'], queryFn: () => getProducts().then((r) => (Array.isArray(r.data) ? r.data : r.data?.data ?? [])) });
  const [cart, setCart] = useState<CartLine[]>([]);
  const [q, setQ] = useState('');
  const [paying, setPaying] = useState(false);

  const total = useMemo(() => cart.reduce((s, l) => s + l.quantity * l.unit_price, 0), [cart]);
  const filtered = useMemo(() => products.filter((p) => p.name?.toLowerCase().includes(q.toLowerCase())), [products, q]);

  const add = (p: Product) => setCart((c) => {
    const i = c.findIndex((l) => l.product.id === p.id);
    if (i >= 0) { const n = [...c]; n[i] = { ...n[i], quantity: n[i].quantity + 1 }; return n; }
    return [...c, { product: p, quantity: 1, unit_price: priceOf(p) }];
  });
  const setQty = (id: string, d: number) => setCart((c) => c.map((l) => l.product.id === id ? { ...l, quantity: Math.max(1, l.quantity + d) } : l));
  const setPrice = (id: string, v: number) => setCart((c) => c.map((l) => l.product.id === id ? { ...l, unit_price: v } : l));
  const remove = (id: string) => setCart((c) => c.filter((l) => l.product.id !== id));

  return (
    <div className="flex flex-col lg:flex-row gap-4 h-[calc(100vh-8rem)]">
      {/* Product grid */}
      <div className="flex-1 flex flex-col min-h-0">
        <div className="flex items-center gap-2 mb-3">
          <div className="relative flex-1"><Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" /><input className="input pl-9" placeholder="Search products…" value={q} onChange={(e) => setQ(e.target.value)} /></div>
          <span className="text-xs text-gray-500 whitespace-nowrap">Till: <b>{session.register_name}</b></span>
        </div>
        <div className="grid grid-cols-2 sm:grid-cols-3 xl:grid-cols-4 gap-2 overflow-y-auto pr-1">
          {filtered.map((p) => (
            <button key={p.id} onClick={() => add(p)} className="text-left rounded-xl border border-gray-200 bg-white p-3 hover:border-indigo-400 hover:shadow-sm active:scale-[0.98] transition">
              <p className="font-medium text-gray-900 text-sm line-clamp-2 min-h-[2.5rem]">{p.name}</p>
              <p className="text-indigo-600 font-semibold mt-1">{formatCurrency(priceOf(p), 'KES')}</p>
            </button>
          ))}
          {filtered.length === 0 && <p className="col-span-full text-center text-gray-400 py-8 text-sm">No products.</p>}
        </div>
      </div>

      {/* Cart */}
      <div className="lg:w-96 flex flex-col bg-white rounded-2xl border border-gray-200 min-h-0">
        <div className="px-4 py-3 border-b flex items-center gap-2"><ShoppingCart className="w-4 h-4" /><span className="font-semibold">Cart</span><span className="ml-auto text-sm text-gray-500">{cart.length} item(s)</span></div>
        <div className="flex-1 overflow-y-auto divide-y">
          {cart.length === 0 ? <p className="text-center text-gray-400 py-10 text-sm">Tap products to add them.</p> : cart.map((l) => (
            <div key={l.product.id} className="p-3">
              <div className="flex justify-between gap-2"><span className="text-sm font-medium text-gray-900">{l.product.name}</span><button onClick={() => remove(l.product.id)} className="text-gray-400 hover:text-red-500"><Trash2 className="w-4 h-4" /></button></div>
              <div className="flex items-center gap-2 mt-2">
                <button onClick={() => setQty(l.product.id, -1)} className="w-8 h-8 rounded-lg border flex items-center justify-center"><Minus className="w-4 h-4" /></button>
                <span className="w-8 text-center font-semibold">{l.quantity}</span>
                <button onClick={() => setQty(l.product.id, 1)} className="w-8 h-8 rounded-lg border flex items-center justify-center"><Plus className="w-4 h-4" /></button>
                <span className="text-gray-400">×</span>
                <input type="number" min="0" step="0.01" className="input text-sm py-1 w-24 text-right" value={l.unit_price} onChange={(e) => setPrice(l.product.id, +e.target.value)} />
                <span className="ml-auto font-semibold">{formatCurrency(l.quantity * l.unit_price, 'KES')}</span>
              </div>
            </div>
          ))}
        </div>
        <div className="p-4 border-t">
          <div className="flex justify-between text-lg font-bold mb-3"><span>Total</span><span>{formatCurrency(total, 'KES')}</span></div>
          <button disabled={cart.length === 0} onClick={() => setPaying(true)} className="btn-primary w-full justify-center text-base py-3 disabled:opacity-40">Charge {formatCurrency(total, 'KES')}</button>
        </div>
      </div>

      {paying && <TenderModal sessionId={session.id} cart={cart} total={total} onClose={() => setPaying(false)} onDone={() => { setCart([]); setPaying(false); }} />}
    </div>
  );
}

function TenderModal({ sessionId, cart, total, onClose, onDone }: { sessionId: string; cart: CartLine[]; total: number; onClose: () => void; onDone: () => void }) {
  const qc = useQueryClient();
  const toast = useToast();
  const [tender, setTender] = useState<'cash' | 'mpesa' | null>(null);
  const [tendered, setTendered] = useState(total);
  const [ref, setRef] = useState('');
  const [phone, setPhone] = useState('');
  const [result, setResult] = useState<any>(null);

  const mut = useMutation({
    mutationFn: () => completePosSale(sessionId, {
      tender: tender!,
      amount_tendered: tender === 'cash' ? Number(tendered) : undefined,
      mpesa_reference: tender === 'mpesa' ? ref : undefined,
      mpesa_phone: tender === 'mpesa' ? phone : undefined,
      lines: cart.map((l) => ({ product_id: l.product.id, quantity: l.quantity, unit_price: l.unit_price })),
    }),
    onSuccess: (r) => { setResult(r.data); qc.invalidateQueries({ queryKey: ['pos-session'] }); },
    onError: (e: any) => toast.fromError(e, 'Sale failed.'),
  });

  if (result) return (
    <Modal open={true} onClose={onDone} title="Sale complete" size="sm">
      <div className="text-center py-4">
        <div className="w-14 h-14 rounded-full bg-emerald-100 text-emerald-600 flex items-center justify-center mx-auto mb-3 text-2xl">✓</div>
        <p className="font-semibold text-lg">{result.invoice_number}</p>
        <p className="text-2xl font-bold my-2">{formatCurrency(result.gross_total, 'KES')}</p>
        {Number(result.change) > 0 && <p className="text-indigo-600 font-medium">Change due: {formatCurrency(result.change, 'KES')}</p>}
        <button onClick={() => printReceipt(result.invoice_id, Number(result.gross_total) + Number(result.change || 0))} className="btn-secondary w-full justify-center mt-4">
          <Printer className="w-4 h-4" /> Print ETR receipt
        </button>
        <button onClick={onDone} className="btn-primary w-full justify-center mt-2">New sale</button>
      </div>
    </Modal>
  );

  return (
    <Modal open={true} onClose={onClose} title={`Charge ${formatCurrency(total, 'KES')}`} size="sm">
      {!tender ? (
        <div className="grid grid-cols-2 gap-3 py-2">
          <button onClick={() => setTender('cash')} className="rounded-xl border-2 border-gray-200 hover:border-indigo-400 p-5 flex flex-col items-center gap-2"><Banknote className="w-8 h-8 text-emerald-600" /><span className="font-semibold">Cash</span></button>
          <button onClick={() => setTender('mpesa')} className="rounded-xl border-2 border-gray-200 hover:border-indigo-400 p-5 flex flex-col items-center gap-2"><Smartphone className="w-8 h-8 text-green-600" /><span className="font-semibold">M-Pesa</span></button>
        </div>
      ) : tender === 'cash' ? (
        <div className="space-y-3">
          <label className="label">Cash received</label>
          <input type="number" min={total} className="input text-2xl text-center py-3" value={tendered} onChange={(e) => setTendered(+e.target.value)} autoFocus />
          <p className="text-center text-sm">Change: <b>{formatCurrency(Math.max(0, tendered - total), 'KES')}</b></p>
          <button onClick={() => mut.mutate()} disabled={mut.isPending || tendered < total} className="btn-primary w-full justify-center text-base py-3">{mut.isPending ? 'Completing…' : 'Complete sale'}</button>
        </div>
      ) : (
        <div className="space-y-3">
          <label className="label">M-Pesa transaction code</label>
          <input className="input" placeholder="e.g. SLJ7XK2P9Q" value={ref} onChange={(e) => setRef(e.target.value.toUpperCase())} autoFocus />
          <label className="label">Customer phone (optional)</label>
          <input className="input" placeholder="2547…" value={phone} onChange={(e) => setPhone(e.target.value)} />
          <button onClick={() => mut.mutate()} disabled={mut.isPending || !ref.trim()} className="btn-primary w-full justify-center text-base py-3">{mut.isPending ? 'Completing…' : 'Complete sale'}</button>
        </div>
      )}
    </Modal>
  );
}
