import { useState } from 'react';
import { useToast } from '../../components/toast/ToastProvider';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getFxRates, upsertFxRate, deleteFxRate, runFxRevaluation, syncCbkRates } from '../../api/client';
import type { ExchangeRateEntry } from '../../types';
import { formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, RefreshCw, Pencil, Trash2, Download } from 'lucide-react';

export default function FxRatesPage() {
  const [showForm, setShowForm] = useState(false);
  const [editing, setEditing] = useState<ExchangeRateEntry | null>(null);
  const queryClient = useQueryClient();

  const { data: rates = [], isLoading } = useQuery<ExchangeRateEntry[]>({
    queryKey: ['fx-rates'],
    queryFn: () => getFxRates().then(r => Array.isArray(r.data) ? r.data : []),
  });

  const toast = useToast();
  const cbkMutation = useMutation({
    mutationFn: () => syncCbkRates(),
    onSuccess: (r) => {
      queryClient.invalidateQueries({ queryKey: ['fx-rates'] });
      const d = r.data;
      toast.success(`Loaded ${d.updated} CBK rate${d.updated === 1 ? '' : 's'} (as at ${d.date}).`);
    },
    onError: (e: any) => toast.fromError(e, 'Could not load CBK rates.'),
  });

  const revalMutation = useMutation({
    mutationFn: () => runFxRevaluation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['fx-rates'] });
      queryClient.invalidateQueries({ queryKey: ['journal-entries'] });
      toast.success('FX revaluation posted, with an auto-reversal in the next period.');
    },
    onError: (e: any) => toast.fromError(e, 'FX revaluation failed.'),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteFxRate(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['fx-rates'] }),
    onError: (e: any) => toast.fromError(e, 'Failed to delete rate.'),
  });

  const openCreate = () => { setEditing(null); setShowForm(true); };
  const openEdit = (r: ExchangeRateEntry) => { setEditing(r); setShowForm(true); };
  const handleDelete = (r: ExchangeRateEntry) => {
    if (confirm(`Delete the ${r.from_ccy}→${r.to_ccy} rate dated ${formatDate(r.rate_date)}?`)) {
      deleteMutation.mutate(r.id);
    }
  };

  const columns: Column<ExchangeRateEntry>[] = [
    {
      key: 'from_ccy', header: 'From',
      render: (r) => <span className="font-mono font-medium">{r.from_ccy}</span>,
    },
    {
      key: 'to_ccy', header: 'To',
      render: (r) => <span className="font-mono font-medium">{r.to_ccy}</span>,
    },
    {
      key: 'rate_date', header: 'Date',
      render: (r) => formatDate(r.rate_date),
    },
    {
      key: 'rate_type', header: 'Type',
      render: (r) => (
        <span className={r.rate_type === 'Spot' ? 'badge-info' : r.rate_type === 'Revaluation' ? 'badge-warning' : 'badge-gray'}>
          {r.rate_type}
        </span>
      ),
    },
    {
      key: 'rate', header: 'Rate',
      // `rate` arrives as a string (backend serialises Decimal as a JSON string),
      // so coerce before formatting — calling .toFixed on a string throws.
      render: (r) => <span className="font-mono">{Number(r.rate).toFixed(4)}</span>,
      className: 'text-right',
    },
    {
      key: 'source', header: 'Source',
      render: (r) => <span className="text-gray-500">{r.source}</span>,
    },
    {
      key: 'actions', header: '',
      className: 'text-right',
      render: (r) => (
        <div className="flex items-center justify-end gap-1">
          <button
            onClick={(e) => { e.stopPropagation(); openEdit(r); }}
            className="p-1.5 text-gray-400 hover:text-blue-600 hover:bg-blue-50 rounded"
            title="Edit rate"
          >
            <Pencil className="w-4 h-4" />
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); handleDelete(r); }}
            className="p-1.5 text-gray-400 hover:text-red-600 hover:bg-red-50 rounded"
            title="Delete rate"
            disabled={deleteMutation.isPending}
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      ),
    },
  ];

  return (
    <div>
      <PageHeader
        title="Exchange Rates"
        subtitle="Manage FX rates for multi-currency transactions and period-end revaluation"
        actions={
          <>
            <button
              onClick={() => cbkMutation.mutate()}
              className="btn-secondary"
              disabled={cbkMutation.isPending}
              title="Auto-load today's Central Bank of Kenya indicative rates"
            >
              <Download className={`w-4 h-4 ${cbkMutation.isPending ? 'animate-pulse' : ''}`} />
              {cbkMutation.isPending ? 'Loading…' : 'Load CBK rates'}
            </button>
            <button
              onClick={() => revalMutation.mutate()}
              className="btn-secondary"
              disabled={revalMutation.isPending}
            >
              <RefreshCw className={`w-4 h-4 ${revalMutation.isPending ? 'animate-spin' : ''}`} />
              {revalMutation.isPending ? 'Running...' : 'Run Revaluation'}
            </button>
            <button onClick={openCreate} className="btn-primary">
              <Plus className="w-4 h-4" /> Add Rate
            </button>
          </>
        }
      />
      <DataTable
        columns={columns}
        data={rates}
        loading={isLoading}
        emptyMessage="No exchange rates. Add rates for multi-currency support."
        onRowClick={openEdit}
      />
      {showForm && <RateModal rate={editing} onClose={() => setShowForm(false)} />}
    </div>
  );
}

const CURRENCIES = ['USD', 'EUR', 'GBP', 'KES'];

function RateModal({ rate, onClose }: { rate: ExchangeRateEntry | null; onClose: () => void }) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const isEdit = !!rate;
  const [form, setForm] = useState({
    // Default foreign → local (KES base): the common case is recording a
    // foreign currency's value in shillings, and the posting engine looks up
    // from=foreign, to=base.
    from_ccy: rate?.from_ccy ?? 'USD',
    to_ccy: rate?.to_ccy ?? 'KES',
    rate_date: rate?.rate_date ?? new Date().toISOString().split('T')[0],
    rate: rate ? String(rate.rate) : '',
    rate_type: rate?.rate_type ?? 'Spot',
    source: rate?.source ?? 'Manual',
  });

  const mutation = useMutation({
    mutationFn: (data: any) => upsertFxRate(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['fx-rates'] }); onClose(); },
    onError: (e: any) => toast.fromError(e, 'Failed to save rate.'),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      from_ccy: form.from_ccy,
      to_ccy: form.to_ccy,
      rate_date: form.rate_date,
      rate: parseFloat(form.rate),
      rate_type: form.rate_type,
      source: form.source,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title={isEdit ? 'Edit Exchange Rate' : 'Add Exchange Rate'}>
      <form onSubmit={handleSubmit} className="space-y-5">
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">From Currency</label>
            <select className="input font-mono" value={form.from_ccy} onChange={(e) => setForm({ ...form, from_ccy: e.target.value })}>
              {CURRENCIES.map((c) => <option key={c} value={c}>{c}</option>)}
            </select>
          </div>
          <div>
            <label className="label">To Currency</label>
            <select className="input font-mono" value={form.to_ccy} onChange={(e) => setForm({ ...form, to_ccy: e.target.value })}>
              {CURRENCIES.map((c) => <option key={c} value={c}>{c}</option>)}
            </select>
          </div>
        </div>

        <p className="text-xs text-gray-500 -mt-2">
          1 {form.from_ccy} = <span className="font-mono">{form.rate || '…'}</span> {form.to_ccy}.
          Record foreign → base (e.g. USD → KES) so posting and revaluation resolve correctly.
        </p>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Rate Date *</label>
            <input type="date" className="input" value={form.rate_date} onChange={(e) => setForm({ ...form, rate_date: e.target.value })} required />
          </div>
          <div>
            <label className="label">Rate *</label>
            <input type="number" step="0.0001" className="input font-mono" value={form.rate} onChange={(e) => setForm({ ...form, rate: e.target.value })} placeholder="e.g. 129.2155" required />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Rate Type</label>
            <select className="input" value={form.rate_type} onChange={(e) => setForm({ ...form, rate_type: e.target.value })}>
              <option value="Spot">Spot</option>
              <option value="Revaluation">Revaluation</option>
              <option value="Budget">Budget</option>
            </select>
          </div>
          <div>
            <label className="label">Source</label>
            <input className="input" value={form.source} onChange={(e) => setForm({ ...form, source: e.target.value })} placeholder="e.g. CBK, Manual, Reuters" />
          </div>
        </div>

        {isEdit && (
          <p className="text-xs text-gray-500">
            Saving updates the rate for this From/To/Date/Type combination (upsert).
          </p>
        )}

        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.rate}>
            {mutation.isPending ? 'Saving...' : 'Save Rate'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
