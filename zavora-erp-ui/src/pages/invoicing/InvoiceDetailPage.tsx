import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getInvoice, postInvoice, sendInvoice, createCreditNote, getAuditForObject, getPayments } from '../../api/client';
import type { Invoice, Payment, AuditEventEntry } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import {
  ArrowLeft, Send, CheckCircle, CreditCard, FileText,
  Clock, User, Calendar, Hash, Download, ReceiptText
} from 'lucide-react';

export default function InvoiceDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [showCreditNote, setShowCreditNote] = useState(false);

  const { data: invoice, isLoading } = useQuery<Invoice>({
    queryKey: ['invoice', id],
    queryFn: () => getInvoice(id!).then(r => r.data),
    enabled: !!id,
  });

  const { data: payments = [] } = useQuery<Payment[]>({
    queryKey: ['payments'],
    queryFn: () => getPayments().then(r => r.data),
  });

  const { data: auditEvents = [] } = useQuery<AuditEventEntry[]>({
    queryKey: ['audit', 'Invoice', id],
    queryFn: () => getAuditForObject('Invoice', id!).then(r => r.data),
    enabled: !!id,
  });

  const postMutation = useMutation({
    mutationFn: () => postInvoice(id!),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['invoice', id] }),
  });

  const sendMutation = useMutation({
    mutationFn: () => sendInvoice(id!, { channels: ['Email'] }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['invoice', id] }),
  });

  if (isLoading) {
    return (
      <div className="p-12 text-center">
        <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
        <p className="mt-3 text-sm text-gray-500">Loading invoice...</p>
      </div>
    );
  }

  if (!invoice) {
    return <div className="p-12 text-center text-gray-500">Invoice not found</div>;
  }

  const invoicePayments = payments.filter(p => p.party_id === invoice.customer_id);

  return (
    <div>
      <PageHeader
        title={`Invoice ${invoice.number}`}
        subtitle={`Created ${formatDate(invoice.created_at)}`}
        actions={
          <div className="flex items-center gap-2">
            <button onClick={() => navigate('/invoices')} className="btn-secondary">
              <ArrowLeft className="w-4 h-4" /> Back
            </button>
            {invoice.status === 'draft' && (
              <button onClick={() => postMutation.mutate()} className="btn-primary" disabled={postMutation.isPending}>
                <CheckCircle className="w-4 h-4" /> {postMutation.isPending ? 'Posting...' : 'Post Invoice'}
              </button>
            )}
            {(invoice.status === 'sent' || invoice.status === 'viewed') && (
              <>
                <button onClick={() => sendMutation.mutate()} className="btn-secondary" disabled={sendMutation.isPending}>
                  <Send className="w-4 h-4" /> Resend
                </button>
                <button className="btn-primary">
                  <CreditCard className="w-4 h-4" /> Record Payment
                </button>
              </>
            )}
            {invoice.status !== 'draft' && invoice.status !== 'voided' && (
              <button onClick={() => setShowCreditNote(true)} className="btn-secondary text-red-600 border-red-200 hover:bg-red-50">
                <ReceiptText className="w-4 h-4" /> Credit Note
              </button>
            )}
          </div>
        }
      />

      {/* Invoice Header */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 mb-6">
        <div className="card p-5 lg:col-span-2">
          <div className="flex items-start justify-between mb-4">
            <div>
              <div className="flex items-center gap-3 mb-1">
                <h2 className="text-lg font-semibold text-gray-900">{invoice.number}</h2>
                <span className={statusColor(invoice.status)}>{invoice.status.replace('_', ' ')}</span>
              </div>
              <p className="text-sm text-gray-500">
                {invoice.invoice_type === 'CreditNote' ? 'Credit Note' : 'Tax Invoice'}
              </p>
            </div>
            <button className="btn-secondary text-sm py-1.5 px-3">
              <Download className="w-3.5 h-3.5" /> PDF
            </button>
          </div>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div>
              <span className="text-gray-500 flex items-center gap-1"><User className="w-3.5 h-3.5" /> Customer</span>
              <p className="font-medium mt-0.5">{invoice.customer_id.slice(0, 8)}...</p>
            </div>
            <div>
              <span className="text-gray-500 flex items-center gap-1"><Calendar className="w-3.5 h-3.5" /> Issue Date</span>
              <p className="font-medium mt-0.5">{formatDate(invoice.issue_date)}</p>
            </div>
            <div>
              <span className="text-gray-500 flex items-center gap-1"><Clock className="w-3.5 h-3.5" /> Due Date</span>
              <p className={`font-medium mt-0.5 ${invoice.status === 'overdue' ? 'text-red-600' : ''}`}>
                {formatDate(invoice.due_date)}
              </p>
            </div>
            <div>
              <span className="text-gray-500 flex items-center gap-1"><Hash className="w-3.5 h-3.5" /> Currency</span>
              <p className="font-medium mt-0.5">{invoice.currency}</p>
            </div>
          </div>
        </div>

        {/* Totals card */}
        <div className="card p-5">
          <h3 className="text-sm font-medium text-gray-500 mb-3">Amount Summary</h3>
          <div className="space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-gray-600">Subtotal</span>
              <span>{formatCurrency(invoice.subtotal, invoice.currency)}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-600">Tax</span>
              <span>{formatCurrency(invoice.tax_total, invoice.currency)}</span>
            </div>
            <div className="flex justify-between border-t pt-2 font-medium">
              <span>Total</span>
              <span>{formatCurrency(invoice.gross_total, invoice.currency)}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-600">Paid</span>
              <span className="text-green-600">{formatCurrency(invoice.amount_paid, invoice.currency)}</span>
            </div>
            <div className="flex justify-between border-t pt-2 font-bold text-lg">
              <span>Balance Due</span>
              <span className={invoice.balance_due > 0 ? 'text-red-600' : 'text-green-600'}>
                {formatCurrency(invoice.balance_due, invoice.currency)}
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Line Items */}
      <div className="card overflow-hidden mb-6">
        <div className="px-5 py-3 border-b bg-gray-50">
          <h3 className="text-sm font-medium text-gray-700">Line Items</h3>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b text-xs font-medium text-gray-500 uppercase">
                <th className="px-5 py-3 text-left">Description</th>
                <th className="px-5 py-3 text-right">Qty</th>
                <th className="px-5 py-3 text-right">Unit Price</th>
                <th className="px-5 py-3 text-right">Tax</th>
                <th className="px-5 py-3 text-right">Amount</th>
              </tr>
            </thead>
            <tbody className="divide-y">
              {/* Since we only have the totals from the invoice summary, show a placeholder */}
              <tr>
                <td className="px-5 py-3 text-sm text-gray-600" colSpan={5}>
                  <div className="flex justify-between">
                    <span>Invoice line items</span>
                    <span className="font-medium">{formatCurrency(invoice.subtotal, invoice.currency)}</span>
                  </div>
                </td>
              </tr>
            </tbody>
            <tfoot className="bg-gray-50">
              <tr className="border-t">
                <td colSpan={4} className="px-5 py-2 text-sm text-right font-medium text-gray-600">Subtotal</td>
                <td className="px-5 py-2 text-sm text-right font-medium">{formatCurrency(invoice.subtotal, invoice.currency)}</td>
              </tr>
              <tr>
                <td colSpan={4} className="px-5 py-2 text-sm text-right text-gray-600">VAT</td>
                <td className="px-5 py-2 text-sm text-right">{formatCurrency(invoice.tax_total, invoice.currency)}</td>
              </tr>
              <tr className="border-t">
                <td colSpan={4} className="px-5 py-2 text-right font-bold">Total</td>
                <td className="px-5 py-2 text-right font-bold">{formatCurrency(invoice.gross_total, invoice.currency)}</td>
              </tr>
            </tfoot>
          </table>
        </div>
      </div>

      {/* Payment History & Audit Trail */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Payment History */}
        <div className="card">
          <div className="px-5 py-3 border-b bg-gray-50">
            <h3 className="text-sm font-medium text-gray-700">Payment History</h3>
          </div>
          <div className="p-5">
            {invoicePayments.length === 0 ? (
              <p className="text-sm text-gray-500 text-center py-4">No payments recorded yet</p>
            ) : (
              <div className="space-y-3">
                {invoicePayments.map(p => (
                  <div key={p.id} className="flex items-center justify-between text-sm">
                    <div>
                      <p className="font-medium">{p.reference}</p>
                      <p className="text-gray-500">{formatDate(p.payment_date)}</p>
                    </div>
                    <span className="font-medium text-green-600">
                      {formatCurrency(p.amount, p.currency)}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Audit Trail */}
        <div className="card">
          <div className="px-5 py-3 border-b bg-gray-50">
            <h3 className="text-sm font-medium text-gray-700">Audit Trail</h3>
          </div>
          <div className="p-5">
            {auditEvents.length === 0 ? (
              <p className="text-sm text-gray-500 text-center py-4">No audit events recorded</p>
            ) : (
              <div className="space-y-3">
                {auditEvents.slice(0, 10).map(evt => (
                  <div key={evt.id} className="flex items-start gap-3 text-sm">
                    <div className="w-2 h-2 mt-1.5 rounded-full bg-blue-400 shrink-0" />
                    <div>
                      <p className="font-medium">{evt.event_type}</p>
                      <p className="text-gray-500">{formatDate(evt.timestamp)}</p>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Notes */}
      {invoice.notes && (
        <div className="card p-5 mt-6">
          <h3 className="text-sm font-medium text-gray-700 mb-2">Notes</h3>
          <p className="text-sm text-gray-600">{invoice.notes}</p>
        </div>
      )}

      {/* Credit Note Modal */}
      {showCreditNote && (
        <CreditNoteModal invoiceId={id!} onClose={() => setShowCreditNote(false)} />
      )}
    </div>
  );
}

function CreditNoteModal({ invoiceId, onClose }: { invoiceId: string; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [reason, setReason] = useState('');

  const mutation = useMutation({
    mutationFn: (data: any) => createCreditNote(invoiceId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['invoice', invoiceId] });
      onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({ invoice_id: invoiceId, reason, lines: [] });
  };

  return (
    <Modal open={true} onClose={onClose} title="Create Credit Note" size="md">
      <form onSubmit={handleSubmit} className="space-y-4">
        <p className="text-sm text-gray-600">
          This will create a full reversal credit note for this invoice.
        </p>
        <div>
          <label className="label">Reason *</label>
          <textarea
            className="input"
            rows={3}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            placeholder="Reason for the credit note..."
            required
          />
        </div>
        <div className="flex justify-end gap-3 pt-2">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary bg-red-600 hover:bg-red-700" disabled={mutation.isPending || !reason}>
            {mutation.isPending ? 'Creating...' : 'Create Credit Note'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
