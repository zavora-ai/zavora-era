import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getVendors, createVendor } from '../../api/client';
import type { Vendor } from '../../types';
import { formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus } from 'lucide-react';

export default function VendorsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const queryClient = useQueryClient();
  const { data: vendors = [], isLoading } = useQuery<Vendor[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then(r => r.data) });

  const columns: Column<Vendor>[] = [
    { key: 'name', header: 'Name', render: (r) => <span className="font-medium">{r.name}</span> },
    { key: 'kra_pin', header: 'KRA PIN', render: (r) => r.kra_pin || '—' },
    { key: 'wht_category', header: 'WHT Category', render: (r) => r.wht_category || '—' },
    { key: 'resident', header: 'Resident', render: (r) => r.resident ? 'Yes' : 'No' },
    { key: 'payment_terms', header: 'Terms' },
    { key: 'is_active', header: 'Status', render: (r) => <span className={r.is_active ? 'badge-success' : 'badge-gray'}>{r.is_active ? 'Active' : 'Inactive'}</span> },
  ];

  return (
    <div>
      <PageHeader title="Vendors" subtitle="Manage suppliers and their withholding tax configuration" actions={<button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> New Vendor</button>} />
      <DataTable columns={columns} data={vendors} loading={isLoading} emptyMessage="No vendors yet." />
      {showCreate && <CreateVendorModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateVendorModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ name: '', kra_pin: '', wht_category: '', resident: true, email: '', phone: '' });
  const mutation = useMutation({ mutationFn: (data: any) => createVendor(data), onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['vendors'] }); onClose(); } });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({ name: form.name, kra_pin: form.kra_pin || undefined, wht_category: form.wht_category || undefined, resident: form.resident, email: form.email ? [{ email: form.email, is_primary: true }] : [], phone: form.phone ? [{ number: form.phone, is_primary: true, whatsapp_enabled: false }] : [] });
  };

  return (
    <Modal open={true} onClose={onClose} title="New Vendor">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div><label className="label">Vendor Name *</label><input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} required /></div>
        <div className="grid grid-cols-2 gap-4">
          <div><label className="label">KRA PIN</label><input className="input" value={form.kra_pin} onChange={(e) => setForm({ ...form, kra_pin: e.target.value })} /></div>
          <div><label className="label">WHT Category</label>
            <select className="input" value={form.wht_category} onChange={(e) => setForm({ ...form, wht_category: e.target.value })}>
              <option value="">None</option><option value="Consultancy">Consultancy (5%/20%)</option><option value="Rent">Rent (10%/30%)</option><option value="Contractual">Contractual (3%/20%)</option><option value="Royalties">Royalties (5%/20%)</option><option value="Interest">Interest (15%/15%)</option>
            </select>
          </div>
        </div>
        <div className="flex items-center gap-2"><input type="checkbox" checked={form.resident} onChange={(e) => setForm({ ...form, resident: e.target.checked })} /><label className="text-sm">Kenyan Resident</label></div>
        <div className="grid grid-cols-2 gap-4">
          <div><label className="label">Email</label><input type="email" className="input" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} /></div>
          <div><label className="label">Phone</label><input className="input" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} /></div>
        </div>
        <div className="flex justify-end gap-3 pt-4 border-t"><button type="button" onClick={onClose} className="btn-secondary">Cancel</button><button type="submit" className="btn-primary" disabled={mutation.isPending}>{mutation.isPending ? 'Creating...' : 'Create Vendor'}</button></div>
      </form>
    </Modal>
  );
}
