import { useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { getEstimate, getCustomer, getSettings } from '../../api/client';
import type { Estimate, Customer, BrandingConfig } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import DocumentLayout from '../../components/documents/DocumentLayout';
import DocumentLineItems, { type LineItem } from '../../components/documents/DocumentLineItems';
import DocumentActions from '../../components/documents/DocumentActions';

export default function EstimatePreview() {
  const { id } = useParams<{ id: string }>();

  const { data: estimateData, isLoading } = useQuery<{ estimate: Estimate; lines: any[] }>({
    queryKey: ['estimate-preview', id],
    queryFn: () => getEstimate(id!).then(r => r.data),
    enabled: !!id,
  });

  const estimate = estimateData?.estimate ?? (estimateData as unknown as Estimate);
  const rawLines: any[] = estimateData?.lines ?? [];

  const { data: customer } = useQuery<Customer>({
    queryKey: ['customer', estimate?.customer_id],
    queryFn: () => getCustomer(estimate!.customer_id).then(r => r.data),
    enabled: !!estimate?.customer_id,
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

  if (!estimate) {
    return <div className="p-12 text-center text-gray-500">Estimate not found</div>;
  }

  const isDraft = estimate.status === 'draft';

  const lines: LineItem[] = rawLines.map((l: any) => ({
    description: l.description || '',
    quantity: l.quantity ?? 1,
    unit_price: l.unit_price ?? 0,
    discount_percent: l.discount_percent,
    vat_amount: l.vat_amount,
    line_total: l.line_total ?? (l.quantity ?? 1) * (l.unit_price ?? 0),
  }));

  return (
    <div className="p-6">
      <DocumentActions />

      <div className={isDraft ? 'draft-watermark' : ''}>
        <DocumentLayout
          branding={branding}
          title="ESTIMATE / QUOTATION"
          documentNumber={estimate.number}
        >
          {/* Document header details */}
          <div className="grid grid-cols-2 gap-8 mb-8">
            <div>
              <h3 className="text-xs font-semibold text-gray-500 uppercase mb-1">Prepared For</h3>
              <p className="text-sm font-medium text-gray-900">{customer?.name ?? estimate.customer_id.slice(0, 8)}</p>
              {customer?.address && (
                <p className="text-sm text-gray-600">
                  {[customer.address.line1, customer.address.line2, customer.address.city, customer.address.country].filter(Boolean).join(', ')}
                </p>
              )}
              {customer?.kra_pin && <p className="text-sm text-gray-600">PIN: {customer.kra_pin}</p>}
            </div>
            <div className="text-right space-y-1">
              <div className="text-sm">
                <span className="text-gray-500">Estimate #: </span>
                <span className="font-medium text-gray-900">{estimate.number}</span>
              </div>
              <div className="text-sm">
                <span className="text-gray-500">Issue Date: </span>
                <span className="text-gray-900">{formatDate(estimate.issue_date)}</span>
              </div>
              <div className="text-sm">
                <span className="text-gray-500">Expiry Date: </span>
                <span className="text-gray-900">{formatDate(estimate.expiry_date)}</span>
              </div>
              <div className="text-sm">
                <span className="text-gray-500">Currency: </span>
                <span className="text-gray-900">{estimate.currency}</span>
              </div>
            </div>
          </div>

          {/* Line items */}
          {lines.length > 0 ? (
            <DocumentLineItems
              lines={lines}
              currency={estimate.currency}
              subtotal={estimate.subtotal}
              taxTotal={estimate.tax_total}
              grossTotal={estimate.gross_total}
            />
          ) : (
            <div className="border rounded-lg p-6 text-center text-sm text-gray-500">
              <p>Line items summary</p>
              <div className="mt-4 space-y-1">
                <div className="flex justify-between max-w-xs mx-auto">
                  <span>Subtotal</span><span className="font-medium">{formatCurrency(estimate.subtotal, estimate.currency)}</span>
                </div>
                <div className="flex justify-between max-w-xs mx-auto">
                  <span>VAT</span><span>{formatCurrency(estimate.tax_total, estimate.currency)}</span>
                </div>
                <div className="flex justify-between max-w-xs mx-auto border-t pt-1 font-bold">
                  <span>Total</span><span>{formatCurrency(estimate.gross_total, estimate.currency)}</span>
                </div>
              </div>
            </div>
          )}

          {/* Notes */}
          {estimate.notes && (
            <div className="mt-8 pt-6 border-t">
              <h4 className="text-xs font-semibold text-gray-500 uppercase mb-1">Notes & Terms</h4>
              <p className="text-sm text-gray-600 whitespace-pre-line">{estimate.notes}</p>
            </div>
          )}

          {/* Validity notice */}
          <div className="mt-6 text-xs text-gray-400 text-center">
            This quotation is valid until {formatDate(estimate.expiry_date)}.
          </div>
        </DocumentLayout>
      </div>
    </div>
  );
}
