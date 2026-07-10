import { useState } from 'react';
import { useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { CreditCard, CheckCircle2, FileText, AlertCircle, Loader2 } from 'lucide-react';
import { getPublicInvoice, payPublicInvoice, type PublicInvoiceView } from '../../api/client';

function money(currency: string, amount: string) {
  const n = Number(amount);
  if (Number.isNaN(n)) return `${currency} ${amount}`;
  return `${currency} ${n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

/**
 * Public, unauthenticated invoice page reached via a share link
 * (`/pay/:token`). Viewing it stamps `viewed_at` server-side; a payable invoice
 * offers card payment via Paystack (redirect to the hosted checkout).
 */
export default function PublicInvoicePage() {
  const { token = '' } = useParams<{ token: string }>();
  const [paying, setPaying] = useState(false);
  const [payError, setPayError] = useState<string | null>(null);

  const { data, isLoading, isError, error } = useQuery<PublicInvoiceView>({
    queryKey: ['public-invoice', token],
    queryFn: () => getPublicInvoice(token).then((r) => r.data),
    retry: false,
    enabled: !!token,
  });

  const notFound = isError && (error as any)?.response?.status === 404;

  const startPayment = async () => {
    setPaying(true);
    setPayError(null);
    try {
      const res = await payPublicInvoice(token, { callback_url: window.location.href });
      const url = res.data?.authorization_url;
      if (url) {
        window.location.href = url;
      } else {
        setPayError('Could not start the payment. Please try again later.');
        setPaying(false);
      }
    } catch (e: any) {
      const msg = e?.response?.data?.error || '';
      setPayError(
        /paystack is not configured/i.test(msg)
          ? 'Online card payment isn’t available for this business yet. Please contact them to pay.'
          : msg || 'Could not start the payment. Please try again later.',
      );
      setPaying(false);
    }
  };

  return (
    <div className="min-h-screen bg-gray-50 flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        {isLoading && (
          <div className="flex flex-col items-center gap-3 py-16 text-gray-500">
            <Loader2 className="w-6 h-6 animate-spin" />
            <p className="text-sm">Loading invoice…</p>
          </div>
        )}

        {notFound && (
          <div className="bg-white rounded-2xl shadow-sm border border-gray-100 p-8 text-center">
            <AlertCircle className="w-10 h-10 text-amber-500 mx-auto" />
            <h1 className="mt-3 text-lg font-semibold text-gray-900">Invoice not found</h1>
            <p className="mt-1 text-sm text-gray-500">
              This payment link is invalid or has expired. Please check with the business that sent it.
            </p>
          </div>
        )}

        {isError && !notFound && (
          <div className="bg-white rounded-2xl shadow-sm border border-gray-100 p-8 text-center">
            <AlertCircle className="w-10 h-10 text-red-500 mx-auto" />
            <h1 className="mt-3 text-lg font-semibold text-gray-900">Something went wrong</h1>
            <p className="mt-1 text-sm text-gray-500">We couldn’t load this invoice. Please try again shortly.</p>
          </div>
        )}

        {data && (
          <div className="bg-white rounded-2xl shadow-sm border border-gray-100 overflow-hidden">
            <div className="bg-gradient-to-br from-indigo-600 to-purple-600 px-8 py-6 text-white">
              <div className="flex items-center gap-2 text-indigo-100 text-xs font-medium uppercase tracking-wide">
                <FileText className="w-4 h-4" /> Invoice
              </div>
              <h1 className="mt-1 text-2xl font-bold">{data.company_name}</h1>
              <p className="text-indigo-100 text-sm mt-0.5">{data.number}</p>
            </div>

            <div className="px-8 py-6 space-y-4">
              <div className="flex items-baseline justify-between">
                <span className="text-sm text-gray-500">Amount due</span>
                <span className="text-2xl font-bold text-gray-900">{money(data.currency, data.balance_due)}</span>
              </div>
              <div className="grid grid-cols-2 gap-3 text-sm">
                <div>
                  <p className="text-gray-400">Total</p>
                  <p className="font-medium text-gray-800">{money(data.currency, data.gross_total)}</p>
                </div>
                <div>
                  <p className="text-gray-400">Due date</p>
                  <p className="font-medium text-gray-800">{data.due_date}</p>
                </div>
              </div>

              {!data.payable ? (
                <div className="flex items-center gap-2 bg-green-50 text-green-800 border border-green-200 rounded-lg p-3 text-sm">
                  <CheckCircle2 className="w-5 h-5 shrink-0" />
                  <span>{Number(data.balance_due) <= 0 ? 'This invoice has been paid in full. Thank you!' : 'This invoice is not open for payment.'}</span>
                </div>
              ) : (
                <>
                  {payError && (
                    <div className="flex items-start gap-2 bg-amber-50 text-amber-800 border border-amber-200 rounded-lg p-3 text-sm">
                      <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
                      <span>{payError}</span>
                    </div>
                  )}
                  <button
                    type="button"
                    onClick={startPayment}
                    disabled={paying}
                    className="w-full flex items-center justify-center gap-2 bg-indigo-600 hover:bg-indigo-700 disabled:opacity-60 text-white font-medium py-3 rounded-xl transition-colors"
                  >
                    {paying ? <Loader2 className="w-5 h-5 animate-spin" /> : <CreditCard className="w-5 h-5" />}
                    {paying ? 'Starting secure payment…' : `Pay ${money(data.currency, data.balance_due)}`}
                  </button>
                  <p className="text-center text-xs text-gray-400">Secured by Paystack · card payment</p>
                </>
              )}
            </div>
          </div>
        )}

        <p className="text-center text-xs text-gray-300 mt-6">Powered by Zavora ERA</p>
      </div>
    </div>
  );
}
