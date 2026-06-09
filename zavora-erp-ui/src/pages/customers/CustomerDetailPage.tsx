import { useParams, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { getCustomer, getCustomerStatement, getInvoices, getPayments } from '../../api/client';
import type { Customer, Invoice, Payment } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import {
  ArrowLeft, Download, Mail, Phone, MapPin, CreditCard,
  FileText, User, Building2
} from 'lucide-react';

export default function CustomerDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const { data: customer, isLoading } = useQuery<Customer>({
    queryKey: ['customer', id],
    queryFn: () => getCustomer(id!).then(r => r.data),
    enabled: !!id,
  });

  const { data: invoices = [] } = useQuery<Invoice[]>({
    queryKey: ['invoices'],
    queryFn: () => getInvoices().then(r => r.data),
  });

  const { data: payments = [] } = useQuery<Payment[]>({
    queryKey: ['payments'],
    queryFn: () => getPayments().then(r => r.data),
  });

  if (isLoading) {
    return (
      <div className="p-12 text-center">
        <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
        <p className="mt-3 text-sm text-gray-500">Loading customer...</p>
      </div>
    );
  }

  if (!customer) {
    return <div className="p-12 text-center text-gray-500">Customer not found</div>;
  }

  const customerInvoices = invoices.filter(inv => inv.customer_id === id);
  const customerPayments = payments.filter(p => p.party_id === id);
  const outstandingBalance = customerInvoices.reduce((sum, inv) => sum + inv.balance_due, 0);
  const overdueInvoices = customerInvoices.filter(inv => inv.status === 'overdue');

  const handleDownloadStatement = async () => {
    try {
      await getCustomerStatement(id!);
      // In production this would trigger a download
    } catch {
      // Silently handle
    }
  };

  const invoiceColumns: Column<Invoice>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status.replace('_', ' ')}</span> },
    { key: 'number', header: 'Invoice #', render: (r) => (
      <button onClick={() => navigate(`/invoices/${r.id}`)} className="font-medium text-blue-600 hover:underline">
        {r.number}
      </button>
    )},
    { key: 'issue_date', header: 'Date', render: (r) => formatDate(r.issue_date) },
    { key: 'due_date', header: 'Due', render: (r) => <span className={r.status === 'overdue' ? 'text-red-600' : ''}>{formatDate(r.due_date)}</span> },
    { key: 'gross_total', header: 'Total', render: (r) => formatCurrency(r.gross_total, r.currency), className: 'text-right' },
    { key: 'balance_due', header: 'Balance', render: (r) => <span className="font-medium">{formatCurrency(r.balance_due, r.currency)}</span>, className: 'text-right' },
  ];

  const paymentColumns: Column<Payment>[] = [
    { key: 'payment_date', header: 'Date', render: (r) => formatDate(r.payment_date) },
    { key: 'reference', header: 'Reference', render: (r) => <span className="font-medium">{r.reference}</span> },
    { key: 'method', header: 'Method', render: (r) => <span className="capitalize">{typeof r.method === 'string' ? r.method : 'Bank Transfer'}</span> },
    { key: 'amount', header: 'Amount', render: (r) => <span className="font-medium text-green-600">{formatCurrency(r.amount, r.currency)}</span>, className: 'text-right' },
  ];

  const primaryEmail = customer.email?.find(e => e.is_primary) || customer.email?.[0];
  const primaryPhone = customer.phone?.find(p => p.is_primary) || customer.phone?.[0];

  return (
    <div>
      <PageHeader
        title={customer.name}
        subtitle="Customer Profile"
        actions={
          <div className="flex items-center gap-2">
            <button onClick={() => navigate('/customers')} className="btn-secondary">
              <ArrowLeft className="w-4 h-4" /> Back
            </button>
            <button onClick={handleDownloadStatement} className="btn-secondary">
              <Download className="w-4 h-4" /> Statement
            </button>
            <button onClick={() => navigate('/invoices')} className="btn-primary">
              <FileText className="w-4 h-4" /> New Invoice
            </button>
          </div>
        }
      />

      {/* Customer Info + Balance Summary */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 mb-6">
        {/* Customer Info Card */}
        <div className="card p-5 lg:col-span-2">
          <div className="flex items-start gap-4">
            <div className="w-12 h-12 rounded-full bg-gradient-to-br from-blue-400 to-indigo-500 flex items-center justify-center shrink-0">
              <User className="w-6 h-6 text-white" />
            </div>
            <div className="flex-1">
              <div className="flex items-center gap-2 mb-1">
                <h2 className="text-lg font-semibold text-gray-900">{customer.name}</h2>
                <span className={customer.is_active ? 'badge-success' : 'badge-gray'}>
                  {customer.is_active ? 'Active' : 'Inactive'}
                </span>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-3 mt-4 text-sm">
                {primaryEmail && (
                  <div className="flex items-center gap-2 text-gray-600">
                    <Mail className="w-4 h-4 text-gray-400" />
                    <span>{primaryEmail.email}</span>
                  </div>
                )}
                {primaryPhone && (
                  <div className="flex items-center gap-2 text-gray-600">
                    <Phone className="w-4 h-4 text-gray-400" />
                    <span>{primaryPhone.number}</span>
                  </div>
                )}
                {customer.kra_pin && (
                  <div className="flex items-center gap-2 text-gray-600">
                    <Building2 className="w-4 h-4 text-gray-400" />
                    <span>KRA PIN: {customer.kra_pin}</span>
                  </div>
                )}
                <div className="flex items-center gap-2 text-gray-600">
                  <CreditCard className="w-4 h-4 text-gray-400" />
                  <span>Terms: {customer.payment_terms}</span>
                </div>
              </div>

              {customer.credit_limit && (
                <div className="mt-3 text-sm text-gray-500">
                  Credit limit: {formatCurrency(customer.credit_limit, customer.currency)}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Balance Summary */}
        <div className="card p-5">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Outstanding Balance</h3>
          <div className="space-y-3">
            <div>
              <p className="text-2xl font-bold text-gray-900">{formatCurrency(outstandingBalance, customer.currency)}</p>
              <p className="text-xs text-gray-500 mt-0.5">Total outstanding</p>
            </div>
            {overdueInvoices.length > 0 && (
              <div className="bg-red-50 border border-red-100 rounded-lg p-3">
                <p className="text-sm font-medium text-red-700">
                  {overdueInvoices.length} overdue invoice{overdueInvoices.length > 1 ? 's' : ''}
                </p>
                <p className="text-xs text-red-500 mt-0.5">
                  {formatCurrency(overdueInvoices.reduce((s, i) => s + i.balance_due, 0), customer.currency)} overdue
                </p>
              </div>
            )}
            <div className="text-sm text-gray-600">
              <p>Total invoices: {customerInvoices.length}</p>
              <p>Total payments: {customerPayments.length}</p>
            </div>
          </div>
        </div>
      </div>

      {/* Recent Invoices */}
      <div className="mb-6">
        <h3 className="text-sm font-medium text-gray-700 mb-3">Recent Invoices</h3>
        <DataTable
          columns={invoiceColumns}
          data={customerInvoices.slice(0, 10)}
          emptyMessage="No invoices for this customer"
          onRowClick={(r) => navigate(`/invoices/${r.id}`)}
        />
      </div>

      {/* Payment History */}
      <div>
        <h3 className="text-sm font-medium text-gray-700 mb-3">Payment History</h3>
        <DataTable
          columns={paymentColumns}
          data={customerPayments.slice(0, 10)}
          emptyMessage="No payments recorded for this customer"
        />
      </div>
    </div>
  );
}
