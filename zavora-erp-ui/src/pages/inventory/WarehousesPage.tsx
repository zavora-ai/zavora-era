import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getWarehouses, createWarehouse, transferStock, getWarehouseStock, getInventory,
  type Warehouse,
} from '../../api/client';
import type { InventoryItem } from '../../types';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import { useToast } from '../../components/toast/ToastProvider';
import { Plus, Warehouse as WarehouseIcon, Truck, ArrowRightLeft } from 'lucide-react';

export default function WarehousesPage() {
  const qc = useQueryClient();
  const { data: whRes } = useQuery({ queryKey: ['warehouses'], queryFn: () => getWarehouses().then((r) => r.data) });
  const warehouses: Warehouse[] = whRes ?? [];
  const { data: items = [] } = useQuery<InventoryItem[]>({ queryKey: ['inventory'], queryFn: () => getInventory().then((r) => Array.isArray(r.data) ? r.data : []) });

  const [showCreate, setShowCreate] = useState(false);
  const [showTransfer, setShowTransfer] = useState(false);
  const [viewWh, setViewWh] = useState<Warehouse | null>(null);

  const { data: whStock = [] } = useQuery<any[]>({
    queryKey: ['warehouse-stock', viewWh?.id], queryFn: () => getWarehouseStock(viewWh!.id).then((r) => r.data), enabled: !!viewWh,
  });

  return (
    <div>
      <PageHeader title="Warehouses" subtitle="Own locations and third-party (3PL) warehouses — track stock per location and transfer between them"
        actions={<>
          <button className="btn-secondary" onClick={() => setShowTransfer(true)} disabled={warehouses.length < 2}><ArrowRightLeft className="w-4 h-4" /> Transfer</button>
          <button className="btn-primary" onClick={() => setShowCreate(true)}><Plus className="w-4 h-4" /> New Warehouse</button>
        </>} />

      {warehouses.length === 0 && (
        <div className="card p-8 text-center text-sm text-gray-500">
          No warehouses yet. Add your own location or a 3PL provider to start tracking stock by location.
        </div>
      )}

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {warehouses.map((w) => (
          <button key={w.id} onClick={() => setViewWh(w)} className="card p-4 text-left hover:ring-2 hover:ring-indigo-100 transition">
            <div className="flex items-center gap-2">
              {w.kind === '3pl' ? <Truck className="w-5 h-5 text-amber-500" /> : <WarehouseIcon className="w-5 h-5 text-indigo-500" />}
              <span className="font-semibold text-gray-900">{w.name}</span>
              {w.is_default && <span className="text-[10px] font-medium bg-indigo-50 text-indigo-600 px-1.5 py-0.5 rounded">DEFAULT</span>}
            </div>
            <p className="text-xs text-gray-400 mt-1 font-mono">{w.code}</p>
            {w.kind === '3pl'
              ? <p className="text-xs text-amber-700 mt-2">3PL{w.provider ? ` · ${w.provider}` : ''}</p>
              : <p className="text-xs text-gray-500 mt-2">Own location</p>}
            {w.location && <p className="text-xs text-gray-400 mt-0.5">{w.location}</p>}
          </button>
        ))}
      </div>

      {showCreate && <CreateWarehouseModal onClose={() => setShowCreate(false)} onDone={() => { qc.invalidateQueries({ queryKey: ['warehouses'] }); setShowCreate(false); }} />}
      {showTransfer && <TransferModal items={items} warehouses={warehouses} onClose={() => setShowTransfer(false)}
        onDone={() => { qc.invalidateQueries({ queryKey: ['warehouse-stock'] }); setShowTransfer(false); }} />}
      {viewWh && (
        <Modal open={true} onClose={() => setViewWh(null)} title={`${viewWh.name} — stock`} subtitle={viewWh.kind === '3pl' ? `3PL · ${viewWh.provider ?? ''}` : 'Own location'} size="lg">
          {whStock.length === 0 ? <p className="text-sm text-gray-400 py-6 text-center">No stock in this warehouse.</p> : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">SKU</th><th className="text-left">Item</th><th className="text-right">Qty</th><th className="text-right">Unit cost</th></tr></thead>
                <tbody>
                  {whStock.map((s) => (
                    <tr key={s.item_id} className="border-b border-gray-50">
                      <td className="py-1.5 font-mono text-xs">{s.sku}</td>
                      <td>{s.description}</td>
                      <td className="text-right tabular-nums">{Number(s.quantity)}</td>
                      <td className="text-right tabular-nums">{Number(s.unit_cost).toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Modal>
      )}
    </div>
  );
}

function CreateWarehouseModal({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const [form, setForm] = useState({ code: '', name: '', kind: 'own', provider: '', location: '' });
  const mut = useMutation({
    mutationFn: () => createWarehouse({ code: form.code, name: form.name, kind: form.kind, provider: form.provider || undefined, location: form.location || undefined }),
    onSuccess: () => { toast.success('Warehouse created.'); onDone(); },
    onError: (e: any) => toast.fromError(e, 'Could not create warehouse.'),
  });
  return (
    <Modal open={true} onClose={onClose} title="New Warehouse">
      <div className="space-y-4">
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Code *</label><input className="input" value={form.code} onChange={(e) => setForm({ ...form, code: e.target.value })} placeholder="e.g. MAIN / 3PL-NBO" /></div>
          <div><label className="label">Name *</label><input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="Warehouse name" /></div>
        </div>
        <div>
          <label className="label">Type</label>
          <div className="flex gap-2">
            <button type="button" onClick={() => setForm({ ...form, kind: 'own' })} className={`flex-1 text-sm py-2 rounded-lg border ${form.kind === 'own' ? 'border-indigo-500 bg-indigo-50 text-indigo-700' : 'border-gray-200 text-gray-600'}`}>Own location</button>
            <button type="button" onClick={() => setForm({ ...form, kind: '3pl' })} className={`flex-1 text-sm py-2 rounded-lg border ${form.kind === '3pl' ? 'border-amber-500 bg-amber-50 text-amber-700' : 'border-gray-200 text-gray-600'}`}>3PL provider</button>
          </div>
        </div>
        {form.kind === '3pl' && <div><label className="label">3PL provider</label><input className="input" value={form.provider} onChange={(e) => setForm({ ...form, provider: e.target.value })} placeholder="e.g. Acme Logistics" /></div>}
        <div><label className="label">Location <span className="text-gray-400 font-normal">(optional)</span></label><input className="input" value={form.location} onChange={(e) => setForm({ ...form, location: e.target.value })} placeholder="City / address" /></div>
        <div className="flex justify-end gap-2 pt-2 border-t">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!form.code.trim() || !form.name.trim() || mut.isPending} onClick={() => mut.mutate()}>Create</button>
        </div>
      </div>
    </Modal>
  );
}

function TransferModal({ items, warehouses, onClose, onDone }: { items: InventoryItem[]; warehouses: Warehouse[]; onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const [form, setForm] = useState({ item_id: '', from_warehouse_id: '', to_warehouse_id: '', quantity: '', notes: '' });
  const mut = useMutation({
    mutationFn: () => transferStock({ item_id: form.item_id, from_warehouse_id: form.from_warehouse_id, to_warehouse_id: form.to_warehouse_id, quantity: Number(form.quantity), notes: form.notes || undefined }),
    onSuccess: () => { toast.success('Stock transferred.'); onDone(); },
    onError: (e: any) => toast.fromError(e, 'Could not transfer stock.'),
  });
  const label = (w: Warehouse) => `${w.name} (${w.code})${w.kind === '3pl' ? ' · 3PL' : ''}`;
  return (
    <Modal open={true} onClose={onClose} title="Transfer stock" subtitle="Move stock between warehouses">
      <div className="space-y-4">
        <div><label className="label">Item</label>
          <select className="input" value={form.item_id} onChange={(e) => setForm({ ...form, item_id: e.target.value })}>
            <option value="">Select an item…</option>
            {items.map((i: any) => <option key={i.id} value={i.id}>{i.sku} — {i.description}</option>)}
          </select>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">From</label>
            <select className="input" value={form.from_warehouse_id} onChange={(e) => setForm({ ...form, from_warehouse_id: e.target.value })}>
              <option value="">From…</option>
              {warehouses.map((w) => <option key={w.id} value={w.id}>{label(w)}</option>)}
            </select>
          </div>
          <div><label className="label">To</label>
            <select className="input" value={form.to_warehouse_id} onChange={(e) => setForm({ ...form, to_warehouse_id: e.target.value })}>
              <option value="">To…</option>
              {warehouses.filter((w) => w.id !== form.from_warehouse_id).map((w) => <option key={w.id} value={w.id}>{label(w)}</option>)}
            </select>
          </div>
        </div>
        <div><label className="label">Quantity</label><input className="input" type="number" value={form.quantity} onChange={(e) => setForm({ ...form, quantity: e.target.value })} /></div>
        <div className="flex justify-end gap-2 pt-2 border-t">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!form.item_id || !form.from_warehouse_id || !form.to_warehouse_id || !(Number(form.quantity) > 0) || mut.isPending} onClick={() => mut.mutate()}>
            <ArrowRightLeft className="w-4 h-4" /> Transfer
          </button>
        </div>
      </div>
    </Modal>
  );
}
