import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getFxRates, upsertFxRate, runFxRevaluation } from '../../api/client';
import type { ExchangeRateEntry } from '../../types';
import { formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, RefreshCw } from 'lucide-react';

export default function FxRatesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const queryClient = useQueryClient();

  const { data: rates = [], isLoading } = useQuery<ExchangeRateEntry[]>({
    queryKey: ['fx-rates'],
    queryFn: () => getFxRates().then(r => Array.isArray(r.data) ? r.data : []),
  });

  const revalMutation = useMutation({
    mutationFn: () => runFxRevaluation(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['fx-rates'] });
      queryClient.invalidateQueries({ queryKey: ['journal-entries'] });
      alert('FX revaluation posted, with an auto-reversal in the next period.');
    },
    onError: (e: any) => alert(e?.response?.data?.error || 'FX revaluation failed.'),
  });

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
      render: (r) => <span className="font-mono">{r.rate.toFixed(4)}</span>,
      className: 'text-right',
    },
    {
      key: 'source', header: 'Source',
      render: (r) => <span className="text-gray-500">{r.source}</span>,
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
              onClick={() => revalMutation.mutate()}
              className="btn-secondary"
              disabled={revalMutation.isPending}
            >
              <RefreshCw className={`w-4 h-4 ${revalMutation.isPending ? 'animate-spin' : ''}`} />
              {revalMutation.isPending ? 'Running...' : 'Run Revaluation'}
            </button>
            <button onClick={() => setShowCreate(true)} className="btn-primary">
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
      />
      {showCreate && <CreateRateModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateRateModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    from_ccy: 'KES',
    to_ccy: 'USD',
    rate_date: new Date().toISOString().split('T')[0],
    rate: '',
    rate_type: 'Spot',
    source: 'Manual',
  });

  const mutation = useMutation({
    mutationFn: (data: any) => upsertFxRate(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['fx-rates'] }); onClose(); },
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
    <Modal open={true} onClose={onClose} title="Add / Update Exchange Rate">
      <form onSubmit={handleSubmit} className="space-y-5">
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">From Currency</label>
            <select className="input font-mono" value={form.from_ccy} onChange={(e) => setForm({ ...form, from_ccy: e.target.value })}>
              <option value="KES">KES</option>
              <option value="USD">USD</option>
              <option value="EUR">EUR</option>
              <option value="GBP">GBP</option>
            </select>
          </div>
          <div>
            <label className="label">To Currency</label>
            <select className="input font-mono" value={form.to_ccy} onChange={(e) => setForm({ ...form, to_ccy: e.target.value })}>
              <option value="USD">USD</option>
              <option value="EUR">EUR</option>
              <option value="GBP">GBP</option>
              <option value="KES">KES</option>
            </select>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Rate Date *</label>
            <input type="date" className="input" value={form.rate_date} onChange={(e) => setForm({ ...form, rate_date: e.target.value })} required />
          </div>
          <div>
            <label className="label">Rate *</label>
            <input type="number" step="0.0001" className="input font-mono" value={form.rate} onChange={(e) => setForm({ ...form, rate: e.target.value })} placeholder="e.g. 153.2500" required />
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
