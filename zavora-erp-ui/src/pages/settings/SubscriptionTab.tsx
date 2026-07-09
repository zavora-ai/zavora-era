import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getSubscription, billingCheckout, cancelSubscription } from '../../api/client';
import { PRICING_PLANS, planByKey } from '../../config/pricing';
import { CheckCircle, AlertCircle } from 'lucide-react';

// Manage the tenant's Zavora subscription: current plan + status, change plan
// (re-checkout via Paystack), and cancel. Renewal itself is automatic
// (server-side scheduler charges the saved authorization each month).
export default function SubscriptionTab() {
  const queryClient = useQueryClient();
  const [picking, setPicking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { data: sub, isLoading } = useQuery<any>({
    queryKey: ['subscription'],
    queryFn: () => getSubscription().then((r) => r.data),
  });

  const checkoutMut = useMutation({
    mutationFn: (plan: string) => billingCheckout(plan, `${window.location.origin}/settings`),
    onSuccess: (r: any) => {
      if (r.data?.authorization_url) {
        window.location.href = r.data.authorization_url; // paid plan → Paystack
      } else {
        queryClient.invalidateQueries({ queryKey: ['subscription'] }); // free plan
        setPicking(false);
      }
    },
    onError: (e: any) => setError(e?.response?.data?.error || 'Could not start checkout.'),
  });

  const cancelMut = useMutation({
    mutationFn: () => cancelSubscription(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['subscription'] }),
    onError: (e: any) => setError(e?.response?.data?.error || 'Could not cancel.'),
  });

  if (isLoading) return <p className="text-sm text-gray-400">Loading subscription…</p>;

  const planKey = sub?.plan ?? 'free';
  const plan = planByKey(planKey);
  const status: string = sub?.status ?? 'trialing';
  const periodEnd = sub?.current_period_end ? new Date(sub.current_period_end) : null;

  const statusBadge: Record<string, string> = {
    active: 'bg-green-50 text-green-700',
    trialing: 'bg-blue-50 text-blue-700',
    past_due: 'bg-amber-50 text-amber-700',
    cancelled: 'bg-gray-100 text-gray-500',
  };

  return (
    <div className="space-y-6 max-w-2xl">
      {error && (
        <div className="flex items-center gap-2 text-sm text-red-700 bg-red-50 border border-red-200 rounded-lg p-3">
          <AlertCircle className="w-4 h-4" /> {error}
        </div>
      )}

      {/* Current subscription */}
      <div className="rounded-lg border border-gray-200 p-5">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-gray-500">Current plan</p>
            <p className="text-xl font-bold text-gray-900">{plan?.name ?? planKey}</p>
            {plan && <p className="text-sm text-gray-500">{plan.price}{plan.per}</p>}
          </div>
          <span className={`text-xs font-medium px-2.5 py-1 rounded-full ${statusBadge[status] ?? 'bg-gray-100 text-gray-500'}`}>
            {status.replace('_', ' ')}
          </span>
        </div>
        {periodEnd && (
          <p className="text-sm text-gray-500 mt-3">
            {status === 'cancelled' ? 'Access ends' : 'Renews'} on {periodEnd.toLocaleDateString()}
          </p>
        )}
        {status === 'past_due' && (
          <p className="text-sm text-amber-700 mt-2">
            The last renewal charge failed. Update your card by choosing your plan again below.
          </p>
        )}
        <div className="flex gap-2 mt-4">
          <button className="btn-primary text-sm" onClick={() => { setPicking((v) => !v); setError(null); }}>
            {picking ? 'Close' : 'Change plan'}
          </button>
          {status !== 'cancelled' && planKey !== 'free' && (
            <button
              className="btn-secondary text-sm text-red-600"
              onClick={() => { if (confirm('Cancel your subscription? Access continues until the paid-through date.')) cancelMut.mutate(); }}
              disabled={cancelMut.isPending}
            >
              Cancel plan
            </button>
          )}
        </div>
      </div>

      {/* Plan picker */}
      {picking && (
        <div className="grid sm:grid-cols-2 gap-3">
          {PRICING_PLANS.map((p) => {
            const current = p.key === planKey;
            return (
              <div key={p.key} className={`rounded-lg border p-4 ${current ? 'border-indigo-500 bg-indigo-50/40' : 'border-gray-200'}`}>
                <div className="flex items-center justify-between">
                  <p className="font-semibold text-gray-900">{p.name}</p>
                  {current && <CheckCircle className="w-4 h-4 text-indigo-600" />}
                </div>
                <p className="text-sm text-gray-500">{p.price}{p.per}</p>
                <p className="text-xs text-gray-400 mt-1">{p.tag}</p>
                <button
                  className="btn-secondary text-xs w-full justify-center mt-3"
                  disabled={current || checkoutMut.isPending}
                  onClick={() => { setError(null); checkoutMut.mutate(p.key); }}
                >
                  {current ? 'Current plan' : p.key === 'free' ? 'Switch to Free' : 'Choose & pay'}
                </button>
              </div>
            );
          })}
        </div>
      )}

      <p className="text-xs text-gray-400">
        Paid plans renew automatically each month via your saved Paystack payment method (card, M-Pesa or bank).
        Changing to a paid plan takes you to secure checkout.
      </p>
    </div>
  );
}
