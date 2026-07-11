import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getBoms, createBom, updateBom, getWorkOrders, createWorkOrder,
  startWorkOrder, completeWorkOrder, cancelWorkOrder, getInventory, getProducts, getWarehouses,
  type Bom, type WorkOrder, type Warehouse,
} from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import { useToast } from '../../components/toast/ToastProvider';
import { usePermissions } from '../../hooks/usePermissions';
import { Plus, Factory, Play, CheckCircle2, X, Trash2, Pencil } from 'lucide-react';

const num = (v: any) => Number(v ?? 0);
const money = (v: any) => num(v).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const XS_PRIMARY = 'inline-flex items-center gap-1 text-xs font-medium px-2 py-1 rounded-md bg-indigo-600 text-white hover:bg-indigo-700 disabled:opacity-50';
const XS_SECONDARY = 'inline-flex items-center gap-1 text-xs font-medium px-2 py-1 rounded-md border border-gray-200 text-gray-600 hover:bg-gray-50 disabled:opacity-50';

const STATUS_STYLE: Record<string, string> = {
  draft: 'bg-gray-100 text-gray-600',
  in_progress: 'bg-amber-50 text-amber-700',
  completed: 'bg-green-50 text-green-700',
  cancelled: 'bg-red-50 text-red-600',
};

export default function ManufacturingPage() {
  const qc = useQueryClient();
  const { can } = usePermissions();
  const canWrite = can('inventory.adjust');
  const [tab, setTab] = useState<'orders' | 'boms'>('orders');

  const { data: boms = [] } = useQuery<Bom[]>({ queryKey: ['boms'], queryFn: () => getBoms().then((r) => r.data ?? []) });
  const { data: workOrders = [] } = useQuery<WorkOrder[]>({ queryKey: ['work-orders'], queryFn: () => getWorkOrders().then((r) => r.data ?? []) });

  const [showBom, setShowBom] = useState<Bom | 'new' | null>(null);
  const [showWo, setShowWo] = useState(false);
  const [viewWo, setViewWo] = useState<WorkOrder | null>(null);

  return (
    <div>
      <PageHeader title="Manufacturing" subtitle="Bills of materials and work orders — produce finished goods from components, with material + overhead costing"
        actions={canWrite ? (
          tab === 'orders'
            ? <button className="btn-primary" onClick={() => setShowWo(true)} disabled={boms.length === 0}><Plus className="w-4 h-4" /> New Work Order</button>
            : <button className="btn-primary" onClick={() => setShowBom('new')}><Plus className="w-4 h-4" /> New BOM</button>
        ) : undefined} />

      <div className="flex gap-1 border-b mb-4 overflow-x-auto">
        {(['orders', 'boms'] as const).map((t) => (
          <button key={t} onClick={() => setTab(t)}
            className={`px-4 py-2 text-sm font-medium whitespace-nowrap border-b-2 -mb-px ${tab === t ? 'border-indigo-500 text-indigo-600' : 'border-transparent text-gray-500 hover:text-gray-700'}`}>
            {t === 'orders' ? 'Work Orders' : 'Bills of Materials'}
          </button>
        ))}
      </div>

      {tab === 'orders' && (
        workOrders.length === 0 ? (
          <div className="card p-8 text-center text-sm text-gray-500">
            No work orders yet. {boms.length === 0 ? 'Create a bill of materials first, then produce against it.' : 'Create a work order to produce a finished good from its BOM.'}
          </div>
        ) : (
          <div className="card overflow-x-auto">
            <table className="w-full text-sm">
              <thead><tr className="text-xs text-gray-500 uppercase border-b">
                <th className="text-left py-2 px-3">Order</th><th className="text-left">Finished good</th>
                <th className="text-right">Qty</th><th className="text-right">Total cost</th><th className="text-right">Unit cost</th>
                <th className="text-center">Status</th><th className="text-right px-3">Actions</th>
              </tr></thead>
              <tbody>
                {workOrders.map((w) => (
                  <tr key={w.id} className="border-b border-gray-50 hover:bg-gray-50">
                    <td className="py-2 px-3 font-mono text-xs cursor-pointer" onClick={() => setViewWo(w)}>{w.number}</td>
                    <td className="cursor-pointer" onClick={() => setViewWo(w)}>{w.product_name ?? w.output_sku ?? '—'}</td>
                    <td className="text-right tabular-nums">{num(w.quantity)}</td>
                    <td className="text-right tabular-nums">{money(w.total_cost)}</td>
                    <td className="text-right tabular-nums">{money(w.output_unit_cost)}</td>
                    <td className="text-center"><span className={`text-[10px] font-medium px-2 py-0.5 rounded ${STATUS_STYLE[w.status] ?? 'bg-gray-100'}`}>{w.status.replace('_', ' ')}</span></td>
                    <td className="text-right px-3"><WorkOrderActions wo={w} canWrite={canWrite} onDone={() => qc.invalidateQueries()} onView={() => setViewWo(w)} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )
      )}

      {tab === 'boms' && (
        boms.length === 0 ? (
          <div className="card p-8 text-center text-sm text-gray-500">
            No bills of materials yet. Define a recipe of component items for a finished-good product to start producing.
          </div>
        ) : (
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {boms.map((b) => (
              <button key={b.id} onClick={() => setShowBom(b)} className="card p-4 text-left hover:ring-2 hover:ring-indigo-100 transition">
                <div className="flex items-center gap-2">
                  <Factory className="w-5 h-5 text-indigo-500" />
                  <span className="font-semibold text-gray-900">{b.product_name ?? 'Finished good'}</span>
                  {canWrite && <Pencil className="w-3.5 h-3.5 text-gray-300 ml-auto" />}
                </div>
                <p className="text-xs text-gray-500 mt-2">{b.lines.length} component{b.lines.length === 1 ? '' : 's'} → {num(b.output_quantity)} unit{num(b.output_quantity) === 1 ? '' : 's'}</p>
                {num(b.overhead_cost) > 0 && <p className="text-xs text-gray-400 mt-0.5">Overhead {money(b.overhead_cost)} / batch</p>}
              </button>
            ))}
          </div>
        )
      )}

      {showBom && <BomModal bom={showBom === 'new' ? null : showBom} onClose={() => setShowBom(null)} onDone={() => { qc.invalidateQueries({ queryKey: ['boms'] }); setShowBom(null); }} />}
      {showWo && <WorkOrderModal boms={boms} onClose={() => setShowWo(false)} onDone={() => { qc.invalidateQueries(); setShowWo(false); }} />}
      {viewWo && <ViewWorkOrderModal wo={viewWo} onClose={() => setViewWo(null)} />}
    </div>
  );
}

function WorkOrderActions({ wo, canWrite, onDone, onView }: { wo: WorkOrder; canWrite: boolean; onDone: () => void; onView: () => void }) {
  const toast = useToast();
  const start = useMutation({ mutationFn: () => startWorkOrder(wo.id), onSuccess: () => { toast.success('Components issued to production.'); onDone(); }, onError: (e: any) => toast.fromError(e, 'Could not start.') });
  const complete = useMutation({ mutationFn: () => completeWorkOrder(wo.id), onSuccess: () => { toast.success('Finished goods received.'); onDone(); }, onError: (e: any) => toast.fromError(e, 'Could not complete.') });
  const cancel = useMutation({ mutationFn: () => cancelWorkOrder(wo.id), onSuccess: () => { toast.success('Work order cancelled.'); onDone(); }, onError: (e: any) => toast.fromError(e, 'Could not cancel.') });
  return (
    <div className="flex justify-end gap-1">
      {canWrite && wo.status === 'draft' && <button className={XS_PRIMARY} onClick={() => start.mutate()} disabled={start.isPending}><Play className="w-3 h-3" /> Start</button>}
      {canWrite && wo.status === 'in_progress' && <button className={XS_PRIMARY} onClick={() => complete.mutate()} disabled={complete.isPending}><CheckCircle2 className="w-3 h-3" /> Complete</button>}
      {canWrite && wo.status === 'draft' && <button className={XS_SECONDARY} onClick={() => cancel.mutate()} disabled={cancel.isPending}><X className="w-3 h-3" /></button>}
      <button className={XS_SECONDARY} onClick={onView}>View</button>
    </div>
  );
}

function BomModal({ bom, onClose, onDone }: { bom: Bom | null; onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const editing = !!bom;
  const { data: products = [] } = useQuery<any[]>({ queryKey: ['products'], queryFn: () => getProducts().then((r) => Array.isArray(r.data) ? r.data : (r.data?.data ?? [])) });
  const { data: items = [] } = useQuery<any[]>({ queryKey: ['inventory'], queryFn: () => getInventory().then((r) => Array.isArray(r.data) ? r.data : []) });
  const trackedProducts = products.filter((p: any) => p.inventory_item_id || p.track_inventory);

  const [productId, setProductId] = useState(bom?.product_id ?? '');
  const [outputQty, setOutputQty] = useState(String(bom ? num(bom.output_quantity) : 1));
  const [overhead, setOverhead] = useState(String(bom ? num(bom.overhead_cost) : 0));
  const [lines, setLines] = useState<{ component_item_id: string; quantity: string }[]>(
    bom ? bom.lines.map((l) => ({ component_item_id: l.component_item_id, quantity: String(num(l.quantity)) })) : [{ component_item_id: '', quantity: '' }]
  );

  const payload = () => ({
    product_id: productId,
    output_quantity: Number(outputQty) || 1,
    overhead_cost: Number(overhead) || 0,
    lines: lines.filter((l) => l.component_item_id && Number(l.quantity) > 0).map((l) => ({ component_item_id: l.component_item_id, quantity: Number(l.quantity) })),
  });
  const mut = useMutation({
    mutationFn: () => editing ? updateBom(bom!.id, payload()) : createBom(payload()),
    onSuccess: () => { toast.success(editing ? 'BOM updated.' : 'BOM created.'); onDone(); },
    onError: (e: any) => toast.fromError(e, 'Could not save the BOM.'),
  });
  const valid = productId && payload().lines.length > 0 && Number(outputQty) > 0;

  return (
    <Modal open={true} onClose={onClose} title={editing ? 'Edit bill of materials' : 'New bill of materials'} size="lg">
      <div className="space-y-4">
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Finished good *</label>
            <select className="input" value={productId} disabled={editing} onChange={(e) => setProductId(e.target.value)}>
              <option value="">Select a product…</option>
              {trackedProducts.map((p: any) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
            {!editing && trackedProducts.length === 0 && <p className="text-xs text-amber-600 mt-1">No inventory-tracked products. Enable “track inventory” on a product first.</p>}
          </div>
          <div><label className="label">Output quantity / batch</label><input className="input" type="number" value={outputQty} onChange={(e) => setOutputQty(e.target.value)} /></div>
        </div>
        <div>
          <label className="label">Components</label>
          <div className="space-y-2">
            {lines.map((l, i) => (
              <div key={i} className="flex gap-2 items-center">
                <select className="input flex-1" value={l.component_item_id} onChange={(e) => setLines(lines.map((x, j) => j === i ? { ...x, component_item_id: e.target.value } : x))}>
                  <option value="">Select component…</option>
                  {items.map((it: any) => <option key={it.id} value={it.id}>{it.sku} — {it.description}</option>)}
                </select>
                <input className="input w-28" type="number" placeholder="Qty" value={l.quantity} onChange={(e) => setLines(lines.map((x, j) => j === i ? { ...x, quantity: e.target.value } : x))} />
                <button className="text-gray-400 hover:text-red-500" onClick={() => setLines(lines.filter((_, j) => j !== i))}><Trash2 className="w-4 h-4" /></button>
              </div>
            ))}
          </div>
          <button className={`${XS_SECONDARY} mt-2`} onClick={() => setLines([...lines, { component_item_id: '', quantity: '' }])}><Plus className="w-3 h-3" /> Add component</button>
        </div>
        <div className="w-1/2"><label className="label">Labour / overhead per batch</label><input className="input" type="number" value={overhead} onChange={(e) => setOverhead(e.target.value)} /></div>
        <div className="flex justify-end gap-2 pt-2 border-t">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!valid || mut.isPending} onClick={() => mut.mutate()}>{editing ? 'Save' : 'Create BOM'}</button>
        </div>
      </div>
    </Modal>
  );
}

function WorkOrderModal({ boms, onClose, onDone }: { boms: Bom[]; onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const { data: whRes } = useQuery({ queryKey: ['warehouses'], queryFn: () => getWarehouses().then((r) => r.data) });
  const warehouses: Warehouse[] = whRes ?? [];
  const [bomId, setBomId] = useState('');
  const [quantity, setQuantity] = useState('');
  const [sourceWh, setSourceWh] = useState('');
  const [destWh, setDestWh] = useState('');
  const [overhead, setOverhead] = useState('');
  const bom = boms.find((b) => b.id === bomId);

  const mut = useMutation({
    mutationFn: () => createWorkOrder({
      bom_id: bomId, quantity: Number(quantity),
      source_warehouse_id: sourceWh || undefined, dest_warehouse_id: destWh || undefined,
      overhead_cost: overhead === '' ? undefined : Number(overhead),
    }),
    onSuccess: () => { toast.success('Work order created (draft). Start it to issue components.'); onDone(); },
    onError: (e: any) => toast.fromError(e, 'Could not create the work order.'),
  });
  const label = (w: Warehouse) => `${w.name} (${w.code})${w.kind === '3pl' ? ' · 3PL' : ''}`;

  return (
    <Modal open={true} onClose={onClose} title="New work order" subtitle="Produce a finished good from its BOM">
      <div className="space-y-4">
        <div><label className="label">Bill of materials *</label>
          <select className="input" value={bomId} onChange={(e) => setBomId(e.target.value)}>
            <option value="">Select a BOM…</option>
            {boms.map((b) => <option key={b.id} value={b.id}>{b.product_name ?? 'Finished good'} ({b.lines.length} components → {num(b.output_quantity)})</option>)}
          </select>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Quantity to produce *</label><input className="input" type="number" value={quantity} onChange={(e) => setQuantity(e.target.value)} /></div>
          <div><label className="label">Overhead override <span className="text-gray-400 font-normal">(optional)</span></label><input className="input" type="number" placeholder={bom ? `default ${money(bom.overhead_cost)}` : ''} value={overhead} onChange={(e) => setOverhead(e.target.value)} /></div>
        </div>
        {warehouses.length > 0 && (
          <div className="grid grid-cols-2 gap-3">
            <div><label className="label">Consume from</label>
              <select className="input" value={sourceWh} onChange={(e) => setSourceWh(e.target.value)}>
                <option value="">Default warehouse</option>
                {warehouses.map((w) => <option key={w.id} value={w.id}>{label(w)}</option>)}
              </select>
            </div>
            <div><label className="label">Produce into</label>
              <select className="input" value={destWh} onChange={(e) => setDestWh(e.target.value)}>
                <option value="">Default warehouse</option>
                {warehouses.map((w) => <option key={w.id} value={w.id}>{label(w)}</option>)}
              </select>
            </div>
          </div>
        )}
        <div className="flex justify-end gap-2 pt-2 border-t">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!bomId || !(Number(quantity) > 0) || mut.isPending} onClick={() => mut.mutate()}>Create</button>
        </div>
      </div>
    </Modal>
  );
}

function ViewWorkOrderModal({ wo, onClose }: { wo: WorkOrder; onClose: () => void }) {
  return (
    <Modal open={true} onClose={onClose} title={`${wo.number} — ${wo.product_name ?? wo.output_sku ?? 'work order'}`}
      subtitle={`${wo.status.replace('_', ' ')} · producing ${num(wo.quantity)} unit${num(wo.quantity) === 1 ? '' : 's'}`} size="lg">
      <div className="space-y-4">
        <div className="grid grid-cols-3 gap-3 text-center">
          <div className="card p-3"><p className="text-xs text-gray-500">Material</p><p className="text-lg font-semibold tabular-nums">{money(wo.material_cost)}</p></div>
          <div className="card p-3"><p className="text-xs text-gray-500">Overhead</p><p className="text-lg font-semibold tabular-nums">{money(wo.overhead_cost)}</p></div>
          <div className="card p-3"><p className="text-xs text-gray-500">Total / unit</p><p className="text-lg font-semibold tabular-nums">{money(wo.total_cost)} <span className="text-xs text-gray-400">/ {money(wo.output_unit_cost)}</span></p></div>
        </div>
        <div>
          <p className="label mb-1">Components consumed</p>
          {wo.consumptions.length === 0 ? <p className="text-sm text-gray-400 py-3 text-center">Not started — components are issued when you Start the order.</p> : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-1.5">Component</th><th className="text-right">Qty</th><th className="text-right">Unit cost</th><th className="text-right">Total</th></tr></thead>
                <tbody>
                  {wo.consumptions.map((c) => (
                    <tr key={c.id} className="border-b border-gray-50">
                      <td className="py-1.5 font-mono text-xs">{c.component_item_id.slice(0, 8)}</td>
                      <td className="text-right tabular-nums">{num(c.quantity)}</td>
                      <td className="text-right tabular-nums">{money(c.unit_cost)}</td>
                      <td className="text-right tabular-nums">{money(c.total_cost)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
        <div className="flex justify-end pt-2 border-t"><button className="btn-secondary" onClick={onClose}>Close</button></div>
      </div>
    </Modal>
  );
}
