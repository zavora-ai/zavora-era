import { useState } from 'react';
import { useToast } from '../../components/toast/ToastProvider';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { getAssets, createAsset, runDepreciation, getAccounts } from '../../api/client';
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
    queryFn: () => getAssets().then(r => Array.isArray(r.data) ? r.data : []),
  });

  const toast = useToast();
  const depreciationMutation = useMutation({
    mutationFn: () => runDepreciation(),
    onSuccess: (res: any) => {
      queryClient.invalidateQueries({ queryKey: ['assets'] });
      queryClient.invalidateQueries({ queryKey: ['journal-entries'] });
      const n = res?.data?.depreciated ?? 0;
      toast.success(n > 0 ? `Posted depreciation for ${n} asset${n === 1 ? '' : 's'}.` : 'No assets were due for depreciation.');
    },
    onError: (e: any) => toast.fromError(e, 'Depreciation run failed.'),
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
      render: (r) => <span className="badge-info">{String(r.category).replace(/"/g, '').replace(/([a-z])([A-Z])/g, '$1 $2')}</span>,
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
    category: 'ComputerEquipment',
    acquisition_date: '',
    cost: '',
    residual_value: '0',
    depreciation_method: 'StraightLine',
    useful_life_months: '60',
    gl_asset_account: '',
    gl_accum_depr_account: '',
    gl_depr_expense: '',
  });

  // Populate the GL account dropdowns from the real chart of accounts — the
  // previously hardcoded codes did not exist in the seeded chart, so assets
  // posted depreciation to phantom accounts.
  const { data: accountsRes } = useQuery({ queryKey: ['accounts'], queryFn: getAccounts });
  const accounts: any[] = accountsRes?.data ?? [];
  const assetAccts = accounts.filter(a => a.account_type === 'Asset');
  const accumAccts = accounts.filter(a => a.account_type === 'ContraAsset');
  const deprAccts = accounts.filter(a => a.account_type === 'Expense');
  const pick = (list: any[], re: RegExp) => (list.find(a => re.test(a.name)) ?? list[0])?.code ?? '';

  useEffect(() => {
    if (!accounts.length || form.gl_asset_account) return;
    setForm(f => ({
      ...f,
      gl_asset_account: pick(assetAccts, /fixed asset|motor|equipment|asset/i),
      gl_accum_depr_account: pick(accumAccts, /accumulated|accum/i),
      gl_depr_expense: pick(deprAccts, /depreciation/i),
    }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accounts.length]);

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

  const categories = [
    { value: 'LandAndBuildings', label: 'Land & Buildings' },
    { value: 'MotorVehicles', label: 'Motor Vehicles' },
    { value: 'PlantAndMachinery', label: 'Plant & Machinery' },
    { value: 'FurnitureAndFittings', label: 'Furniture & Fittings' },
    { value: 'ComputerEquipment', label: 'Computer Equipment' },
    { value: 'Software', label: 'Software' },
  ];
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
              {categories.map(c => <option key={c.value} value={c.value}>{c.label}</option>)}
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
              {assetAccts.map(a => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Accum. Depr. Account</label>
            <select className="input" value={form.gl_accum_depr_account} onChange={(e) => setForm({ ...form, gl_accum_depr_account: e.target.value })}>
              {accumAccts.map(a => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
            </select>
          </div>
          <div>
            <label className="label">Depr. Expense Account</label>
            <select className="input" value={form.gl_depr_expense} onChange={(e) => setForm({ ...form, gl_depr_expense: e.target.value })}>
              {deprAccts.map(a => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
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
