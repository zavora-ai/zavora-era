import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { getVendor, getBills, getPayments, getSupplierCreditNotes } from '../../api/client';
import type { Bill, Payment, SupplierCreditNote } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import { SkeletonCard } from '../../components/shared/Skeleton';
import ErrorRetry from '../../components/shared/ErrorRetry';
import {
  ArrowLeft, Building2, Mail, Phone, CreditCard, Receipt, FileMinus, FilePlus2, Pencil,
} from 'lucide-react';
import { VendorFormModal } from './VendorsPage';
import type { Vendor } from '../../types';

// The enriched vendor record returned by GET /vendors/{id}.
interface VendorDetail {
  id: string;
  name: string;
  kra_pin?: string;
  vat_number?: string;
  wht_category?: string;
  resident?: boolean;
  payment_terms?: string;
  currency?: string;
  is_active?: boolean;
  email?: { email: string; is_primary?: boolean }[];
  phone?: { number: string; is_primary?: boolean }[];
  total_billed?: number;
  total_paid?: number;
  total_credit_notes?: number;
  outstanding_balance?: number;
  bill_count?: number;
  payment_count?: number;
  credit_note_count?: number;
}

type Tab = 'bills' | 'payments' | 'credit_notes';

export default function VendorDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [tab, setTab] = useState<Tab>('bills');
  const [showEdit, setShowEdit] = useState(false);

  const { data: vendor, isLoading, isError, refetch } = useQuery<VendorDetail>({
    queryKey: ['vendor', id],
    queryFn: () => getVendor(id!).then(r => r.data),
    enabled: !!id,
  });

  const { data: bills = [] } = useQuery<Bill[]>({
    queryKey: ['bills', 'all'],
    queryFn: () => getBills({ limit: 500 }).then(r => r.data.data ?? r.data),
  });
  const { data: payments = [] } = useQuery<Payment[]>({
    queryKey: ['payments', 'all'],
    queryFn: () => getPayments({ limit: 500 }).then(r => r.data.data ?? r.data),
  });
  const { data: creditNotes = [] } = useQuery<SupplierCreditNote[]>({
    queryKey: ['supplier-credit-notes'],
    queryFn: () => getSupplierCreditNotes().then(r => Array.isArray(r.data) ? r.data : []),
  });

  if (isLoading) {
    return (
      <div>
        <PageHeader title="Vendor" subtitle="Supplier profile" />
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <SkeletonCard className="lg:col-span-2" />
          <SkeletonCard />
        </div>
      </div>
    );
  }
  if (isError || !vendor) {
    return (
      <div>
        <PageHeader title="Vendor" subtitle="Supplier profile" />
        <ErrorRetry message="Couldn't load this vendor." onRetry={() => refetch()} />
      </div>
    );
  }

  const vendorBills = bills.filter(b => b.vendor_id === id);
  const vendorPayments = payments.filter(p => p.party_id === id);
  const vendorCreditNotes = creditNotes.filter(c => c.vendor_id === id);

  const primaryEmail = vendor.email?.find(e => e.is_primary) || vendor.email?.[0];
  const primaryPhone = vendor.phone?.find(p => p.is_primary) || vendor.phone?.[0];

  const billColumns: Column<Bill>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status.replace('_', ' ')}</span> },
    { key: 'number', header: 'Bill #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'issue_date', header: 'Date', render: (r) => formatDate(r.issue_date) },
    { key: 'due_date', header: 'Due', render: (r) => formatDate(r.due_date) },
    { key: 'gross_total', header: 'Total', render: (r) => formatCurrency(r.gross_total, r.currency), className: 'text-right' },
    { key: 'balance_due', header: 'Balance', render: (r) => <span className="font-medium">{formatCurrency(r.balance_due, r.currency)}</span>, className: 'text-right' },
  ];
  const paymentColumns: Column<Payment>[] = [
    { key: 'payment_date', header: 'Date', render: (r) => formatDate(r.payment_date) },
    { key: 'reference', header: 'Reference', render: (r) => <span className="font-medium">{r.reference}</span> },
    { key: 'amount', header: 'Amount', render: (r) => <span className="font-medium text-green-600">{formatCurrency(r.amount, r.currency)}</span>, className: 'text-right' },
  ];
  const cnColumns: Column<SupplierCreditNote>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status}</span> },
    { key: 'credit_note_number', header: 'CN #', render: (r) => <span className="font-medium text-blue-600">{r.credit_note_number}</span> },
    { key: 'credit_note_date', header: 'Date', render: (r) => formatDate(r.credit_note_date) },
    { key: 'gross_total', header: 'Total', render: (r) => <span className="font-medium">{formatCurrency(r.gross_total)}</span>, className: 'text-right' },
  ];

  const tabs: { key: Tab; label: string; count: number }[] = [
    { key: 'bills', label: 'Bills', count: vendor.bill_count ?? vendorBills.length },
    { key: 'payments', label: 'Payments', count: vendor.payment_count ?? vendorPayments.length },
    { key: 'credit_notes', label: 'Credit Notes', count: vendor.credit_note_count ?? vendorCreditNotes.length },
  ];

  return (
    <div>
      <PageHeader
        title={vendor.name}
        subtitle="Supplier profile"
        actions={
          <div className="flex items-center gap-2">
            <button onClick={() => navigate('/vendors')} className="btn-secondary">
              <ArrowLeft className="w-4 h-4" /> Back
            </button>
            <button onClick={() => setShowEdit(true)} className="btn-secondary">
              <Pencil className="w-4 h-4" /> Edit
            </button>
            <button onClick={() => navigate(`/bills?new=1&vendor=${id}`)} className="btn-secondary">
              <FilePlus2 className="w-4 h-4" /> New Bill
            </button>
            <button onClick={() => navigate(`/payments?vendor=${id}`)} className="btn-primary">
              <CreditCard className="w-4 h-4" /> Record Payment
            </button>
          </div>
        }
      />

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 mb-6">
        {/* Vendor info */}
        <div className="card p-5 lg:col-span-2">
          <div className="flex items-start gap-4">
            <div className="w-12 h-12 rounded-full bg-gradient-to-br from-amber-400 to-orange-500 flex items-center justify-center shrink-0">
              <Building2 className="w-6 h-6 text-white" />
            </div>
            <div className="flex-1">
              <div className="flex items-center gap-2 mb-1">
                <h2 className="text-lg font-semibold text-gray-900">{vendor.name}</h2>
                <span className={vendor.is_active ? 'badge-success' : 'badge-gray'}>{vendor.is_active ? 'Active' : 'Inactive'}</span>
                {vendor.wht_category && <span className="badge-warning">{vendor.wht_category}</span>}
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3 mt-4 text-sm">
                {primaryEmail && (
                  <div className="flex items-center gap-2 text-gray-600"><Mail className="w-4 h-4 text-gray-400" /><span>{primaryEmail.email}</span></div>
                )}
                {primaryPhone && (
                  <div className="flex items-center gap-2 text-gray-600"><Phone className="w-4 h-4 text-gray-400" /><span>{primaryPhone.number}</span></div>
                )}
                {vendor.kra_pin && (
                  <div className="flex items-center gap-2 text-gray-600"><Building2 className="w-4 h-4 text-gray-400" /><span>KRA PIN: {vendor.kra_pin}</span></div>
                )}
                <div className="flex items-center gap-2 text-gray-600"><CreditCard className="w-4 h-4 text-gray-400" /><span>Terms: {vendor.payment_terms ?? '—'}</span></div>
                <div className="flex items-center gap-2 text-gray-600"><Receipt className="w-4 h-4 text-gray-400" /><span>{vendor.resident ? 'Kenyan Resident' : 'Non-Resident'}</span></div>
              </div>
            </div>
          </div>
        </div>

        {/* Balance summary */}
        <div className="card p-5">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Outstanding Balance</h3>
          <p className="text-2xl font-bold text-gray-900">{formatCurrency(vendor.outstanding_balance ?? 0, vendor.currency)}</p>
          <p className="text-xs text-gray-500 mt-0.5">Unpaid bills less open credit notes</p>
          <div className="mt-4 space-y-1.5 text-sm text-gray-600 border-t pt-3">
            <div className="flex justify-between"><span>Total billed</span><span className="font-medium">{formatCurrency(vendor.total_billed ?? 0, vendor.currency)}</span></div>
            <div className="flex justify-between"><span>Total paid</span><span className="font-medium text-green-600">{formatCurrency(vendor.total_paid ?? 0, vendor.currency)}</span></div>
            <div className="flex justify-between"><span>Credit notes</span><span className="font-medium">{formatCurrency(vendor.total_credit_notes ?? 0, vendor.currency)}</span></div>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-4 bg-gray-100 p-1 rounded-lg w-fit">
        {tabs.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${tab === t.key ? 'bg-white shadow-sm text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
          >
            {t.label} ({t.count})
          </button>
        ))}
      </div>

      {tab === 'bills' && (
        <DataTable columns={billColumns} data={vendorBills} emptyMessage="No bills for this vendor yet." />
      )}
      {tab === 'payments' && (
        <DataTable columns={paymentColumns} data={vendorPayments} emptyMessage="No payments recorded for this vendor." />
      )}
      {tab === 'credit_notes' && (
        <div>
          <DataTable columns={cnColumns} data={vendorCreditNotes} emptyMessage="No supplier credit notes for this vendor." />
          {vendorCreditNotes.length === 0 && (
            <p className="mt-2 text-xs text-gray-400 flex items-center gap-1"><FileMinus className="w-3.5 h-3.5" /> Credit notes reverse AP and input VAT.</p>
          )}
        </div>
      )}
      {showEdit && (
        <VendorFormModal
          vendor={vendor as unknown as Vendor}
          onClose={() => { setShowEdit(false); refetch(); }}
        />
      )}
    </div>
  );
}
