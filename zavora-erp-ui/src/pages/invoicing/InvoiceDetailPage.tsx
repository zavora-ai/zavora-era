import { useState } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getInvoice, postInvoice, sendInvoice, createCreditNote, transmitInvoiceEtims, getAuditForObject, getPayments, mpesaStkPush } from '../../api/client';
import type { Invoice, Payment, AuditEventEntry } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { hasRole, ROLES_POST, ROLES_SEND, ROLES_CREATE } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import {
  ArrowLeft, Send, CheckCircle, CreditCard,
  Clock, User, Calendar, Hash, Download, ReceiptText, Phone, Loader2, ShieldCheck, FileText
} from 'lucide-react';

const ETIMS_BADGE: Record<string, { label: string; cls: string }> = {
  not_transmitted: { label: 'eTIMS: Not transmitted', cls: 'bg-gray-100 text-gray-600' },
  transmitted: { label: 'eTIMS: Transmitted to KRA', cls: 'bg-green-100 text-green-700' },
  transmission_failed: { label: 'eTIMS: Transmission failed', cls: 'bg-red-100 text-red-700' },
};

export default function InvoiceDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [showCreditNote, setShowCreditNote] = useState(false);
  const [showTransmit, setShowTransmit] = useState(false);
  const [showMpesaModal, setShowMpesaModal] = useState(false);
  const [mpesaNotification, setMpesaNotification] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

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
            <Link to={`/documents/invoice/${id}`} className="btn-secondary">
              <FileText className="w-4 h-4" /> Preview
            </Link>
            {invoice.status === 'draft' && hasRole(ROLES_POST) && (
              <button onClick={() => postMutation.mutate()} className="btn-primary" disabled={postMutation.isPending}>
                <CheckCircle className="w-4 h-4" /> {postMutation.isPending ? 'Posting...' : 'Post Invoice'}
              </button>
            )}
            {(invoice.status === 'sent' || invoice.status === 'viewed') && (
              <>
                {hasRole(ROLES_SEND) && (
                  <button onClick={() => sendMutation.mutate()} className="btn-secondary" disabled={sendMutation.isPending}>
                    <Send className="w-4 h-4" /> Resend
                  </button>
                )}
                <button className="btn-primary">
                  <CreditCard className="w-4 h-4" /> Record Payment
                </button>
              </>
            )}
            {invoice.status !== 'draft' && invoice.status !== 'voided' && hasRole(ROLES_CREATE) && (
              <button onClick={() => setShowCreditNote(true)} className="btn-secondary text-red-600 border-red-200 hover:bg-red-50">
                <ReceiptText className="w-4 h-4" /> Credit Note
              </button>
            )}
            {invoice.invoice_type !== 'CreditNote'
              && invoice.status !== 'draft' && invoice.status !== 'voided'
              && invoice.etims_status !== 'transmitted' && hasRole(ROLES_SEND) && (
              <button onClick={() => setShowTransmit(true)} className="btn-secondary text-indigo-600 border-indigo-200 hover:bg-indigo-50">
                <ShieldCheck className="w-4 h-4" /> Transmit to eTIMS
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
              {invoice.invoice_type !== 'CreditNote' && (
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <span className={`px-2 py-0.5 rounded text-xs font-medium ${(ETIMS_BADGE[invoice.etims_status ?? 'not_transmitted'] ?? ETIMS_BADGE.not_transmitted).cls}`}>
                    {(ETIMS_BADGE[invoice.etims_status ?? 'not_transmitted'] ?? ETIMS_BADGE.not_transmitted).label}
                  </span>
                  {invoice.etims_status === 'transmitted' && invoice.etims_invoice_number && (
                    <span className="text-xs text-gray-500">No. {invoice.etims_invoice_number}{invoice.etims_transmitted_at ? ` · ${formatDate(invoice.etims_transmitted_at)}` : ''}</span>
                  )}
                </div>
              )}
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

      {/* M-Pesa Payment Section */}
      {(invoice.status === 'sent' || invoice.status === 'viewed' || invoice.status === 'partially_paid' || invoice.status === 'overdue') && (
        <div className="card p-5 mt-6">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-sm font-medium text-gray-700">Mobile Payment</h3>
              <p className="text-xs text-gray-500 mt-0.5">Pay this invoice using M-Pesa STK Push</p>
            </div>
            <button
              onClick={() => setShowMpesaModal(true)}
              className="inline-flex items-center gap-2 px-4 py-2 bg-green-600 text-white text-sm font-medium rounded-lg hover:bg-green-700 transition-colors"
            >
              <Phone className="w-4 h-4" />
              Pay with M-Pesa
            </button>
          </div>
        </div>
      )}

      {/* M-Pesa Notification Toast */}
      {mpesaNotification && (
        <div className={`fixed bottom-6 right-6 z-50 max-w-sm px-4 py-3 rounded-lg shadow-lg text-sm font-medium flex items-center gap-2 animate-slideUp ${
          mpesaNotification.type === 'success'
            ? 'bg-green-50 text-green-800 border border-green-200'
            : 'bg-red-50 text-red-800 border border-red-200'
        }`}>
          {mpesaNotification.type === 'success' ? (
            <CheckCircle className="w-4 h-4 text-green-600 shrink-0" />
          ) : (
            <Phone className="w-4 h-4 text-red-600 shrink-0" />
          )}
          <span>{mpesaNotification.message}</span>
          <button
            onClick={() => setMpesaNotification(null)}
            className="ml-auto text-gray-400 hover:text-gray-600"
          >
            ×
          </button>
        </div>
      )}

      {/* M-Pesa Payment Modal */}
      {showMpesaModal && (
        <MpesaPaymentModal
          invoiceId={id!}
          balanceDue={invoice.balance_due}
          currency={invoice.currency}
          onClose={() => setShowMpesaModal(false)}
          onSuccess={() => {
            setShowMpesaModal(false);
            setMpesaNotification({ type: 'success', message: 'M-Pesa payment initiated successfully. You will receive a prompt on your phone.' });
            queryClient.invalidateQueries({ queryKey: ['invoice', id] });
            setTimeout(() => setMpesaNotification(null), 8000);
          }}
          onError={(msg) => {
            setMpesaNotification({ type: 'error', message: msg });
            setTimeout(() => setMpesaNotification(null), 8000);
          }}
        />
      )}

      {/* Credit Note Modal */}
      {showCreditNote && (
        <CreditNoteModal invoiceId={id!} onClose={() => setShowCreditNote(false)} />
      )}

      {/* eTIMS Transmit Modal */}
      {showTransmit && (
        <TransmitEtimsModal invoiceId={id!} onClose={() => setShowTransmit(false)} />
      )}
    </div>
  );
}

function TransmitEtimsModal({ invoiceId, onClose }: { invoiceId: string; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [etimsNumber, setEtimsNumber] = useState('');
  const [error, setError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => transmitInvoiceEtims(invoiceId, { etims_invoice_number: etimsNumber.trim() || undefined }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['invoice', invoiceId] });
      onClose();
    },
    onError: (e: any) => setError(e?.response?.data?.error || e?.response?.data?.message || 'Failed to transmit to eTIMS.'),
  });

  return (
    <Modal open={true} onClose={onClose} title="Transmit to KRA eTIMS" subtitle="Record this tax invoice as transmitted" size="sm">
      <form onSubmit={(e) => { e.preventDefault(); setError(null); mutation.mutate(); }} className="space-y-4">
        {error && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 text-red-700 text-sm">
            <ShieldCheck className="w-4 h-4 shrink-0" /><span>{error}</span>
          </div>
        )}
        <div className="flex items-start gap-2 p-3 rounded-lg bg-amber-50 text-amber-700 text-sm">
          <ShieldCheck className="w-4 h-4 shrink-0 mt-0.5" />
          <span>Once transmitted, this invoice is on record at KRA and can only be corrected with a credit note — it can no longer be edited or voided.</span>
        </div>
        <div>
          <label className="label">eTIMS Invoice Number <span className="text-gray-400 font-normal">(optional)</span></label>
          <input className="input" value={etimsNumber} onChange={(e) => setEtimsNumber(e.target.value)} placeholder="KRA control / CU invoice number" />
        </div>
        <div className="flex justify-end gap-3 pt-2">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Transmitting...' : 'Mark Transmitted'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function MpesaPaymentModal({
  invoiceId,
  balanceDue,
  currency,
  onClose,
  onSuccess,
  onError,
}: {
  invoiceId: string;
  balanceDue: number;
  currency: string;
  onClose: () => void;
  onSuccess: () => void;
  onError: (msg: string) => void;
}) {
  const [phone, setPhone] = useState('+254');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    const cleaned = phone.replace(/\s/g, '');
    if (!/^\+254\d{9}$/.test(cleaned)) {
      onError('Please enter a valid Kenyan phone number (e.g. +254712345678)');
      return;
    }

    setIsSubmitting(true);
    try {
      await mpesaStkPush({ invoice_id: invoiceId, phone: cleaned });
      onSuccess();
    } catch (err: any) {
      const message = err?.response?.data?.message || err?.message || 'Failed to initiate M-Pesa payment. Please try again.';
      onError(message);
      onClose();
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Modal open={true} onClose={onClose} title="Pay with M-Pesa" subtitle="Enter the phone number to receive the STK push" size="sm">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="bg-green-50 border border-green-200 rounded-lg p-3">
          <p className="text-sm text-green-800">
            Amount to pay: <span className="font-semibold">{formatCurrency(balanceDue, currency)}</span>
          </p>
        </div>

        <div>
          <label htmlFor="mpesa-phone" className="label">Phone Number *</label>
          <div className="relative">
            <Phone className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
            <input
              id="mpesa-phone"
              type="tel"
              className="input pl-10"
              value={phone}
              onChange={(e) => setPhone(e.target.value)}
              placeholder="+254712345678"
              required
              disabled={isSubmitting}
            />
          </div>
          <p className="text-xs text-gray-500 mt-1">Safaricom number that will receive the payment prompt</p>
        </div>

        <div className="flex justify-end gap-3 pt-2">
          <button type="button" onClick={onClose} className="btn-secondary" disabled={isSubmitting}>
            Cancel
          </button>
          <button
            type="submit"
            disabled={isSubmitting || phone.length < 13}
            className="inline-flex items-center gap-2 px-4 py-2 bg-green-600 text-white text-sm font-medium rounded-lg hover:bg-green-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSubmitting ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                Sending STK Push...
              </>
            ) : (
              <>
                <Phone className="w-4 h-4" />
                Send Payment Request
              </>
            )}
          </button>
        </div>
      </form>
    </Modal>
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
