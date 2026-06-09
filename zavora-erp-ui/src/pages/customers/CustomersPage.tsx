import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getCustomers, createCustomer } from '../../api/client';
import type { Customer } from '../../types';
import { formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus, UserPlus } from 'lucide-react';

export default function CustomersPage() {
  const [showCreate, setShowCreate] = useState(false);
  const queryClient = useQueryClient();

  const { data: customers = [], isLoading } = useQuery<Customer[]>({
    queryKey: ['customers'],
    queryFn: () => getCustomers().then(r => r.data),
  });

  const columns: Column<Customer>[] = [
    { key: 'name', header: 'Name', render: (r) => <span className="font-medium">{r.name}</span> },
    { key: 'kra_pin', header: 'KRA PIN', render: (r) => r.kra_pin || '—' },
    { key: 'currency', header: 'Currency' },
    { key: 'payment_terms', header: 'Terms' },
    { key: 'is_active', header: 'Status', render: (r) => <span className={r.is_active ? 'badge-success' : 'badge-gray'}>{r.is_active ? 'Active' : 'Inactive'}</span> },
    { key: 'created_at', header: 'Created', render: (r) => formatDate(r.created_at) },
  ];

  return (
    <div>
      <PageHeader
        title="Customers"
        subtitle="Manage your customers and their billing information"
        actions={<button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> New Customer</button>}
      />
      <DataTable columns={columns} data={customers} loading={isLoading} emptyMessage="No customers yet." />
      {showCreate && <CreateCustomerModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateCustomerModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ name: '', kra_pin: '', email: '', phone: '', payment_terms: 'Net30' });

  const mutation = useMutation({
    mutationFn: (data: any) => createCustomer(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['customers'] }); onClose(); },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      name: form.name,
      kra_pin: form.kra_pin || undefined,
      email: form.email ? [{ email: form.email, is_primary: true }] : [],
      phone: form.phone ? [{ number: form.phone, is_primary: true, whatsapp_enabled: true }] : [],
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="New Customer">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div><label className="label">Company / Customer Name *</label><input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} required /></div>
        <div className="grid grid-cols-2 gap-4">
          <div><label className="label">KRA PIN</label><input className="input" value={form.kra_pin} onChange={(e) => setForm({ ...form, kra_pin: e.target.value })} placeholder="P00XXXXXXX" /></div>
          <div><label className="label">Payment Terms</label>
            <select className="input" value={form.payment_terms} onChange={(e) => setForm({ ...form, payment_terms: e.target.value })}>
              <option value="DueOnReceipt">Due on Receipt</option><option value="Net7">Net 7</option><option value="Net14">Net 14</option><option value="Net30">Net 30</option><option value="Net60">Net 60</option>
            </select>
          </div>
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div><label className="label">Email</label><input type="email" className="input" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} /></div>
          <div><label className="label">Phone</label><input className="input" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} placeholder="+254 7XX XXX XXX" /></div>
        </div>
        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>{mutation.isPending ? 'Creating...' : 'Create Customer'}</button>
        </div>
      </form>
    </Modal>
  );
}
