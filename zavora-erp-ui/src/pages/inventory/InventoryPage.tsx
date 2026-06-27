import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getInventory, createInventoryItem, receiveInventory, issueInventory, adjustInventory, getAccounts } from '../../api/client';
import type { InventoryItem } from '../../types';
import { formatCurrency, formatNumber } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, PackagePlus, PackageMinus, AlertTriangle, ClipboardCheck } from 'lucide-react';

export default function InventoryPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [showReceive, setShowReceive] = useState(false);
  const [showIssue, setShowIssue] = useState(false);
  const [showAdjust, setShowAdjust] = useState(false);

  const { data: items = [], isLoading } = useQuery<InventoryItem[]>({
    queryKey: ['inventory'],
    queryFn: () => getInventory().then(r => Array.isArray(r.data) ? r.data : []),
  });

  const columns: Column<InventoryItem>[] = [
    {
      key: 'sku', header: 'SKU',
      render: (r) => <span className="font-mono text-xs font-medium">{r.sku}</span>,
    },
    {
      key: 'description', header: 'Description',
      render: (r) => (
        <div className="flex items-center gap-2">
          <span className="font-medium text-gray-900">{r.description}</span>
          {r.reorder_point != null && r.available <= r.reorder_point && (
            <span className="inline-flex items-center gap-1 text-amber-600" title="Below reorder point">
              <AlertTriangle className="w-3.5 h-3.5" />
            </span>
          )}
        </div>
      ),
    },
    {
      key: 'on_hand', header: 'On Hand',
      render: (r) => formatNumber(r.on_hand),
      className: 'text-right',
    },
    {
      key: 'committed', header: 'Committed',
      render: (r) => formatNumber(r.committed),
      className: 'text-right',
    },
    {
      key: 'available', header: 'Available',
      render: (r) => (
        <span className={r.reorder_point != null && r.available <= r.reorder_point ? 'text-red-600 font-semibold' : ''}>
          {formatNumber(r.available)}
        </span>
      ),
      className: 'text-right',
    },
    {
      key: 'unit_cost', header: 'Unit Cost',
      render: (r) => formatCurrency(r.unit_cost),
      className: 'text-right',
    },
    {
      key: 'total_value', header: 'Total Value',
      render: (r) => formatCurrency(r.total_value),
      className: 'text-right',
    },
    {
      key: 'reorder_point', header: 'Reorder Pt',
      render: (r) => r.reorder_point != null ? formatNumber(r.reorder_point) : <span className="text-gray-400">—</span>,
      className: 'text-right',
    },
  ];

  return (
    <div>
      <PageHeader
        title="Inventory"
        subtitle="Track stock levels, receive and issue goods"
        actions={
          <>
            <button onClick={() => setShowReceive(true)} className="btn-success">
              <PackagePlus className="w-4 h-4" /> Receive Stock
            </button>
            <button onClick={() => setShowIssue(true)} className="btn-danger">
              <PackageMinus className="w-4 h-4" /> Issue Stock
            </button>
            <button onClick={() => setShowAdjust(true)} className="btn-secondary">
              <ClipboardCheck className="w-4 h-4" /> Stock Take
            </button>
            <button onClick={() => setShowCreate(true)} className="btn-primary">
              <Plus className="w-4 h-4" /> Add Item
            </button>
          </>
        }
      />
      <DataTable
        columns={columns}
        data={items}
        loading={isLoading}
        emptyMessage="No inventory items. Add stock items to track quantities and value."
      />
      {showCreate && <CreateItemModal onClose={() => setShowCreate(false)} />}
      {showReceive && <ReceiveStockModal items={items} onClose={() => setShowReceive(false)} />}
      {showIssue && <IssueStockModal items={items} onClose={() => setShowIssue(false)} />}
      {showAdjust && <AdjustStockModal items={items} onClose={() => setShowAdjust(false)} />}
    </div>
  );
}

function CreateItemModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    sku: '',
    description: '',
    uom: 'Each',
    costing_method: 'WeightedAvgCost',
    gl_inventory: '1300',
    gl_cogs: '6000',
    reorder_point: '',
    reorder_quantity: '',
  });

  const [error, setError] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: (data: any) => createInventoryItem(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['inventory'] }); onClose(); },
    onError: (e: any) => setError(e?.response?.data?.error || 'Failed to create item.'),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    mutation.mutate({
      sku: form.sku,
      description: form.description,
      uom: form.uom,
      costing_method: form.costing_method,
      gl_inventory: form.gl_inventory,
      gl_cogs: form.gl_cogs,
      reorder_point: form.reorder_point ? parseInt(form.reorder_point) : undefined,
      reorder_quantity: form.reorder_quantity ? parseInt(form.reorder_quantity) : undefined,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Add Inventory Item" size="lg">
      <form onSubmit={handleSubmit} className="space-y-5">
        {error && (
          <div className="p-3 rounded-lg bg-red-50 text-red-700 text-sm">{error}</div>
        )}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">SKU *</label>
            <input className="input font-mono" value={form.sku} onChange={(e) => setForm({ ...form, sku: e.target.value })} placeholder="e.g. INV-001" required />
          </div>
          <div>
            <label className="label">Unit of Measure</label>
            <select className="input" value={form.uom} onChange={(e) => setForm({ ...form, uom: e.target.value })}>
              <option value="Each">Each</option>
              <option value="Kg">Kilogram (Kg)</option>
              <option value="Litre">Litre</option>
              <option value="Metre">Metre</option>
              <option value="Box">Box</option>
              <option value="Pack">Pack</option>
            </select>
          </div>
        </div>

        <div>
          <label className="label">Description *</label>
          <input className="input" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="Item description" required />
        </div>

        <div>
          <label className="label">Costing Method</label>
          <div className="grid grid-cols-2 gap-3">
            {[
              { value: 'Fifo', label: 'FIFO', desc: 'First In, First Out' },
              { value: 'WeightedAvgCost', label: 'Weighted Average', desc: 'Running average cost' },
            ].map(opt => (
              <label key={opt.value} className={`p-3 rounded-lg border cursor-pointer transition-colors ${form.costing_method === opt.value ? 'border-blue-500 bg-blue-50' : 'border-gray-200 hover:border-gray-300'}`}>
                <input type="radio" name="costing" value={opt.value} checked={form.costing_method === opt.value} onChange={(e) => setForm({ ...form, costing_method: e.target.value })} className="sr-only" />
                <p className="text-sm font-medium">{opt.label}</p>
                <p className="text-xs text-gray-500">{opt.desc}</p>
              </label>
            ))}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Inventory GL Account</label>
            <select className="input" value={form.gl_inventory} onChange={(e) => setForm({ ...form, gl_inventory: e.target.value })}>
              <option value="1300">1300 — Inventory</option>
              <option value="1310">1310 — Raw Materials</option>
              <option value="1320">1320 — Work in Progress</option>
              <option value="1330">1330 — Finished Goods</option>
            </select>
          </div>
          <div>
            <label className="label">COGS GL Account</label>
            <select className="input" value={form.gl_cogs} onChange={(e) => setForm({ ...form, gl_cogs: e.target.value })}>
              <option value="6000">6000 — Cost of Goods Sold</option>
              <option value="6100">6100 — Direct Materials</option>
              <option value="6200">6200 — Direct Labour</option>
            </select>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Reorder Point</label>
            <input type="number" className="input" value={form.reorder_point} onChange={(e) => setForm({ ...form, reorder_point: e.target.value })} placeholder="e.g. 10" />
            <p className="text-xs text-gray-400 mt-1">Alert when stock falls below this</p>
          </div>
          <div>
            <label className="label">Reorder Quantity</label>
            <input type="number" className="input" value={form.reorder_quantity} onChange={(e) => setForm({ ...form, reorder_quantity: e.target.value })} placeholder="e.g. 50" />
          </div>
        </div>

        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.sku || !form.description}>
            {mutation.isPending ? 'Saving...' : 'Save Item'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function ReceiveStockModal({ items, onClose }: { items: InventoryItem[]; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ item_id: '', quantity: '', unit_cost: '' });

  const mutation = useMutation({
    mutationFn: (data: any) => receiveInventory(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['inventory'] }); onClose(); },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      item_id: form.item_id,
      quantity: parseFloat(form.quantity),
      unit_cost: parseFloat(form.unit_cost),
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Receive Stock" subtitle="Record incoming inventory">
      <form onSubmit={handleSubmit} className="space-y-5">
        <div>
          <label className="label">Item *</label>
          <select className="input" value={form.item_id} onChange={(e) => setForm({ ...form, item_id: e.target.value })} required>
            <option value="">Select item...</option>
            {items.map(item => (
              <option key={item.id} value={item.id}>{item.sku} — {item.description}</option>
            ))}
          </select>
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Quantity *</label>
            <input type="number" step="0.01" className="input" value={form.quantity} onChange={(e) => setForm({ ...form, quantity: e.target.value })} placeholder="0" required />
          </div>
          <div>
            <label className="label">Unit Cost (KES) *</label>
            <input type="number" step="0.01" className="input" value={form.unit_cost} onChange={(e) => setForm({ ...form, unit_cost: e.target.value })} placeholder="0.00" required />
          </div>
        </div>
        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-success" disabled={mutation.isPending || !form.item_id || !form.quantity || !form.unit_cost}>
            {mutation.isPending ? 'Recording...' : 'Receive'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function IssueStockModal({ items, onClose }: { items: InventoryItem[]; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ item_id: '', quantity: '' });

  const mutation = useMutation({
    mutationFn: (data: any) => issueInventory(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['inventory'] }); onClose(); },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      item_id: form.item_id,
      quantity: parseFloat(form.quantity),
    });
  };

  const selectedItem = items.find(i => i.id === form.item_id);

  return (
    <Modal open={true} onClose={onClose} title="Issue Stock" subtitle="Record stock going out">
      <form onSubmit={handleSubmit} className="space-y-5">
        <div>
          <label className="label">Item *</label>
          <select className="input" value={form.item_id} onChange={(e) => setForm({ ...form, item_id: e.target.value })} required>
            <option value="">Select item...</option>
            {items.map(item => (
              <option key={item.id} value={item.id}>{item.sku} — {item.description} (Avail: {item.available})</option>
            ))}
          </select>
          {selectedItem && (
            <p className="text-xs text-gray-500 mt-1">Available: {formatNumber(selectedItem.available)} {selectedItem.uom}</p>
          )}
        </div>
        <div>
          <label className="label">Quantity *</label>
          <input type="number" step="0.01" className="input" value={form.quantity} onChange={(e) => setForm({ ...form, quantity: e.target.value })} placeholder="0" required />
        </div>
        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-danger" disabled={mutation.isPending || !form.item_id || !form.quantity}>
            {mutation.isPending ? 'Recording...' : 'Issue'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function AdjustStockModal({ items, onClose }: { items: InventoryItem[]; onClose: () => void }) {
  const qc = useQueryClient();
  const { data: accounts = [] } = useQuery<any[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then(r => Array.isArray(r.data) ? r.data : []) });
  const adjAccounts = accounts.filter((a) => a.account_type === 'Expense' || a.account_type === 'Revenue');
  const [itemId, setItemId] = useState('');
  const [counted, setCounted] = useState('');
  const [account, setAccount] = useState('');
  const [reason, setReason] = useState('');

  const item = items.find((i) => i.id === itemId);
  const variance = item ? Number(counted || 0) - Number(item.on_hand) : 0;

  const mut = useMutation({
    mutationFn: () => adjustInventory({ item_id: itemId, counted_quantity: Number(counted), adjustment_account: account, reason: reason || undefined }),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['inventory'] }); onClose(); },
  });

  return (
    <Modal open title="Stock take adjustment" onClose={onClose}>
      <div className="space-y-3">
        <div>
          <label className="label">Item</label>
          <select className="input w-full" value={itemId} onChange={(e) => { setItemId(e.target.value); setCounted(''); }}>
            <option value="">Select item…</option>
            {items.map((i) => <option key={i.id} value={i.id}>{i.sku} — {i.description}</option>)}
          </select>
        </div>
        {item && (
          <div className="grid grid-cols-3 gap-3 text-sm">
            <div><p className="label">System on-hand</p><p className="font-medium">{formatNumber(item.on_hand)}</p></div>
            <div><p className="label">Counted</p><input type="number" step="0.01" className="input w-full" value={counted} onChange={(e) => setCounted(e.target.value)} /></div>
            <div><p className="label">Variance</p><p className={`font-medium ${variance < 0 ? 'text-red-600' : variance > 0 ? 'text-green-700' : ''}`}>{formatNumber(variance)}</p></div>
          </div>
        )}
        <div>
          <label className="label">Adjustment account (gain/loss)</label>
          <select className="input w-full" value={account} onChange={(e) => setAccount(e.target.value)}>
            <option value="">Select account…</option>
            {adjAccounts.map((a) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
          </select>
        </div>
        <div><label className="label">Reason</label><input className="input w-full" value={reason} onChange={(e) => setReason(e.target.value)} placeholder="e.g. Annual stock count" /></div>
        {mut.isError && <p className="text-sm text-red-600">{(mut.error as any)?.response?.data?.error ?? 'Failed'}</p>}
        <div className="flex justify-end gap-2 pt-2">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!itemId || counted === '' || !account || variance === 0 || mut.isPending} onClick={() => mut.mutate()}>
            {mut.isPending ? 'Adjusting…' : 'Post adjustment'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
