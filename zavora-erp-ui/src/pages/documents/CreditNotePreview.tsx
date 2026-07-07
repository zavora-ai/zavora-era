import { useParams } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getInvoice, getCustomer, getSettings, transmitInvoiceKra } from '../../api/client';
import type { Invoice, Customer, BrandingConfig } from '../../types';
import { formatCurrency, formatDate } from '../../utils/format';
import DocumentLayout from '../../components/documents/DocumentLayout';
import DocumentLineItems, { type LineItem } from '../../components/documents/DocumentLineItems';
import DocumentActions from '../../components/documents/DocumentActions';
import { ShieldCheck } from 'lucide-react';

/** eTIMS status + real KRA transmission for a credit note (a credit/refund receipt). */
function EtimsCreditNoteBar({ inv }: { inv: any }) {
  const qc = useQueryClient();
  const status = inv.etims_status ?? 'not_transmitted';
  const transmitted = status === 'transmitted';
  const mut = useMutation({
    mutationFn: () => transmitInvoiceKra(inv.id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['credit-note-preview', inv.id] }),
    onError: (e: any) => window.alert(e?.response?.data?.error || 'Transmission failed.'),
  });
  const badge = transmitted ? 'bg-green-100 text-green-700'
    : status === 'transmission_failed' ? 'bg-red-100 text-red-700' : 'bg-gray-100 text-gray-600';
  const label = transmitted ? 'Transmitted to KRA'
    : status === 'transmission_failed' ? 'Transmission failed' : 'Not transmitted';
  return (
    <div className="print:hidden flex items-center gap-3 mb-4 flex-wrap">
      <span className={`px-2 py-0.5 rounded text-xs font-medium ${badge}`}>eTIMS: {label}</span>
      {!transmitted && (
        <button onClick={() => mut.mutate()} disabled={mut.isPending} className="btn-secondary text-sm">
          <ShieldCheck className="w-4 h-4" /> {mut.isPending ? 'Transmitting…' : 'Transmit to KRA'}
        </button>
      )}
    </div>
  );
}

export default function CreditNotePreview() {
  const { id } = useParams<{ id: string }>();

  const { data: invoiceData, isLoading } = useQuery<{ invoice: Invoice; lines: any[] }>({
    queryKey: ['credit-note-preview', id],
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

  if (isLoading) {
    return (
      <div className="p-12 text-center">
        <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
        <p className="mt-3 text-sm text-gray-500">Loading document…</p>
      </div>
    );
  }

  if (!invoice) {
    return <div className="p-12 text-center text-gray-500">Credit note not found</div>;
  }

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
      <EtimsCreditNoteBar inv={invoice} />

      <DocumentLayout
        branding={branding}
        title="CREDIT NOTE"
        subtitle={invoice.credit_note_for ? `Ref: Invoice ${invoice.credit_note_for}` : undefined}
        documentNumber={invoice.number}
      >
        {/* Document header details */}
        <div className="grid grid-cols-2 gap-8 mb-8">
          <div>
            <h3 className="text-xs font-semibold text-gray-500 uppercase mb-1">Issued To</h3>
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
              <span className="text-gray-500">Credit Note #: </span>
              <span className="font-medium text-gray-900">{invoice.number}</span>
            </div>
            <div className="text-sm">
              <span className="text-gray-500">Issue Date: </span>
              <span className="text-gray-900">{formatDate(invoice.issue_date)}</span>
            </div>
            {invoice.credit_note_for && (
              <div className="text-sm">
                <span className="text-gray-500">Original Invoice: </span>
                <span className="text-gray-900">{invoice.credit_note_for}</span>
              </div>
            )}
            <div className="text-sm">
              <span className="text-gray-500">Currency: </span>
              <span className="text-gray-900">{invoice.currency}</span>
            </div>
          </div>
        </div>

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
            <div className="space-y-1">
              <div className="flex justify-between max-w-xs mx-auto">
                <span>Subtotal</span><span className="font-medium">{formatCurrency(invoice.subtotal, invoice.currency)}</span>
              </div>
              <div className="flex justify-between max-w-xs mx-auto">
                <span>VAT</span><span>{formatCurrency(invoice.tax_total, invoice.currency)}</span>
              </div>
              <div className="flex justify-between max-w-xs mx-auto border-t pt-1 font-bold">
                <span>Total Credit</span><span>{formatCurrency(invoice.gross_total, invoice.currency)}</span>
              </div>
            </div>
          </div>
        )}

        {/* KRA eTIMS control block (once signed by KRA) */}
        {(invoice as any).etims_status === 'transmitted' && (
          <div className="mt-8 pt-6 border-t text-xs text-gray-600">
            <h4 className="text-xs font-semibold text-gray-500 uppercase mb-1">KRA eTIMS</h4>
            <div className="grid grid-cols-2 gap-x-8 gap-y-0.5">
              {(invoice as any).etims_sdc_id && <div><span className="text-gray-400">SCU ID: </span>{(invoice as any).etims_sdc_id}</div>}
              {(invoice as any).etims_rcpt_no != null && <div><span className="text-gray-400">Receipt No: </span>{(invoice as any).etims_rcpt_no}</div>}
              {(invoice as any).etims_invc_no != null && <div><span className="text-gray-400">Invoice No: </span>{(invoice as any).etims_invc_no}</div>}
              {(invoice as any).etims_rcpt_sign && <div className="col-span-2 break-all"><span className="text-gray-400">Signature: </span>{(invoice as any).etims_rcpt_sign}</div>}
            </div>
          </div>
        )}

        {/* Notes */}
        {invoice.notes && (
          <div className="mt-8 pt-6 border-t">
            <h4 className="text-xs font-semibold text-gray-500 uppercase mb-1">Reason / Notes</h4>
            <p className="text-sm text-gray-600 whitespace-pre-line">{invoice.notes}</p>
          </div>
        )}
      </DocumentLayout>
    </div>
  );
}
