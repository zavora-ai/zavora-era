import { useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { getPayment, getCustomer, getVendor, getSettings } from '../../api/client';
import type { Payment, Customer, Vendor, BrandingConfig } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import DocumentLayout from '../../components/documents/DocumentLayout';
import DocumentActions from '../../components/documents/DocumentActions';

export default function PaymentReceiptPreview() {
  const { id } = useParams<{ id: string }>();

  const { data: payment, isLoading, isError } = useQuery<Payment>({
    queryKey: ['payment-preview', id],
    queryFn: () => getPayment(id!).then(r => r.data),
    enabled: !!id,
  });

  const isCustomerPayment = payment?.payment_type === 'customer_payment';

  const { data: party } = useQuery<Customer | Vendor>({
    queryKey: [isCustomerPayment ? 'customer' : 'vendor', payment?.party_id],
    queryFn: () =>
      isCustomerPayment
        ? getCustomer(payment!.party_id).then(r => r.data)
        : getVendor(payment!.party_id).then(r => r.data),
    enabled: !!payment?.party_id,
  });

  const { data: settingsRes } = useQuery({ queryKey: ['settings'], queryFn: getSettings });
  const branding: BrandingConfig | undefined = settingsRes?.data?.branding;

  if (isLoading) {
    return (
      <div className="p-12 text-center">
        <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
        <p className="mt-3 text-sm text-gray-500">Loading document…</p>
      </div>
    );
  }

  if (isError || !payment) {
    return (
      <div className="p-12 text-center text-gray-500">
        <p>Payment receipt not found.</p>
        <p className="text-xs mt-2 text-gray-400">
          {/* TODO: The backend may not have a GET /payments/:id endpoint yet. */}
          If this persists, the payment detail endpoint may not be available.
        </p>
      </div>
    );
  }

  const methodLabel = typeof payment.method === 'string' ? payment.method : JSON.stringify(payment.method);

  return (
    <div className="p-6">
      <DocumentActions />

      <DocumentLayout
        branding={branding}
        title="PAYMENT RECEIPT"
        documentNumber={payment.number}
      >
        {/* Receipt details */}
        <div className="grid grid-cols-2 gap-8 mb-8">
          <div>
            <h3 className="text-xs font-semibold text-gray-500 uppercase mb-1">
              {isCustomerPayment ? 'Received From' : 'Paid To'}
            </h3>
            <p className="text-sm font-medium text-gray-900">{party?.name ?? payment.party_id.slice(0, 8)}</p>
          </div>
          <div className="text-right space-y-1">
            <div className="text-sm">
              <span className="text-gray-500">Receipt #: </span>
              <span className="font-medium text-gray-900">{payment.number}</span>
            </div>
            <div className="text-sm">
              <span className="text-gray-500">Date: </span>
              <span className="text-gray-900">{formatDate(payment.payment_date)}</span>
            </div>
            <div className="text-sm">
              <span className="text-gray-500">Method: </span>
              <span className="text-gray-900">{methodLabel}</span>
            </div>
            {payment.reference && (
              <div className="text-sm">
                <span className="text-gray-500">Reference: </span>
                <span className="text-gray-900">{payment.reference}</span>
              </div>
            )}
          </div>
        </div>

        {/* Amount */}
        <div className="border rounded-lg p-6 text-center">
          <p className="text-sm text-gray-500 mb-1">Amount {isCustomerPayment ? 'Received' : 'Paid'}</p>
          <p className="text-3xl font-bold text-gray-900">{formatCurrency(payment.amount, payment.currency)}</p>
          <p className="text-sm text-gray-500 mt-1">{payment.currency}</p>
        </div>

        {/* Applications */}
        {payment.applications && payment.applications.length > 0 && (
          <div className="mt-6">
            <h4 className="text-xs font-semibold text-gray-500 uppercase mb-2">Applied To</h4>
            <div className="border rounded-lg overflow-hidden">
              <table className="w-full text-sm">
                <thead>
                  <tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
                    <th className="px-4 py-2 text-left">Document</th>
                    <th className="px-4 py-2 text-left">Type</th>
                    <th className="px-4 py-2 text-right">Amount Applied</th>
                  </tr>
                </thead>
                <tbody className="divide-y">
                  {payment.applications.map((app, i) => (
                    <tr key={i}>
                      <td className="px-4 py-2 text-gray-900">{app.document_id.slice(0, 8)}...</td>
                      <td className="px-4 py-2 text-gray-600">{app.document_type}</td>
                      <td className="px-4 py-2 text-right font-medium">{formatCurrency(app.amount_applied, payment.currency)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {payment.unapplied > 0 && (
          <div className="mt-4 text-sm text-amber-700 bg-amber-50 px-4 py-2 rounded-lg inline-block">
            Unapplied balance: {formatCurrency(payment.unapplied, payment.currency)}
          </div>
        )}
      </DocumentLayout>
    </div>
  );
}
