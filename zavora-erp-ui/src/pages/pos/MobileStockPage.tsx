import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getInventory, receiveInventory, adjustInventory, getAccounts } from '../../api/client';
import { formatNumber } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import { PackagePlus, ClipboardCheck } from 'lucide-react';

interface Item { id: string; name?: string; sku?: string; on_hand: string | number; unit_cost?: string | number; }

export default function MobileStockPage() {
  const [mode, setMode] = useState<'receive' | 'count'>('receive');
  const { data: items = [] } = useQuery<Item[]>({ queryKey: ['inventory'], queryFn: () => getInventory().then((r) => (Array.isArray(r.data) ? r.data : r.data?.data ?? [])) });

  return (
    <div className="max-w-xl mx-auto">
      <PageHeader title="Stock (Mobile)" subtitle="Receive new stock and do stock counts from the shop floor." />
      <div className="grid grid-cols-2 gap-2 mb-5">
        <button onClick={() => setMode('receive')} className={`rounded-xl border-2 p-4 flex flex-col items-center gap-1 ${mode === 'receive' ? 'border-indigo-500 bg-indigo-50' : 'border-gray-200'}`}><PackagePlus className="w-6 h-6 text-emerald-600" /><span className="font-semibold text-sm">Receive stock</span></button>
        <button onClick={() => setMode('count')} className={`rounded-xl border-2 p-4 flex flex-col items-center gap-1 ${mode === 'count' ? 'border-indigo-500 bg-indigo-50' : 'border-gray-200'}`}><ClipboardCheck className="w-6 h-6 text-amber-600" /><span className="font-semibold text-sm">Stock count</span></button>
      </div>
      {mode === 'receive' ? <ReceiveForm items={items} /> : <CountForm items={items} />}
    </div>
  );
}

function ItemSelect({ items, value, onChange }: { items: Item[]; value: string; onChange: (v: string) => void }) {
  return (
    <select className="input text-base py-3" value={value} onChange={(e) => onChange(e.target.value)} required>
      <option value="">Select an item…</option>
      {items.map((it) => <option key={it.id} value={it.id}>{it.name ?? it.sku} — on hand {formatNumber(Number(it.on_hand))}</option>)}
    </select>
  );
}

function ReceiveForm({ items }: { items: Item[] }) {
  const qc = useQueryClient();
  const [itemId, setItemId] = useState('');
  const [qty, setQty] = useState(0);
  const [cost, setCost] = useState(0);
  const [msg, setMsg] = useState<string | null>(null);
  const mut = useMutation({
    mutationFn: () => receiveInventory({ item_id: itemId, quantity: Number(qty), unit_cost: Number(cost) }),
    onSuccess: () => { setMsg('Stock received.'); setQty(0); setCost(0); qc.invalidateQueries({ queryKey: ['inventory'] }); },
    onError: (e: any) => setMsg(e?.response?.data?.error || 'Failed.'),
  });
  return (
    <div className="bg-white rounded-2xl border border-gray-200 p-4 space-y-3">
      <label className="label">Item *</label>
      <ItemSelect items={items} value={itemId} onChange={setItemId} />
      <label className="label">Quantity received *</label>
      <input type="number" min="0" step="0.01" className="input text-2xl text-center py-3" value={qty} onChange={(e) => setQty(+e.target.value)} />
      <label className="label">Unit cost (KES) *</label>
      <input type="number" min="0" step="0.01" className="input text-lg py-3" value={cost} onChange={(e) => setCost(+e.target.value)} />
      {msg && <div className="rounded-lg bg-gray-50 border px-3 py-2 text-sm text-gray-700">{msg}</div>}
      <button onClick={() => { setMsg(null); mut.mutate(); }} disabled={mut.isPending || !itemId || qty <= 0} className="btn-primary w-full justify-center text-base py-3">{mut.isPending ? 'Receiving…' : 'Receive stock'}</button>
    </div>
  );
}

function CountForm({ items }: { items: Item[] }) {
  const qc = useQueryClient();
  const { data: accounts = [] } = useQuery<any[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then((r) => (Array.isArray(r.data) ? r.data : [])) });
  const expenseAccts = accounts.filter((a) => ['Expense', 'ContraExpense', 'CostOfSales'].includes(a.account_type));
  const [itemId, setItemId] = useState('');
  const [counted, setCounted] = useState(0);
  const [account, setAccount] = useState('');
  const [reason, setReason] = useState('');
  const [msg, setMsg] = useState<string | null>(null);
  const current = items.find((i) => i.id === itemId);
  const variance = current ? counted - Number(current.on_hand) : 0;

  const mut = useMutation({
    mutationFn: () => adjustInventory({ item_id: itemId, counted_quantity: Number(counted), adjustment_account: account, reason: reason || undefined }),
    onSuccess: () => { setMsg('Count posted.'); qc.invalidateQueries({ queryKey: ['inventory'] }); },
    onError: (e: any) => setMsg(e?.response?.data?.error || 'Failed.'),
  });
  return (
    <div className="bg-white rounded-2xl border border-gray-200 p-4 space-y-3">
      <label className="label">Item *</label>
      <ItemSelect items={items} value={itemId} onChange={(v) => { setItemId(v); const it = items.find((i) => i.id === v); setCounted(Number(it?.on_hand ?? 0)); }} />
      {current && <p className="text-sm text-gray-500">System says on hand: <b>{formatNumber(Number(current.on_hand))}</b></p>}
      <label className="label">Counted quantity *</label>
      <input type="number" min="0" step="0.01" className="input text-2xl text-center py-3" value={counted} onChange={(e) => setCounted(+e.target.value)} />
      {current && <p className="text-center text-sm">Variance: <b className={variance < 0 ? 'text-red-600' : variance > 0 ? 'text-emerald-600' : ''}>{variance > 0 ? '+' : ''}{formatNumber(variance)}</b></p>}
      <label className="label">Adjustment account *</label>
      <select className="input" value={account} onChange={(e) => setAccount(e.target.value)} required>
        <option value="">Select account…</option>
        {expenseAccts.map((a) => <option key={a.code} value={a.code}>{a.code} · {a.name}</option>)}
      </select>
      <label className="label">Reason</label>
      <input className="input" value={reason} onChange={(e) => setReason(e.target.value)} placeholder="e.g. breakage, recount" />
      {msg && <div className="rounded-lg bg-gray-50 border px-3 py-2 text-sm text-gray-700">{msg}</div>}
      <button onClick={() => { setMsg(null); mut.mutate(); }} disabled={mut.isPending || !itemId || !account} className="btn-primary w-full justify-center text-base py-3">{mut.isPending ? 'Posting…' : 'Post count'}</button>
    </div>
  );
}
