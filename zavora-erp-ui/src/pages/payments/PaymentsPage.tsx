import { useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPayments, recordPayment, getCustomers, getVendors, getInvoices, getBills, getBankAccounts, getAccounts } from '../../api/client';
import api from '../../api/client';
import type { Payment } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import { workToday } from '../../utils/workDate';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import PaginationControls from '../../components/shared/PaginationControls';
import { usePagination } from '../../hooks/usePagination';
import Modal from '../../components/shared/Modal';
import Attachments from '../../components/shared/Attachments';
import { Plus, ArrowRightLeft } from 'lucide-react';

type TabKey = 'all' | 'unapplied';

export default function PaymentsPage() {
  const [activeTab, setActiveTab] = useState<TabKey>('all');
  const [showCreate, setShowCreate] = useState(false);
  const [searchParams, setSearchParams] = useSearchParams();

  // Deep-link from a "Pay" button on an invoice/bill row:
  // /payments?record=customer&party=<id>&invoice=<id>  (or ?bill=<id>)
  const recordKind = searchParams.get('record');
  const preset: RecordPreset | undefined = recordKind
    ? {
        payment_type: recordKind === 'vendor' ? 'vendor_payment' : 'customer_payment',
        party_id: searchParams.get('party') ?? undefined,
        apply_to_document_id: searchParams.get('invoice') ?? searchParams.get('bill') ?? undefined,
      }
    : undefined;
  const presetOpen = !!recordKind;
  const closePreset = () => setSearchParams({}, { replace: true });

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
      {presetOpen && <RecordPaymentModal preset={preset} onClose={closePreset} />}
    </div>
  );
}

// ─── All Payments Tab ──────────────────────────────────────────────────────────

function AllPaymentsTab() {
  const { page, limit, offset, setPage } = usePagination();
  const [viewing, setViewing] = useState<Payment | null>(null);
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
    {
      key: 'amount',
      header: 'Amount',
      className: 'text-right',
      render: (r) => (
        <div>
          <span className="font-medium">{formatCurrency(r.amount, r.currency)}</span>
          {r.currency !== 'KES' && (
            <p className="text-xs text-gray-400">≈ {formatCurrency(Number(r.amount) * Number(r.fx_rate || 1), 'KES')}</p>
          )}
        </div>
      ),
    },
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
      <DataTable columns={columns} data={payments} loading={isLoading} onRowClick={(r) => setViewing(r)} emptyMessage="No payments recorded." />
      <PaginationControls page={page} limit={limit} total={total} onPage={setPage} />
      {viewing && <PaymentDetailModal payment={viewing} onClose={() => setViewing(null)} />}
    </>
  );
}

/** Read-only payment summary + attachments (receipt, WHT certificate, remittance advice). */
function PaymentDetailModal({ payment, onClose }: { payment: Payment; onClose: () => void }) {
  const method = (() => {
    const m: any = payment.method;
    if (m?.Mpesa) return 'M-Pesa';
    if (m?.BankTransfer) return 'Bank Transfer';
    if (m?.Card) return 'Card';
    if (m === 'Cash') return 'Cash';
    return 'Other';
  })();
  return (
    <Modal open={true} onClose={onClose} title={`Payment ${payment.number}`}>
      <div className="space-y-4">
        <div className="grid grid-cols-2 gap-3 text-sm">
          <div><span className="text-gray-500">Date</span><p className="font-medium">{formatDate(payment.payment_date)}</p></div>
          <div><span className="text-gray-500">Type</span><p className="font-medium">{payment.payment_type === 'customer_payment' ? 'Customer receipt' : 'Vendor payment'}</p></div>
          <div><span className="text-gray-500">Amount</span><p className="font-medium">{formatCurrency(payment.amount, payment.currency)}{payment.currency !== 'KES' && <span className="text-xs text-gray-400"> · ≈ {formatCurrency(Number(payment.amount) * Number(payment.fx_rate || 1), 'KES')}</span>}</p></div>
          <div><span className="text-gray-500">Method</span><p className="font-medium">{method}</p></div>
          <div className="col-span-2"><span className="text-gray-500">Reference</span><p className="font-medium">{payment.reference || '—'}</p></div>
        </div>
        <div className="border-t pt-4">
          <Attachments linkedType="payment" linkedId={payment.id} label="Attachments (receipt, WHT certificate, remittance advice)" />
        </div>
        <div className="flex justify-end pt-2 border-t">
          <button type="button" onClick={onClose} className="btn-secondary">Close</button>
        </div>
      </div>
    </Modal>
  );
}

// ─── Unapplied Payments Tab ────────────────────────────────────────────────────

function UnappliedPaymentsTab() {
  const [allocatePayment, setAllocatePayment] = useState<Payment | null>(null);

  const { data: unappliedPayments = [], isLoading } = useQuery<Payment[]>({
    queryKey: ['payments', 'unapplied'],
    queryFn: () => api.get('/payments', { params: { status: 'unapplied' } }).then((r) => {
      const d = r.data;
      return Array.isArray(d) ? d : (Array.isArray(d?.data) ? d.data : []);
    }),
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
    {
      key: 'amount',
      header: 'Amount',
      className: 'text-right',
      render: (r) => (
        <div>
          <span className="font-medium">{formatCurrency(r.amount, r.currency)}</span>
          {r.currency !== 'KES' && (
            <p className="text-xs text-gray-400">≈ {formatCurrency(Number(r.amount) * Number(r.fx_rate || 1), 'KES')}</p>
          )}
        </div>
      ),
    },
    {
      key: 'unapplied',
      header: 'Unapplied Balance',
      render: (r) => (
        <span className="font-semibold text-amber-600">{formatCurrency(r.unapplied, r.currency)}</span>
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

  // Open documents for THIS payment's party, so the user picks from a list
  // instead of pasting a UUID. Customer receipt → open invoices for the
  // customer; vendor payment → open bills for the vendor.
  const isCustomer = payment.payment_type === 'customer_payment';
  const { data: openDocs = [] } = useQuery<any[]>({
    queryKey: ['open-docs', payment.id],
    queryFn: async () => {
      const r = isCustomer ? await getInvoices({ limit: 200 }) : await getBills({ limit: 200 });
      const rows: any[] = r.data?.items ?? (Array.isArray(r.data) ? r.data : []);
      const partyKey = isCustomer ? 'customer_id' : 'vendor_id';
      return rows.filter(
        (d) => d[partyKey] === payment.party_id && Number(d.balance_due) > 0 && d.status !== 'draft' && d.status !== 'voided',
      );
    },
  });

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
      setError(`Please select an ${isCustomer ? 'invoice' : 'bill'} to apply to.`);
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

        {/* Document selection — a picker of the party's open documents. */}
        <div>
          <label className="label">{isCustomer ? 'Invoice' : 'Bill'} to apply to *</label>
          <select
            className="input"
            value={documentId}
            onChange={(e) => {
              setDocumentId(e.target.value);
              // Default the amount to min(unapplied, that document's balance).
              const doc = openDocs.find((d) => d.id === e.target.value);
              if (doc) setApplyAmount(Math.min(payment.unapplied, Number(doc.balance_due)));
            }}
            required
          >
            <option value="">Select an open {isCustomer ? 'invoice' : 'bill'}…</option>
            {openDocs.map((d) => (
              <option key={d.id} value={d.id}>
                {d.number} — {formatCurrency(d.balance_due, d.currency)} due
              </option>
            ))}
          </select>
          {openDocs.length === 0 && (
            <p className="text-xs text-gray-400 mt-1">
              No open {isCustomer ? 'invoices' : 'bills'} for this party.
            </p>
          )}
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

interface RecordPreset {
  payment_type?: string;
  party_id?: string;
  apply_to_document_id?: string;
}

function RecordPaymentModal({ onClose, preset }: { onClose: () => void; preset?: RecordPreset }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({
    // API contract: serde expects snake_case PaymentType ('customer_payment' | 'vendor_payment').
    payment_type: preset?.payment_type ?? 'customer_payment',
    amount: 0,
    currency: 'KES',
    fx_rate: 1,
    method: 'BankTransfer',
    reference: '',
    party_id: preset?.party_id ?? '',
    payment_date: workToday(),
    bank_account_id: '',
    funding_source: 'bank' as 'bank' | 'director', // pay-from: company bank vs director's loan / owner funds
    funding_account: '4200', // GL account when funding_source = 'director'
    apply_to_document_id: preset?.apply_to_document_id ?? '',
    apply_amount: 0,
    wht_amount: 0, // withholding tax withheld by the customer (KES), customer receipts only
  });

  const isCustomer = form.payment_type === 'customer_payment';

  // Parties for the selector — customers for AR receipts, vendors for AP payments.
  const { data: customers = [] } = useQuery<any[]>({
    queryKey: ['customers'],
    queryFn: () => getCustomers().then((r) => r.data),
  });
  const { data: vendors = [] } = useQuery<any[]>({
    queryKey: ['vendors'],
    queryFn: () => getVendors().then((r) => r.data),
  });
  const { data: bankAccounts = [] } = useQuery<any[]>({
    queryKey: ['bank-accounts'],
    queryFn: () => getBankAccounts().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });
  // Liability/equity accounts that can fund a payment off-bank (e.g. Directors
  // Loans, owner's capital) — used when 'Paid from' is the director.
  const { data: accounts = [] } = useQuery<any[]>({
    queryKey: ['accounts'],
    queryFn: () => getAccounts().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });
  const fundingAccounts = accounts
    .filter((a) => (a.account_type === 'Liability' || a.account_type === 'Equity') && a.is_active && !a.is_control)
    .sort((a, b) => a.code.localeCompare(b.code));
  const parties = isCustomer ? customers : vendors;

  // Open documents to apply against, scoped to the selected party.
  const { data: invoicesResp } = useQuery<any>({
    queryKey: ['invoices', 'for-payment'],
    queryFn: () => getInvoices({ limit: 200 }).then((r) => r.data),
    enabled: isCustomer,
  });
  const { data: billsResp } = useQuery<any>({
    queryKey: ['bills', 'for-payment'],
    queryFn: () => getBills({ limit: 200 }).then((r) => r.data),
    enabled: !isCustomer,
  });
  const docItems: any[] = isCustomer
    ? (invoicesResp?.data ?? invoicesResp ?? [])
    : (billsResp?.data ?? billsResp ?? []);
  const openDocs = (Array.isArray(docItems) ? docItems : []).filter(
    (d) =>
      (!form.party_id || d.customer_id === form.party_id || d.vendor_id === form.party_id) &&
      !['paid', 'voided', 'draft'].includes(d.status) &&
      Number(d.balance_due ?? 0) > 0,
  );

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
      currency: form.currency,
      fx_rate: form.fx_rate,
      method,
      reference: form.reference,
      bank_account_id: form.funding_source === 'bank' ? (form.bank_account_id || undefined) : undefined,
      // Off-bank funding (director's loan / owner funds): the payment's bank leg
      // posts to this GL account instead of a cash account.
      funding_account: form.funding_source === 'director' ? form.funding_account : undefined,
      applications,
      // WHT withheld by the customer (KES). The receipt clears the full invoice
      // as cash (amount) + WHT, posting the credit to WHT Receivable.
      wht_amount: isCustomer && form.wht_amount > 0 ? form.wht_amount : undefined,
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
            onChange={(e) =>
              setForm({ ...form, payment_type: e.target.value, party_id: '', apply_to_document_id: '' })
            }
          >
            <option value="customer_payment">Payment Received (from Customer)</option>
            <option value="vendor_payment">Payment Sent (to Vendor)</option>
          </select>
        </div>
        <div>
          <label className="label">{isCustomer ? 'Customer' : 'Vendor'} *</label>
          <select
            className="input"
            value={form.party_id}
            onChange={(e) => setForm({ ...form, party_id: e.target.value, apply_to_document_id: '' })}
            required
          >
            <option value="">Choose a {isCustomer ? 'customer' : 'vendor'}...</option>
            {parties.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </div>
        <div className="grid grid-cols-3 gap-4">
          <div>
            <label className="label">Amount ({form.currency}) *</label>
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
            <label className="label">Currency</label>
            <select className="input" value={form.currency} onChange={(e) => setForm({ ...form, currency: e.target.value, fx_rate: e.target.value === 'KES' ? 1 : form.fx_rate })}>
              <option value="KES">KES</option><option value="USD">USD</option><option value="EUR">EUR</option><option value="GBP">GBP</option>
            </select>
          </div>
          <div>
            <label className="label">FX Rate → KES</label>
            <input type="number" step="0.0001" className="input" value={form.fx_rate} disabled={form.currency === 'KES'} onChange={(e) => setForm({ ...form, fx_rate: +e.target.value })} />
          </div>
        </div>
        <div className="grid grid-cols-2 gap-4">
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
            <label className="label">Paid {isCustomer ? 'into' : 'from'}</label>
            <select
              className="input"
              value={form.funding_source}
              onChange={(e) => setForm({ ...form, funding_source: e.target.value as 'bank' | 'director' })}
            >
              <option value="bank">Bank account</option>
              <option value="director">Director's loan / owner funds</option>
            </select>
            {form.funding_source === 'bank' ? (
              <select
                className="input mt-2"
                value={form.bank_account_id}
                onChange={(e) => setForm({ ...form, bank_account_id: e.target.value })}
              >
                <option value="">Default / unspecified</option>
                {bankAccounts.map((b) => (
                  <option key={b.id} value={b.id}>{b.name} ({b.currency})</option>
                ))}
              </select>
            ) : (
              <>
                <select
                  className="input mt-2"
                  value={form.funding_account}
                  onChange={(e) => setForm({ ...form, funding_account: e.target.value })}
                >
                  {fundingAccounts.map((a) => (
                    <option key={a.code} value={a.code}>{a.code} — {a.name}</option>
                  ))}
                </select>
                <p className="text-xs text-gray-400 mt-1">
                  {isCustomer ? 'Funds received by the director on the company\u2019s behalf' : 'Paid by the director personally — credits the director\u2019s loan instead of a company bank account.'}
                </p>
              </>
            )}
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
              <label className="label">{isCustomer ? 'Invoice' : 'Bill'}</label>
              <select
                className="input"
                value={form.apply_to_document_id}
                onChange={(e) => {
                  const doc = openDocs.find((d) => d.id === e.target.value);
                  setForm({
                    ...form,
                    apply_to_document_id: e.target.value,
                    apply_amount: doc ? Number(doc.balance_due ?? 0) : form.apply_amount,
                    // Match the receipt currency/rate to the document being settled.
                    currency: doc?.currency ?? form.currency,
                    fx_rate: doc ? Number(doc.fx_rate ?? 1) : form.fx_rate,
                  });
                }}
                disabled={!form.party_id}
              >
                <option value="">{form.party_id ? 'None (unapplied)' : 'Select a party first'}</option>
                {openDocs.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.number} — balance {Number(d.balance_due ?? 0).toFixed(2)}
                  </option>
                ))}
              </select>
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

        {isCustomer && (
          <div>
            <label className="label">Withholding Tax withheld (KES)</label>
            <input
              type="number"
              step="0.01"
              className="input"
              value={form.wht_amount}
              onChange={(e) => setForm({ ...form, wht_amount: +e.target.value })}
              placeholder="0.00"
            />
            <p className="text-xs text-gray-400 mt-1">
              If the customer withheld 5% WHT, enter the certificate amount (in KES). The cash received above
              plus this WHT clears the full invoice; the WHT posts to <span className="font-mono">1310 WHT Receivable</span>.
            </p>
            {form.wht_amount > 0 && form.apply_amount > 0 && (
              <p className="text-xs text-blue-600 mt-1">
                Cash {Number(form.amount).toFixed(2)} + WHT {Number(form.wht_amount).toFixed(2)} clears invoice balance {Number(form.apply_amount).toFixed(2)}.
              </p>
            )}
          </div>
        )}

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
