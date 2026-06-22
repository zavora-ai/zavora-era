import { useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { getBill, getVendor, getSettings } from '../../api/client';
import type { Bill, Vendor, BrandingConfig } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import DocumentLayout from '../../components/documents/DocumentLayout';
import DocumentLineItems, { type LineItem } from '../../components/documents/DocumentLineItems';
import DocumentActions from '../../components/documents/DocumentActions';

export default function BillPreview() {
  const { id } = useParams<{ id: string }>();

  const { data: billData, isLoading } = useQuery<{ bill: Bill; lines: any[] }>({
    queryKey: ['bill-preview', id],
    queryFn: () => getBill(id!).then(r => r.data),
    enabled: !!id,
  });

  const bill = billData?.bill ?? (billData as unknown as Bill);
  const rawLines: any[] = billData?.lines ?? [];

  const { data: vendor } = useQuery<Vendor>({
    queryKey: ['vendor', bill?.vendor_id],
    queryFn: () => getVendor(bill!.vendor_id).then(r => r.data),
    enabled: !!bill?.vendor_id,
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

  if (!bill) {
    return <div className="p-12 text-center text-gray-500">Bill not found</div>;
  }

  const isDraft = bill.status === 'draft';

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
          title="PURCHASE INVOICE / BILL"
          documentNumber={bill.number}
        >
          {/* Document header details */}
          <div className="grid grid-cols-2 gap-8 mb-8">
            <div>
              <h3 className="text-xs font-semibold text-gray-500 uppercase mb-1">Vendor</h3>
              <p className="text-sm font-medium text-gray-900">{vendor?.name ?? bill.vendor_id.slice(0, 8)}</p>
              {vendor?.address && (
                <p className="text-sm text-gray-600">
                  {[vendor.address.line1, vendor.address.line2, vendor.address.city, vendor.address.country].filter(Boolean).join(', ')}
                </p>
              )}
              {vendor?.kra_pin && <p className="text-sm text-gray-600">PIN: {vendor.kra_pin}</p>}
            </div>
            <div className="text-right space-y-1">
              <div className="text-sm">
                <span className="text-gray-500">Bill #: </span>
                <span className="font-medium text-gray-900">{bill.number}</span>
              </div>
              {bill.vendor_invoice_number && (
                <div className="text-sm">
                  <span className="text-gray-500">Vendor Inv #: </span>
                  <span className="text-gray-900">{bill.vendor_invoice_number}</span>
                </div>
              )}
              <div className="text-sm">
                <span className="text-gray-500">Issue Date: </span>
                <span className="text-gray-900">{formatDate(bill.issue_date)}</span>
              </div>
              <div className="text-sm">
                <span className="text-gray-500">Due Date: </span>
                <span className="text-gray-900">{formatDate(bill.due_date)}</span>
              </div>
              <div className="text-sm">
                <span className="text-gray-500">Currency: </span>
                <span className="text-gray-900">{bill.currency}</span>
              </div>
            </div>
          </div>

          {/* Line items */}
          {lines.length > 0 ? (
            <DocumentLineItems
              lines={lines}
              currency={bill.currency}
              subtotal={bill.subtotal}
              taxTotal={bill.tax_total}
              grossTotal={bill.gross_total}
            />
          ) : (
            <div className="border rounded-lg p-6 text-center text-sm text-gray-500">
              <div className="space-y-1">
                <div className="flex justify-between max-w-xs mx-auto">
                  <span>Subtotal</span><span className="font-medium">{formatCurrency(bill.subtotal, bill.currency)}</span>
                </div>
                <div className="flex justify-between max-w-xs mx-auto">
                  <span>VAT</span><span>{formatCurrency(bill.tax_total, bill.currency)}</span>
                </div>
                {bill.wht_amount > 0 && (
                  <div className="flex justify-between max-w-xs mx-auto">
                    <span>WHT</span><span className="text-orange-600">-{formatCurrency(bill.wht_amount, bill.currency)}</span>
                  </div>
                )}
                <div className="flex justify-between max-w-xs mx-auto border-t pt-1 font-bold">
                  <span>Total</span><span>{formatCurrency(bill.gross_total, bill.currency)}</span>
                </div>
              </div>
            </div>
          )}

          {/* WHT info if applicable */}
          {bill.wht_amount > 0 && lines.length > 0 && (
            <div className="mt-4 flex justify-end">
              <div className="text-sm text-orange-700 bg-orange-50 px-4 py-2 rounded-lg">
                Withholding Tax: {formatCurrency(bill.wht_amount, bill.currency)}
              </div>
            </div>
          )}

          {/* Balance due */}
          <div className="mt-6 flex justify-end">
            <div className="bg-gray-50 rounded-lg px-6 py-3 text-right space-y-1">
              <div className="text-sm flex justify-between gap-8">
                <span className="text-gray-600">Amount Paid</span>
                <span className="text-green-600 font-medium">{formatCurrency(bill.amount_paid, bill.currency)}</span>
              </div>
              <div className="text-base flex justify-between gap-8 border-t pt-1 font-bold">
                <span>Balance Due</span>
                <span className={bill.balance_due > 0 ? 'text-red-600' : 'text-green-600'}>
                  {formatCurrency(bill.balance_due, bill.currency)}
                </span>
              </div>
            </div>
          </div>

          {/* Notes */}
          {bill.notes && (
            <div className="mt-8 pt-6 border-t">
              <h4 className="text-xs font-semibold text-gray-500 uppercase mb-1">Notes</h4>
              <p className="text-sm text-gray-600 whitespace-pre-line">{bill.notes}</p>
            </div>
          )}
        </DocumentLayout>
      </div>
    </div>
  );
}
