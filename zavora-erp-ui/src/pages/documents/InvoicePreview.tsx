import { useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { getInvoice, getCustomer, getSettings } from '../../api/client';
import type { Invoice, Customer, BrandingConfig } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import DocumentLayout from '../../components/documents/DocumentLayout';
import DocumentLineItems, { type LineItem } from '../../components/documents/DocumentLineItems';
import DocumentActions from '../../components/documents/DocumentActions';

export default function InvoicePreview() {
  const { id } = useParams<{ id: string }>();

  const { data: invoiceData, isLoading: loadingInvoice } = useQuery<{ invoice: Invoice; lines: any[] }>({
    queryKey: ['invoice-preview', id],
    queryFn: () => getInvoice(id!).then(r => r.data),
    enabled: !!id,
  });

  const invoice = invoiceData?.invoice ?? (invoiceData as unknown as Invoice);
  const rawLines: any[] = invoiceData?.lines ?? [];

  const { data: customer } = useQuery<Customer>({
    queryKey: ['customer', invoice?.customer_id],
    queryFn: () => getCustomer(invoice!.customer_id).then(r => r.data),
    enabled: !!invoice?.customer_id,
  });

  const { data: settingsRes } = useQuery({ queryKey: ['settings'], queryFn: getSettings });
  const branding: BrandingConfig | undefined = settingsRes?.data?.branding;

  if (loadingInvoice) {
    return (
      <div className="p-12 text-center">
        <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
        <p className="mt-3 text-sm text-gray-500">Loading document…</p>
      </div>
    );
  }

  if (!invoice) {
    return <div className="p-12 text-center text-gray-500">Invoice not found</div>;
  }

  const isDraft = invoice.status === 'draft';
  const isCreditNote = invoice.invoice_type === 'CreditNote';
  const title = isCreditNote ? 'CREDIT NOTE' : isDraft ? 'PROFORMA INVOICE' : 'TAX INVOICE';

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
          title={title}
          documentNumber={invoice.number}
        >
          {/* Document header details */}
          <div className="grid grid-cols-2 gap-8 mb-8">
            <div>
              <h3 className="text-xs font-semibold text-gray-500 uppercase mb-1">Bill To</h3>
              <p className="text-sm font-medium text-gray-900">{customer?.name ?? invoice.customer_id.slice(0, 8)}</p>
              {customer?.address && (
                <p className="text-sm text-gray-600">
                  {[customer.address.line1, customer.address.line2, customer.address.city, customer.address.country].filter(Boolean).join(', ')}
                </p>
              )}
              {customer?.kra_pin && <p className="text-sm text-gray-600">PIN: {customer.kra_pin}</p>}
            </div>
            <div className="text-right space-y-1">
              <div className="text-sm">
                <span className="text-gray-500">Invoice #: </span>
                <span className="font-medium text-gray-900">{invoice.number}</span>
              </div>
              <div className="text-sm">
                <span className="text-gray-500">Issue Date: </span>
                <span className="text-gray-900">{formatDate(invoice.issue_date)}</span>
              </div>
              <div className="text-sm">
                <span className="text-gray-500">Due Date: </span>
                <span className="text-gray-900">{formatDate(invoice.due_date)}</span>
              </div>
              <div className="text-sm">
                <span className="text-gray-500">Currency: </span>
                <span className="text-gray-900">{invoice.currency}</span>
              </div>
              {isCreditNote && invoice.credit_note_for && (
                <div className="text-sm">
                  <span className="text-gray-500">Applies to: </span>
                  <span className="text-gray-900">{invoice.credit_note_for}</span>
                </div>
              )}
            </div>
          </div>

          {/* eTIMS badge */}
          {!isCreditNote && invoice.etims_status === 'transmitted' && invoice.etims_invoice_number && (
            <div className="mb-6 inline-flex items-center gap-2 px-3 py-1.5 rounded-lg bg-green-50 border border-green-200">
              <span className="text-xs font-medium text-green-700">eTIMS Control No: {invoice.etims_invoice_number}</span>
            </div>
          )}

          {/* Line items */}
          {lines.length > 0 ? (
            <DocumentLineItems
              lines={lines}
              currency={invoice.currency}
              subtotal={invoice.subtotal}
              taxTotal={invoice.tax_total}
              grossTotal={invoice.gross_total}
            />
          ) : (
            <div className="border rounded-lg p-6 text-center text-sm text-gray-500">
              <p>Line items summary</p>
              <div className="mt-4 space-y-1">
                <div className="flex justify-between max-w-xs mx-auto">
                  <span>Subtotal</span><span className="font-medium">{formatCurrency(invoice.subtotal, invoice.currency)}</span>
                </div>
                <div className="flex justify-between max-w-xs mx-auto">
                  <span>VAT</span><span>{formatCurrency(invoice.tax_total, invoice.currency)}</span>
                </div>
                <div className="flex justify-between max-w-xs mx-auto border-t pt-1 font-bold">
                  <span>Total</span><span>{formatCurrency(invoice.gross_total, invoice.currency)}</span>
                </div>
              </div>
            </div>
          )}

          {/* Balance due */}
          <div className="mt-6 flex justify-end">
            <div className="bg-gray-50 rounded-lg px-6 py-3 text-right space-y-1">
              <div className="text-sm flex justify-between gap-8">
                <span className="text-gray-600">Amount Paid</span>
                <span className="text-green-600 font-medium">{formatCurrency(invoice.amount_paid, invoice.currency)}</span>
              </div>
              <div className="text-base flex justify-between gap-8 border-t pt-1 font-bold">
                <span>Balance Due</span>
                <span className={invoice.balance_due > 0 ? 'text-red-600' : 'text-green-600'}>
                  {formatCurrency(invoice.balance_due, invoice.currency)}
                </span>
              </div>
            </div>
          </div>

          {/* Notes */}
          {invoice.notes && (
            <div className="mt-8 pt-6 border-t">
              <h4 className="text-xs font-semibold text-gray-500 uppercase mb-1">Notes</h4>
              <p className="text-sm text-gray-600 whitespace-pre-line">{invoice.notes}</p>
            </div>
          )}
        </DocumentLayout>
      </div>
    </div>
  );
}
