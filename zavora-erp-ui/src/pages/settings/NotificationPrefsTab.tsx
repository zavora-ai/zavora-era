import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getNotificationSettings, updateNotificationSettings, type EventPref, type ChannelStatus } from '../../api/client';
import { Save, CheckCircle, AlertCircle, Mail, Smartphone, MessageSquare, Bell, AlertTriangle } from 'lucide-react';

const CHANNELS: { key: string; label: string; icon: typeof Mail }[] = [
  { key: 'Email', label: 'Email', icon: Mail },
  { key: 'Sms', label: 'SMS', icon: Smartphone },
  { key: 'WhatsApp', label: 'WhatsApp', icon: MessageSquare },
  { key: 'InApp', label: 'In-App', icon: Bell },
];

// Friendly labels + descriptions for each configurable event.
const EVENT_META: Record<string, { label: string; desc: string }> = {
  InvoiceSent: { label: 'Invoice sent', desc: 'When an invoice is emailed to a customer.' },
  InvoicePaid: { label: 'Invoice paid', desc: 'When an invoice is fully paid.' },
  PaymentReceived: { label: 'Payment received', desc: 'When a customer payment is recorded.' },
  CreditLimitExceeded: { label: 'Credit limit exceeded', desc: 'When a new invoice would exceed a customer’s credit limit.' },
  BillApprovalNeeded: { label: 'Bill approval needed', desc: 'When a bill is awaiting approval.' },
  BillOverdue: { label: 'Bill overdue', desc: 'When a vendor bill becomes overdue.' },
  PayRunApprovalNeeded: { label: 'Pay run approval needed', desc: 'When a payroll run awaits approval.' },
  PeriodCloseWarning: { label: 'Period close warning', desc: 'When a fiscal period is soft-closed.' },
  BankFeedError: { label: 'Bank feed error', desc: 'When a bank import/feed fails.' },
  ReceiptProcessed: { label: 'Receipt processed', desc: 'When a captured receipt finishes OCR.' },
  ScheduledReport: { label: 'Scheduled report', desc: 'When a scheduled report is generated and emailed.' },
};

export default function NotificationPrefsTab() {
  const { data, isLoading, refetch } = useQuery<{ events: EventPref[]; channels: ChannelStatus[] }>({
    queryKey: ['notification-settings'],
    queryFn: () => getNotificationSettings().then((r) => r.data),
  });

  const [prefs, setPrefs] = useState<EventPref[]>([]);
  const [saving, setSaving] = useState(false);
  const [toast, setToast] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  // Which channels are actually configured on the server (env-based).
  const configured: Record<string, boolean> = {};
  (data?.channels ?? []).forEach((c) => { configured[c.channel] = c.configured; });
  const isConfigured = (key: string) => configured[key] !== false; // default true until loaded
  const unconfigured = (data?.channels ?? []).filter((c) => !c.configured).map((c) => c.channel);

  useEffect(() => {
    if (data?.events) setPrefs(data.events);
  }, [data]);

  const toggleEnabled = (event_type: string) =>
    setPrefs((p) => p.map((e) => (e.event_type === event_type ? { ...e, enabled: !e.enabled, is_default: false } : e)));

  const toggleChannel = (event_type: string, channel: string) =>
    setPrefs((p) =>
      p.map((e) => {
        if (e.event_type !== event_type) return e;
        const has = e.channels.includes(channel);
        return {
          ...e,
          channels: has ? e.channels.filter((c) => c !== channel) : [...e.channels, channel],
          is_default: false,
        };
      }),
    );

  const save = async () => {
    setSaving(true);
    setToast(null);
    try {
      await updateNotificationSettings(
        prefs.map(({ event_type, enabled, channels }) => ({ event_type, enabled, channels })),
      );
      setToast({ type: 'success', message: 'Notification preferences saved' });
      await refetch();
    } catch (err: any) {
      setToast({ type: 'error', message: err?.response?.data?.error || 'Failed to save preferences' });
    } finally {
      setSaving(false);
      setTimeout(() => setToast(null), 4000);
    }
  };

  if (isLoading) {
    return <div className="card p-6 text-sm text-gray-500">Loading notification preferences…</div>;
  }

  return (
    <div className="card p-6">
      <div className="mb-4">
        <h3 className="text-base font-semibold text-gray-900">Event notifications</h3>
        <p className="text-sm text-gray-500 mt-1">
          Choose which events notify your team and on which channels. Channels must also be configured
          on the server (SMTP / SMS / WhatsApp) to actually deliver. Invoice payment reminders are
          configured per customer, not here.
        </p>
      </div>

      {unconfigured.length > 0 && (
        <div className="mb-4 flex items-start gap-2 p-3 rounded-lg bg-amber-50 text-amber-800 text-sm">
          <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
          <span>
            Not configured on the server:{' '}
            <strong>{unconfigured.map((c) => CHANNELS.find((x) => x.key === c)?.label ?? c).join(', ')}</strong>.
            You can still tick these, but messages won’t deliver until the provider credentials are set
            (SMTP / Africa’s Talking / Twilio).
          </span>
        </div>
      )}

      {toast && (
        <div className={`mb-4 flex items-center gap-2 p-3 rounded-lg text-sm ${toast.type === 'success' ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'}`}>
          {toast.type === 'success' ? <CheckCircle className="w-4 h-4" /> : <AlertCircle className="w-4 h-4" />}
          <span>{toast.message}</span>
        </div>
      )}

      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-xs text-gray-500 uppercase tracking-wide border-b border-gray-100">
              <th className="py-2.5 pr-4 font-medium">Event</th>
              <th className="py-2.5 px-3 font-medium text-center">Enabled</th>
              {CHANNELS.map((c) => (
                <th key={c.key} className="py-2.5 px-3 font-medium text-center">
                  <span className={isConfigured(c.key) ? '' : 'text-gray-300'}>{c.label}</span>
                  {!isConfigured(c.key) && (
                    <span className="block text-[10px] font-normal text-amber-500 normal-case">not set up</span>
                  )}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100">
            {prefs.map((e) => {
              const meta = EVENT_META[e.event_type] ?? { label: e.event_type, desc: '' };
              return (
                <tr key={e.event_type} className={e.enabled ? '' : 'opacity-50'}>
                  <td className="py-3 pr-4">
                    <p className="font-medium text-gray-800">{meta.label}</p>
                    {meta.desc && <p className="text-[12px] text-gray-400">{meta.desc}</p>}
                  </td>
                  <td className="py-3 px-3 text-center">
                    <input
                      type="checkbox"
                      className="h-4 w-4 accent-indigo-600"
                      checked={e.enabled}
                      onChange={() => toggleEnabled(e.event_type)}
                      aria-label={`Enable ${meta.label}`}
                    />
                  </td>
                  {CHANNELS.map((c) => {
                    const ticked = e.channels.includes(c.key);
                    const warn = ticked && e.enabled && !isConfigured(c.key);
                    return (
                      <td key={c.key} className="py-3 px-3 text-center">
                        <input
                          type="checkbox"
                          className={`h-4 w-4 accent-indigo-600 disabled:opacity-40 ${warn ? 'ring-2 ring-amber-400 rounded' : ''}`}
                          checked={ticked}
                          disabled={!e.enabled}
                          onChange={() => toggleChannel(e.event_type, c.key)}
                          aria-label={`${meta.label} via ${c.label}`}
                          title={warn ? `${c.label} is selected but not configured on the server` : undefined}
                        />
                      </td>
                    );
                  })}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <div className="mt-6 pt-4 border-t flex justify-end">
        <button onClick={save} disabled={saving} className="btn-primary">
          {saving ? (
            <><div className="animate-spin w-4 h-4 border-2 border-white border-t-transparent rounded-full" /> Saving…</>
          ) : (
            <><Save className="w-4 h-4" /> Save Changes</>
          )}
        </button>
      </div>
    </div>
  );
}
