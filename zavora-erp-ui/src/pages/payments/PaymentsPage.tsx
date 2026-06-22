import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPayments, recordPayment } from '../../api/client';
import api from '../../api/client';
import type { Payment } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import PaginationControls from '../../components/shared/PaginationControls';
import { usePagination } from '../../hooks/usePagination';
import Modal from '../../components/shared/Modal';
import { Plus, ArrowRightLeft } from 'lucide-react';

type TabKey = 'all' | 'unapplied';

export default function PaymentsPage() {
  const [activeTab, setActiveTab] = useState<TabKey>('all');
  const [showCreate, setShowCreate] = useState(false);

  return (
    <div>
      <PageHeader
        title="Payments"
        subtitle="Record and track payments received and sent"
        actions={
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> Record Payment
          </button>
        }
      />

      {/* Tab navigation */}
      <div className="flex gap-1 mb-6 border-b border-gray-200">
        <button
          onClick={() => setActiveTab('all')}
          className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === 'all'
              ? 'border-blue-600 text-blue-600'
              : 'border-transparent text-gray-500 hover:text-gray-700'
          }`}
        >
          All Payments
        </button>
        <button
          onClick={() => setActiveTab('unapplied')}
          className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === 'unapplied'
              ? 'border-blue-600 text-blue-600'
              : 'border-transparent text-gray-500 hover:text-gray-700'
          }`}
        >
          Unapplied Payments
        </button>
      </div>

      {activeTab === 'all' && <AllPaymentsTab />}
      {activeTab === 'unapplied' && <UnappliedPaymentsTab />}

      {showCreate && <RecordPaymentModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

// ─── All Payments Tab ──────────────────────────────────────────────────────────

function AllPaymentsTab() {
  const { page, limit, offset, setPage } = usePagination();
  const { data: resp, isLoading } = useQuery({
    queryKey: ['payments', offset, limit],
    queryFn: () => getPayments({ limit, offset }).then((r) => r.data),
  });
  const payments: Payment[] = resp?.data ?? [];
  const total: number = resp?.total_count ?? 0;

  const columns: Column<Payment>[] = [
    { key: 'number', header: 'Number', render: (r) => <span className="font-medium">{r.number}</span> },
    {
      key: 'payment_type',
      header: 'Type',
      render: (r) =>
        r.payment_type === 'customer_payment' ? (
          <span className="badge-success">Received</span>
        ) : (
          <span className="badge-info">Sent</span>
        ),
    },
    { key: 'payment_date', header: 'Date', render: (r) => formatDate(r.payment_date) },
    { key: 'amount', header: 'Amount', render: (r) => formatCurrency(r.amount), className: 'text-right' },
    {
      key: 'method',
      header: 'Method',
      render: (r) => {
        const m = r.method;
        if (m?.Mpesa) return 'M-Pesa';
        if (m?.BankTransfer) return 'Bank Transfer';
        if (m?.Card) return 'Card';
        if (m === 'Cash') return 'Cash';
        return 'Other';
      },
    },
    { key: 'reference', header: 'Reference' },
    { key: 'status', header: 'Status', render: (r) => <span className="badge-success">{r.status}</span> },
  ];

  return (
    <>
      <DataTable columns={columns} data={payments} loading={isLoading} emptyMessage="No payments recorded." />
      <PaginationControls page={page} limit={limit} total={total} onPage={setPage} />
    </>
  );
}

// ─── Unapplied Payments Tab ────────────────────────────────────────────────────

function UnappliedPaymentsTab() {
  const [allocatePayment, setAllocatePayment] = useState<Payment | null>(null);

  const { data: unappliedPayments = [], isLoading } = useQuery<Payment[]>({
    queryKey: ['payments', 'unapplied'],
    queryFn: () => api.get('/payments', { params: { status: 'unapplied' } }).then((r) => r.data),
  });

  const columns: Column<Payment>[] = [
    { key: 'number', header: 'Number', render: (r) => <span className="font-medium">{r.number}</span> },
    {
      key: 'payment_type',
      header: 'Customer / Vendor',
      render: (r) => (
        <span className="text-sm">
          {r.payment_type === 'customer_payment' ? (
            <span className="inline-flex items-center gap-1">
              <span className="w-2 h-2 rounded-full bg-green-400" />
              Customer
            </span>
          ) : (
            <span className="inline-flex items-center gap-1">
              <span className="w-2 h-2 rounded-full bg-blue-400" />
              Vendor
            </span>
          )}
          <span className="ml-2 text-gray-500 font-mono text-xs">{r.party_id?.slice(0, 8)}...</span>
        </span>
      ),
    },
    { key: 'amount', header: 'Amount', render: (r) => formatCurrency(r.amount), className: 'text-right' },
    {
      key: 'unapplied',
      header: 'Unapplied Balance',
      render: (r) => (
        <span className="font-semibold text-amber-600">{formatCurrency(r.unapplied)}</span>
      ),
      className: 'text-right',
    },
    { key: 'payment_date', header: 'Date', render: (r) => formatDate(r.payment_date) },
    {
      key: 'actions',
      header: 'Action',
      render: (r) => (
        <button
          onClick={() => setAllocatePayment(r)}
          className="btn-secondary text-xs inline-flex items-center gap-1"
        >
          <ArrowRightLeft className="w-3 h-3" />
          Allocate
        </button>
      ),
    },
  ];

  return (
    <>
      <DataTable
        columns={columns}
        data={unappliedPayments}
        loading={isLoading}
        emptyMessage="No unapplied payments. All payments have been fully allocated."
      />
      {allocatePayment && (
        <AllocatePaymentModal
          payment={allocatePayment}
          onClose={() => setAllocatePayment(null)}
        />
      )}
    </>
  );
}

// ─── Allocate Payment Modal ────────────────────────────────────────────────────

function AllocatePaymentModal({ payment, onClose }: { payment: Payment; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [documentId, setDocumentId] = useState('');
  const [applyAmount, setApplyAmount] = useState<number>(payment.unapplied);
  const [error, setError] = useState('');

  const mutation = useMutation({
    mutationFn: (data: { payment_id: string; document_id: string; amount: number }) =>
      api.post('/payments/apply', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['payments'] });
      queryClient.invalidateQueries({ queryKey: ['payments', 'unapplied'] });
      onClose();
    },
    onError: (err: any) => {
      setError(err.response?.data?.message || 'Failed to apply payment. Please try again.');
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!documentId.trim()) {
      setError('Please enter a document ID (invoice or bill).');
      return;
    }
    if (applyAmount <= 0) {
      setError('Amount must be greater than zero.');
      return;
    }
    if (applyAmount > payment.unapplied) {
      setError(`Amount cannot exceed unapplied balance of ${formatCurrency(payment.unapplied)}.`);
      return;
    }

    mutation.mutate({
      payment_id: payment.id,
      document_id: documentId.trim(),
      amount: applyAmount,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title="Allocate Payment" subtitle={`Apply unapplied funds from ${payment.number}`}>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* Payment summary */}
        <div className="bg-gray-50 rounded-lg p-4 space-y-2">
          <div className="flex justify-between text-sm">
            <span className="text-gray-500">Payment Number</span>
            <span className="font-medium">{payment.number}</span>
          </div>
          <div className="flex justify-between text-sm">
            <span className="text-gray-500">Total Amount</span>
            <span>{formatCurrency(payment.amount)}</span>
          </div>
          <div className="flex justify-between text-sm">
            <span className="text-gray-500">Unapplied Balance</span>
            <span className="font-semibold text-amber-600">{formatCurrency(payment.unapplied)}</span>
          </div>
          <div className="flex justify-between text-sm">
            <span className="text-gray-500">Date</span>
            <span>{formatDate(payment.payment_date)}</span>
          </div>
        </div>

        {/* Document selection */}
        <div>
          <label className="label">Invoice / Bill ID *</label>
          <input
            className="input font-mono text-xs"
            value={documentId}
            onChange={(e) => setDocumentId(e.target.value)}
            placeholder="Paste the invoice or bill ID to apply payment to"
            required
          />
          <p className="text-xs text-gray-400 mt-1">
            Enter the document ID of the invoice or bill you want to allocate this payment to.
          </p>
        </div>

        {/* Amount */}
        <div>
          <label className="label">Amount to Apply (KES) *</label>
          <input
            type="number"
            step="0.01"
            min="0.01"
            max={payment.unapplied}
            className="input"
            value={applyAmount}
            onChange={(e) => setApplyAmount(+e.target.value)}
            required
          />
          <p className="text-xs text-gray-400 mt-1">
            Maximum: {formatCurrency(payment.unapplied)}
          </p>
        </div>

        {/* Error message */}
        {error && (
          <div className="bg-red-50 border border-red-200 rounded-lg p-3 text-sm text-red-700">
            {error}
          </div>
        )}

        {/* Actions */}
        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">
            Cancel
          </button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Applying...' : 'Apply Payment'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

// ─── Record Payment Modal ──────────────────────────────────────────────────────

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
  const mutation = useMutation({
    mutationFn: (data: any) => recordPayment(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['payments'] });
      queryClient.invalidateQueries({ queryKey: ['payments', 'unapplied'] });
      onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const method =
      form.method === 'Mpesa'
        ? { Mpesa: { transaction_id: form.reference, phone: '' } }
        : form.method === 'BankTransfer'
          ? { BankTransfer: { reference: form.reference } }
          : form.method === 'Cheque'
            ? { Cheque: { number: form.reference } }
            : form.method === 'Card'
              ? { Card: { processor: 'Manual', authorization: form.reference } }
              : 'Cash';
    const applications = form.apply_to_document_id
      ? [{ document_id: form.apply_to_document_id, amount: form.apply_amount || form.amount }]
      : [];
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
        <div>
          <label className="label">Type</label>
          <select
            className="input"
            value={form.payment_type}
            onChange={(e) => setForm({ ...form, payment_type: e.target.value })}
          >
            <option value="CustomerPayment">Payment Received (from Customer)</option>
            <option value="VendorPayment">Payment Sent (to Vendor)</option>
          </select>
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Amount (KES) *</label>
            <input
              type="number"
              className="input"
              step="0.01"
              value={form.amount}
              onChange={(e) => setForm({ ...form, amount: +e.target.value })}
              required
            />
          </div>
          <div>
            <label className="label">Payment Date *</label>
            <input
              type="date"
              className="input"
              value={form.payment_date}
              onChange={(e) => setForm({ ...form, payment_date: e.target.value })}
              required
            />
          </div>
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="label">Method</label>
            <select
              className="input"
              value={form.method}
              onChange={(e) => setForm({ ...form, method: e.target.value })}
            >
              <option value="BankTransfer">Bank Transfer</option>
              <option value="Mpesa">M-Pesa</option>
              <option value="Cash">Cash</option>
              <option value="Cheque">Cheque</option>
              <option value="Card">Card</option>
            </select>
          </div>
          <div>
            <label className="label">Bank Account</label>
            <input
              className="input"
              value={form.bank_account_id}
              onChange={(e) => setForm({ ...form, bank_account_id: e.target.value })}
              placeholder="Optional — bank account ID"
            />
          </div>
        </div>
        <div>
          <label className="label">Reference / Receipt No.</label>
          <input
            className="input"
            value={form.reference}
            onChange={(e) => setForm({ ...form, reference: e.target.value })}
            placeholder="e.g. M-Pesa receipt number"
          />
        </div>
        <hr />
        <div>
          <h4 className="text-sm font-medium text-gray-700 mb-2">Apply to Document</h4>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="label">Invoice/Bill ID</label>
              <input
                className="input font-mono text-xs"
                value={form.apply_to_document_id}
                onChange={(e) => setForm({ ...form, apply_to_document_id: e.target.value })}
                placeholder="Paste document ID to apply"
              />
            </div>
            <div>
              <label className="label">Amount to Apply</label>
              <input
                type="number"
                step="0.01"
                className="input"
                value={form.apply_amount}
                onChange={(e) => setForm({ ...form, apply_amount: +e.target.value })}
                placeholder="Full amount if blank"
              />
            </div>
          </div>
          <p className="text-xs text-gray-400 mt-1">Leave blank to record as unapplied payment</p>
        </div>
        <div className="flex justify-end gap-3 pt-4 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">
            Cancel
          </button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Recording...' : 'Record Payment'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
