import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getAssets, createAsset, runDepreciation } from '../../api/client';
import type { FixedAsset } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, Calculator, Building } from 'lucide-react';

const KRA_RATES: Record<string, string> = {
  'KRA Tax Class 1': '37.5% — Computers & accessories',
  'KRA Tax Class 2': '30% — Motor vehicles, plant & machinery',
  'KRA Tax Class 3': '25% — Other machinery',
  'KRA Tax Class 4': '12.5% — Buildings',
};

export default function AssetsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const queryClient = useQueryClient();

  const { data: assets = [], isLoading } = useQuery<FixedAsset[]>({
    queryKey: ['assets'],
    queryFn: () => getAssets().then(r => r.data),
  });

  const depreciationMutation = useMutation({
    mutationFn: () => runDepreciation(),
    onSuccess: (res: any) => {
      queryClient.invalidateQueries({ queryKey: ['assets'] });
      queryClient.invalidateQueries({ queryKey: ['journal-entries'] });
      const n = res?.data?.depreciated ?? 0;
      alert(n > 0 ? `Posted depreciation for ${n} asset${n === 1 ? '' : 's'}.` : 'No assets were due for depreciation.');
    },
    onError: (e: any) => alert(e?.response?.data?.error || 'Depreciation run failed.'),
  });

  const columns: Column<FixedAsset>[] = [
    {
      key: 'asset_number', header: 'Asset #',
      render: (r) => <span className="font-mono text-xs font-medium">{r.asset_number}</span>,
    },
    {
      key: 'description', header: 'Description',
      render: (r) => <span className="font-medium text-gray-900">{r.description}</span>,
    },
    {
      key: 'category', header: 'Category',
      render: (r) => <span className="badge-info">{r.category}</span>,
    },
    {
      key: 'acquisition_date', header: 'Acquired',
      render: (r) => formatDate(r.acquisition_date),
    },
    {
      key: 'cost', header: 'Cost',
      render: (r) => formatCurrency(r.cost),
      className: 'text-right',
    },
    {
      key: 'accumulated_depreciation', header: 'Accum. Depr.',
      render: (r) => formatCurrency(r.accumulated_depreciation),
      className: 'text-right',
    },
    {
      key: 'net_book_value', header: 'NBV',
      render: (r) => <span className="font-semibold">{formatCurrency(r.net_book_value)}</span>,
      className: 'text-right',
    },
    {
      key: 'status', header: 'Status',
      render: (r) => (
        <span className={r.status === 'Active' ? 'badge-success' : r.status === 'Disposed' ? 'badge-gray' : 'badge-warning'}>
          {r.status}
        </span>
      ),
    },
  ];

  return (
    <div>
      <PageHeader
        title="Fixed Assets"
        subtitle="Manage capital assets, depreciation, and KRA tax classes"
        actions={
          <>
            <button
              onClick={() => depreciationMutation.mutate()}
              className="btn-secondary"
              disabled={depreciationMutation.isPending}
            >
              <Calculator className="w-4 h-4" />
              {depreciationMutation.isPending ? 'Running...' : 'Run Depreciation'}
            </button>
            <button onClick={() => setShowCreate(true)} className="btn-primary">
              <Plus className="w-4 h-4" /> Add Asset
            </button>
          </>
        }
      />

      {/* KRA Rate Reference */}
      <div className="card mb-4 p-4">
        <div className="flex items-center gap-2 mb-2">
          <Building className="w-4 h-4 text-gray-500" />
          <span className="text-xs font-semibold text-gray-500 uppercase tracking-wide">KRA Wear & Tear Rates</span>
        </div>
        <div className="grid grid-cols-4 gap-3">
          {Object.entries(KRA_RATES).map(([cls, desc]) => (
            <div key={cls} className="text-xs">
              <span className="font-medium text-gray-900">{cls}</span>
              <span className="text-gray-500 ml-1">— {desc}</span>
            </div>
          ))}
        </div>
      </div>

      <DataTable
        columns={columns}
        data={assets}
        loading={isLoading}
        emptyMessage="No assets registered. Add capital items to track depreciation."
      />
      {showCreate && <CreateAssetModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateAssetModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    description: '',
    category: 'Computer Equipment',
    acquisition_date: '',
    cost: '',
    residual_value: '0',
    depreciation_method: 'StraightLine',
    useful_life_months: '60',
    gl_asset_account: '1500',
    gl_accum_depr_account: '1510',
    gl_depr_expense: '7100',
  });

  const mutation = useMutation({
    mutationFn: (data: any) => createAsset(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['assets'] }); onClose(); },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    // Build correct depreciation_method structure for the backend
    let depreciation_method: any;
    switch (form.depreciation_method) {
      case 'StraightLine':
        depreciation_method = 'StraightLine';
        break;
      case 'DecliningBalance':
        depreciation_method = { DecliningBalance: { rate_percent: 20 } };
        break;
      case 'KRATaxClass1':
        depreciation_method = { KraTax: { class: 'Class1' } };
        break;
      case 'KRATaxClass2':
        depreciation_method = { KraTax: { class: 'Class2' } };
        break;
      case 'KRATaxClass3':
        depreciation_method = { KraTax: { class: 'Class3' } };
        break;
      case 'KRATaxClass4':
        depreciation_method = { KraTax: { class: 'Class4' } };
        break;
      default:
        depreciation_method = 'StraightLine';
    }
    mutation.mutate({
      description: form.description,
      category: form.category,
      acquisition_date: form.acquisition_date,
      cost: parseFloat(form.cost),
      residual_value: parseFloat(form.residual_value),
      depreciation_method,
      useful_life_months: parseInt(form.useful_life_months),
      gl_asset_account: form.gl_asset_account,
      gl_accum_depr_account: form.gl_accum_depr_account,
      gl_depr_expense: form.gl_depr_expense,
    });
  };

  const categories = ['Land & Buildings', 'Motor Vehicles', 'Plant & Machinery', 'Furniture & Fittings', 'Computer Equipment'];
  const methods = [
    { value: 'StraightLine', label: 'Straight Line' },
    { value: 'DecliningBalance', label: 'Declining Balance' },
    { value: 'KRATaxClass1', label: 'KRA Tax Class 1 (37.5%)' },
    { value: 'KRATaxClass2', label: 'KRA Tax Class 2 (30%)' },
    { value: 'KRATaxClass3', label: 'KRA Tax Class 3 (25%)' },
    { value: 'KRATaxClass4', label: 'KRA Tax Class 4 (12.5%)' },
  ];

  return (
    <Modal open={true} onClose={onClose} title="Register Fixed Asset" size="lg">
      <form onSubmit={handleSubmit} className="space-y-5">
        <div>
          <label className="label">Description *</label>
          <input className="input" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="e.g. Dell OptiPlex 7090, Toyota Hilux" required />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Category *</label>
            <select className="input" value={form.category} onChange={(e) => setForm({ ...form, category: e.target.value })}>
              {categories.map(c => <option key={c} value={c}>{c}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Acquisition Date *</label>
            <input type="date" className="input" value={form.acquisition_date} onChange={(e) => setForm({ ...form, acquisition_date: e.target.value })} required />
          </div>
        </div>

        <div className="grid grid-cols-3 gap-4">
          <div>
            <label className="label">Cost (KES) *</label>
            <input type="number" step="0.01" className="input" value={form.cost} onChange={(e) => setForm({ ...form, cost: e.target.value })} placeholder="0.00" required />
          </div>
          <div>
            <label className="label">Residual Value</label>
            <input type="number" step="0.01" className="input" value={form.residual_value} onChange={(e) => setForm({ ...form, residual_value: e.target.value })} placeholder="0.00" />
          </div>
          <div>
            <label className="label">Useful Life (months)</label>
            <input type="number" className="input" value={form.useful_life_months} onChange={(e) => setForm({ ...form, useful_life_months: e.target.value })} placeholder="60" />
          </div>
        </div>

        <div>
          <label className="label">Depreciation Method *</label>
          <select className="input" value={form.depreciation_method} onChange={(e) => setForm({ ...form, depreciation_method: e.target.value })}>
            {methods.map(m => <option key={m.value} value={m.value}>{m.label}</option>)}
          </select>
          {form.depreciation_method.startsWith('KRATaxClass') && (
            <p className="text-xs text-blue-600 mt-1">
              KRA rate: {form.depreciation_method === 'KRATaxClass1' ? '37.5%' : form.depreciation_method === 'KRATaxClass2' ? '30%' : form.depreciation_method === 'KRATaxClass3' ? '25%' : '12.5%'} per annum on reducing balance
            </p>
          )}
        </div>

        <div className="grid grid-cols-3 gap-4">
          <div>
            <label className="label">Asset Account</label>
            <select className="input" value={form.gl_asset_account} onChange={(e) => setForm({ ...form, gl_asset_account: e.target.value })}>
              <option value="1500">1500 — Fixed Assets</option>
              <option value="1510">1510 — Motor Vehicles</option>
              <option value="1520">1520 — Computer Equipment</option>
              <option value="1530">1530 — Furniture & Fittings</option>
            </select>
          </div>
          <div>
            <label className="label">Accum. Depr. Account</label>
            <select className="input" value={form.gl_accum_depr_account} onChange={(e) => setForm({ ...form, gl_accum_depr_account: e.target.value })}>
              <option value="1510">1510 — Accum Depr</option>
              <option value="1550">1550 — Accum Depr MV</option>
              <option value="1560">1560 — Accum Depr Comp</option>
            </select>
          </div>
          <div>
            <label className="label">Depr. Expense Account</label>
            <select className="input" value={form.gl_depr_expense} onChange={(e) => setForm({ ...form, gl_depr_expense: e.target.value })}>
              <option value="7100">7100 — Depreciation Expense</option>
              <option value="7110">7110 — Depr — Vehicles</option>
              <option value="7120">7120 — Depr — Equipment</option>
            </select>
          </div>
        </div>

        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.description || !form.cost || !form.acquisition_date}>
            {mutation.isPending ? 'Saving...' : 'Register Asset'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
