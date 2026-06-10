import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPayments, recordPayment } from '../../api/client';
import type { Payment } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import { Plus } from 'lucide-react';

export default function PaymentsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const { data: payments = [], isLoading } = useQuery<Payment[]>({ queryKey: ['payments'], queryFn: () => getPayments().then(r => r.data) });

  const columns: Column<Payment>[] = [
    { key: 'number', header: 'Number', render: (r) => <span className="font-medium">{r.number}</span> },
    { key: 'payment_type', header: 'Type', render: (r) => r.payment_type === 'CustomerPayment' ? <span className="badge-success">Received</span> : <span className="badge-info">Sent</span> },
    { key: 'payment_date', header: 'Date', render: (r) => formatDate(r.payment_date) },
    { key: 'amount', header: 'Amount', render: (r) => formatCurrency(r.amount), className: 'text-right' },
    { key: 'method', header: 'Method', render: (r) => { const m = r.method; if (m?.Mpesa) return 'M-Pesa'; if (m?.BankTransfer) return 'Bank Transfer'; if (m?.Card) return 'Card'; if (m === 'Cash') return 'Cash'; return 'Other'; } },
    { key: 'reference', header: 'Reference' },
    { key: 'status', header: 'Status', render: (r) => <span className="badge-success">{r.status}</span> },
  ];

  return (
    <div>
      <PageHeader title="Payments" subtitle="Record and track payments received and sent" actions={<button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> Record Payment</button>} />
      <DataTable columns={columns} data={payments} loading={isLoading} emptyMessage="No payments recorded." />
      {showCreate && <RecordPaymentModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function RecordPaymentModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    payment_type: 'CustomerPayment',
    amount: 0,
    method: 'BankTransfer',
    reference: '',
    party_id: '',
    payment_date: new Date().toISOString().split('T')[0],
    bank_account_id: '',
    apply_to_document_id: '',
    apply_amount: 0,
  });
  const mutation = useMutation({ mutationFn: (data: any) => recordPayment(data), onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['payments'] }); onClose(); } });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const method = form.method === 'Mpesa' ? { Mpesa: { transaction_id: form.reference, phone: '' } } : form.method === 'BankTransfer' ? { BankTransfer: { reference: form.reference } } : form.method === 'Cheque' ? { Cheque: { number: form.reference } } : form.method === 'Card' ? { Card: { processor: 'Manual', authorization: form.reference } } : 'Cash';
    const applications = form.apply_to_document_id ? [{ document_id: form.apply_to_document_id, amount: form.apply_amount || form.amount }] : [];
    mutation.mutate({
      payment_type: form.payment_type,
      party_id: form.party_id,
      payment_date: form.payment_date,
      amount: form.amount,
      method,
      reference: form.reference,
      bank_account_id: form.bank_account_id || undefined,
      applications,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Record Payment">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div><label className="label">Type</label><select className="input" value={form.payment_type} onChange={(e) => setForm({ ...form, payment_type: e.target.value })}><option value="CustomerPayment">Payment Received (from Customer)</option><option value="VendorPayment">Payment Sent (to Vendor)</option></select></div>
        <div className="grid grid-cols-2 gap-4">
          <div><label className="label">Amount (KES) *</label><input type="number" className="input" step="0.01" value={form.amount} onChange={(e) => setForm({ ...form, amount: +e.target.value })} required /></div>
          <div><label className="label">Payment Date *</label><input type="date" className="input" value={form.payment_date} onChange={(e) => setForm({ ...form, payment_date: e.target.value })} required /></div>
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div><label className="label">Method</label><select className="input" value={form.method} onChange={(e) => setForm({ ...form, method: e.target.value })}><option value="BankTransfer">Bank Transfer</option><option value="Mpesa">M-Pesa</option><option value="Cash">Cash</option><option value="Cheque">Cheque</option><option value="Card">Card</option></select></div>
          <div><label className="label">Bank Account</label><input className="input" value={form.bank_account_id} onChange={(e) => setForm({ ...form, bank_account_id: e.target.value })} placeholder="Optional — bank account ID" /></div>
        </div>
        <div><label className="label">Reference / Receipt No.</label><input className="input" value={form.reference} onChange={(e) => setForm({ ...form, reference: e.target.value })} placeholder="e.g. M-Pesa receipt number" /></div>
        <hr />
        <div>
          <h4 className="text-sm font-medium text-gray-700 mb-2">Apply to Document</h4>
          <div className="grid grid-cols-2 gap-4">
            <div><label className="label">Invoice/Bill ID</label><input className="input font-mono text-xs" value={form.apply_to_document_id} onChange={(e) => setForm({ ...form, apply_to_document_id: e.target.value })} placeholder="Paste document ID to apply" /></div>
            <div><label className="label">Amount to Apply</label><input type="number" step="0.01" className="input" value={form.apply_amount} onChange={(e) => setForm({ ...form, apply_amount: +e.target.value })} placeholder="Full amount if blank" /></div>
          </div>
          <p className="text-xs text-gray-400 mt-1">Leave blank to record as unapplied payment</p>
        </div>
        <div className="flex justify-end gap-3 pt-4 border-t"><button type="button" onClick={onClose} className="btn-secondary">Cancel</button><button type="submit" className="btn-primary" disabled={mutation.isPending}>{mutation.isPending ? 'Recording...' : 'Record Payment'}</button></div>
      </form>
    </Modal>
  );
}
