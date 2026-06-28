import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getRecurringInvoices, createRecurringInvoice, deleteRecurringInvoice, getCustomers, getProducts, getRecurringDocumentPdf, getRecurringInvoiceHistory } from '../../api/client';
import type { Customer, Product } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, RefreshCw, Calendar, Pause, Play, Trash2, AlertCircle, Eye, Download, History, Loader2 } from 'lucide-react';

interface RecurringInvoice {
  id: string;
  entity_id: string;
  customer_id: string;
  frequency: string;
  start_date: string;
  end_date?: string;
  next_run: string;
  last_run?: string;
  auto_send: boolean;
  auto_charge: boolean;
  run_count: number;
  is_active: boolean;
  created_at: string;
}

export default function RecurringInvoicesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [historyFor, setHistoryFor] = useState<string | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const downloadPdf = async (id: string) => {
    setDownloadingId(id);
    try {
      const r = await getRecurringDocumentPdf(id);
      const url = URL.createObjectURL(new Blob([r.data], { type: 'application/pdf' }));
      const a = document.createElement('a');
      a.href = url;
      a.download = 'recurring-invoice-preview.pdf';
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 4000);
    } catch { /* no-op */ } finally {
      setDownloadingId(null);
    }
  };

  const { data: recurring = [], isLoading } = useQuery<RecurringInvoice[]>({
    queryKey: ['recurring-invoices'],
    queryFn: () => getRecurringInvoices().then(r => Array.isArray(r.data) ? r.data : []),
  });

  const { data: customers = [] } = useQuery<Customer[]>({
    queryKey: ['customers'],
    queryFn: () => getCustomers().then(r => Array.isArray(r.data) ? r.data : []),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteRecurringInvoice(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['recurring-invoices'] }),
  });

  const getCustomerName = (customerId: string) => {
    const c = customers.find(cust => cust.id === customerId);
    return c?.name || customerId.slice(0, 8) + '...';
  };

  const columns: Column<RecurringInvoice>[] = [
    {
      key: 'is_active', header: 'Status',
      render: (r) => (
        <span className={r.is_active ? 'badge-success' : 'badge-gray'}>
          {r.is_active ? 'Active' : 'Paused'}
        </span>
      ),
    },
    {
      key: 'customer_id', header: 'Customer',
      render: (r) => <span className="font-medium">{getCustomerName(r.customer_id)}</span>,
    },
    {
      key: 'frequency', header: 'Frequency',
      render: (r) => (
        <span className="flex items-center gap-1.5 text-sm">
          <RefreshCw className="w-3.5 h-3.5 text-gray-400" />
          {r.frequency}
        </span>
      ),
    },
    {
      key: 'next_run', header: 'Next Run',
      render: (r) => (
        <span className="flex items-center gap-1.5">
          <Calendar className="w-3.5 h-3.5 text-gray-400" />
          {formatDate(r.next_run)}
        </span>
      ),
    },
    {
      key: 'last_run', header: 'Last Run',
      render: (r) => r.last_run ? formatDate(r.last_run) : <span className="text-gray-400">Never</span>,
    },
    { key: 'run_count', header: 'Runs', render: (r) => <span className="font-medium">{r.run_count}</span> },
    {
      key: 'auto_send', header: 'Auto Send',
      render: (r) => r.auto_send ? <span className="text-green-600 text-xs font-medium">Yes</span> : <span className="text-gray-400 text-xs">No</span>,
    },
    {
      key: 'id', header: '',
      render: (r) => (
        <div className="flex items-center justify-end gap-1">
          <Link
            to={`/documents/recurring/${r.id}`}
            onClick={(e) => e.stopPropagation()}
            className="btn-secondary text-xs py-1 px-2"
            title="Preview next invoice"
          >
            <Eye className="w-3 h-3" />
          </Link>
          <button
            onClick={(e) => { e.stopPropagation(); downloadPdf(r.id); }}
            disabled={downloadingId === r.id}
            className="btn-secondary text-xs py-1 px-2"
            title="Download preview PDF"
          >
            {downloadingId === r.id ? <Loader2 className="w-3 h-3 animate-spin" /> : <Download className="w-3 h-3" />}
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); setHistoryFor(r.id); }}
            className="btn-secondary text-xs py-1 px-2"
            title="Generated invoices"
          >
            <History className="w-3 h-3" /> {r.run_count}
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              if (confirm('Delete this recurring schedule? This stops future automatic invoices.')) {
                deleteMutation.mutate(r.id);
              }
            }}
            className="btn-secondary text-xs py-1 px-2 text-red-600 border-red-200 hover:bg-red-50"
            title="Delete schedule"
          >
            <Trash2 className="w-3 h-3" />
          </button>
        </div>
      ),
      className: 'text-right',
    },
  ];

  return (
    <div>
      <PageHeader
        title="Recurring Invoices"
        subtitle="Automate invoice generation on a schedule"
        actions={
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> Create Schedule
          </button>
        }
      />

      {/* Stats */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
        <div className="card p-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-green-50 flex items-center justify-center">
              <Play className="w-5 h-5 text-green-600" />
            </div>
            <div>
              <p className="text-2xl font-bold">{recurring.filter(r => r.is_active).length}</p>
              <p className="text-xs text-gray-500">Active schedules</p>
            </div>
          </div>
        </div>
        <div className="card p-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-gray-100 flex items-center justify-center">
              <Pause className="w-5 h-5 text-gray-500" />
            </div>
            <div>
              <p className="text-2xl font-bold">{recurring.filter(r => !r.is_active).length}</p>
              <p className="text-xs text-gray-500">Paused</p>
            </div>
          </div>
        </div>
        <div className="card p-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-blue-50 flex items-center justify-center">
              <RefreshCw className="w-5 h-5 text-blue-600" />
            </div>
            <div>
              <p className="text-2xl font-bold">{recurring.reduce((s, r) => s + r.run_count, 0)}</p>
              <p className="text-xs text-gray-500">Total invoices created</p>
            </div>
          </div>
        </div>
      </div>

      <DataTable
        columns={columns}
        data={recurring}
        loading={isLoading}
        onRowClick={(r) => navigate(`/documents/recurring/${r.id}`)}
        emptyMessage="No recurring invoices set up yet. Create one to automate your billing."
      />

      {showCreate && <CreateRecurringModal onClose={() => setShowCreate(false)} />}
      {historyFor && (
        <HistoryModal
          recurringId={historyFor}
          customerName={getCustomerName(recurring.find(r => r.id === historyFor)?.customer_id || '')}
          onClose={() => setHistoryFor(null)}
        />
      )}
    </div>
  );
}

interface HistoryItem { id: string; number: string; issue_date: string; status: string; gross_total: number; balance_due: number; }

function HistoryModal({ recurringId, customerName, onClose }: { recurringId: string; customerName: string; onClose: () => void }) {
  const navigate = useNavigate();
  const { data: items = [], isLoading } = useQuery<HistoryItem[]>({
    queryKey: ['recurring-history', recurringId],
    queryFn: () => getRecurringInvoiceHistory(recurringId).then(r => Array.isArray(r.data) ? r.data : []),
  });

  return (
    <Modal open={true} onClose={onClose} title="Generated Invoices" subtitle={`Invoices created by this schedule${customerName ? ' · ' + customerName : ''}`} size="lg">
      {isLoading ? (
        <div className="p-8 text-center text-gray-400 text-sm">Loading…</div>
      ) : items.length === 0 ? (
        <div className="p-8 text-center text-gray-400 text-sm">
          No invoices have been generated by this schedule yet. They appear here automatically on each run.
        </div>
      ) : (
        <div className="divide-y border rounded-lg overflow-hidden">
          {items.map((inv) => (
            <button
              key={inv.id}
              onClick={() => { onClose(); navigate(`/invoices/${inv.id}`); }}
              className="w-full px-4 py-3 flex items-center justify-between hover:bg-gray-50 transition-colors text-left"
            >
              <div className="flex items-center gap-3 min-w-0">
                <span className={statusColor(inv.status)}>{inv.status.replace('_', ' ')}</span>
                <span className="font-medium text-blue-600">{inv.number}</span>
                <span className="text-sm text-gray-500">{formatDate(inv.issue_date)}</span>
              </div>
              <div className="text-right shrink-0 ml-3">
                <p className="text-sm font-medium text-gray-900">{formatCurrency(inv.gross_total)}</p>
                {Number(inv.balance_due) > 0 && <p className="text-xs text-gray-500">{formatCurrency(inv.balance_due)} due</p>}
              </div>
            </button>
          ))}
        </div>
      )}
    </Modal>
  );
}

function CreateRecurringModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data: customers = [] } = useQuery<Customer[]>({ queryKey: ['customers'], queryFn: () => getCustomers().then(r => Array.isArray(r.data) ? r.data : []) });
  const { data: products = [] } = useQuery<Product[]>({ queryKey: ['products'], queryFn: () => getProducts().then(r => Array.isArray(r.data) ? r.data : []) });

  const today = new Date().toISOString().split('T')[0];

  const [form, setForm] = useState({
    customer_id: '',
    frequency: 'Monthly',
    start_date: today,
    end_date: '',
    auto_send: true,
    lines: [emptyLine()],
  });

  function emptyLine() {
    return { product_id: '', description: '', quantity: 1, unit_price: 0, account_code: '5000', vat_treatment: 'Standard16' };
  }

  const [error, setError] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: (data: any) => createRecurringInvoice(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['recurring-invoices'] });
      onClose();
    },
    onError: (e: any) => {
      setError(e?.response?.data?.error || e?.response?.data?.message || 'Failed to create recurring invoice.');
    },
  });

  const addLine = () => setForm({ ...form, lines: [...form.lines, emptyLine()] });

  const updateLine = (i: number, field: string, value: any) => {
    const lines = [...form.lines];
    (lines[i] as any)[field] = value;

    if (field === 'product_id' && value) {
      const product = products.find(p => p.id === value);
      if (product) {
        lines[i].description = product.description || product.name;
        lines[i].unit_price = product.unit_price || 0;
        lines[i].account_code = product.sales_account;
      }
    }
    setForm({ ...form, lines });
  };

  const removeLine = (i: number) => {
    if (form.lines.length === 1) return;
    setForm({ ...form, lines: form.lines.filter((_, idx) => idx !== i) });
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    mutation.mutate({
      customer_id: form.customer_id,
      frequency: form.frequency,
      start_date: form.start_date,
      end_date: form.end_date || undefined,
      auto_send: form.auto_send,
      template: {
        customer_id: form.customer_id,
        lines: form.lines.map(l => ({
          product_id: l.product_id || undefined,
          description: l.description,
          quantity: l.quantity,
          unit_price: l.unit_price,
          account_code: l.account_code,
          vat_treatment: l.vat_treatment,
        })),
      },
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Create Recurring Invoice" subtitle="Set up automatic invoice generation" size="xl">
      <form onSubmit={handleSubmit} className="space-y-5">
        {error && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 text-red-700 text-sm">
            <AlertCircle className="w-4 h-4 shrink-0" /><span>{error}</span>
          </div>
        )}
        {/* Customer & Frequency */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label className="label">Customer *</label>
            <select className="input" value={form.customer_id} onChange={(e) => setForm({ ...form, customer_id: e.target.value })} required>
              <option value="">Select customer...</option>
              {customers.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Frequency *</label>
            <select className="input" value={form.frequency} onChange={(e) => setForm({ ...form, frequency: e.target.value })}>
              <option value="Weekly">Weekly</option>
              <option value="Biweekly">Biweekly</option>
              <option value="Monthly">Monthly</option>
              <option value="Quarterly">Quarterly</option>
              <option value="SemiAnnual">Semi-Annual</option>
              <option value="Annual">Annual</option>
            </select>
          </div>
        </div>

        {/* Dates */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label className="label">Start Date *</label>
            <input type="date" className="input" value={form.start_date} onChange={(e) => setForm({ ...form, start_date: e.target.value })} required />
          </div>
          <div>
            <label className="label">End Date <span className="text-gray-400 font-normal">(optional)</span></label>
            <input type="date" className="input" value={form.end_date} onChange={(e) => setForm({ ...form, end_date: e.target.value })} />
          </div>
        </div>

        {/* Auto-send toggle */}
        <label className="flex items-center gap-2 text-sm cursor-pointer">
          <input type="checkbox" checked={form.auto_send} onChange={(e) => setForm({ ...form, auto_send: e.target.checked })} className="rounded" />
          <span>Automatically send invoice to customer on creation</span>
        </label>

        {/* Line Items */}
        <div>
          <label className="label">Invoice Lines</label>
          <div className="border rounded-lg overflow-hidden">
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-3">Product</div>
              <div className="col-span-4">Description</div>
              <div className="col-span-1">Qty</div>
              <div className="col-span-2">Price</div>
              <div className="col-span-1">Amount</div>
              <div className="col-span-1"></div>
            </div>
            {form.lines.map((line, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-3">
                  <select className="input text-sm py-1.5" value={line.product_id} onChange={(e) => updateLine(i, 'product_id', e.target.value)}>
                    <option value="">Select...</option>
                    {products.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                </div>
                <div className="col-span-4">
                  <input className="input text-sm py-1.5" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} placeholder="Description" />
                </div>
                <div className="col-span-1">
                  <input className="input text-sm py-1.5 text-center" type="number" min="1" step="0.01" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} />
                </div>
                <div className="col-span-2">
                  <input className="input text-sm py-1.5" type="number" min="0" step="0.01" value={line.unit_price} onChange={(e) => updateLine(i, 'unit_price', +e.target.value)} />
                </div>
                <div className="col-span-1 text-sm font-medium text-right">
                  {formatCurrency(line.quantity * line.unit_price)}
                </div>
                <div className="col-span-1 text-center">
                  <button type="button" onClick={() => removeLine(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={form.lines.length === 1}>×</button>
                </div>
              </div>
            ))}
          </div>
          <button type="button" onClick={addLine} className="mt-2 text-sm font-medium text-blue-600 hover:text-blue-800">
            + Add a Line
          </button>
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.customer_id}>
            {mutation.isPending ? 'Creating...' : 'Create Schedule'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
