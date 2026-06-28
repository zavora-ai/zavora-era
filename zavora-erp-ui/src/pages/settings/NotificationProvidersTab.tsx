import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  getNotificationProviders,
  putNotificationProvider,
  testNotificationProvider,
  type MaskedProvider,
} from '../../api/client';
import { Save, CheckCircle, AlertCircle, AlertTriangle, Send, Mail, Smartphone, MessageSquare } from 'lucide-react';

// Field definitions per channel. `secret` marks the single write-only secret.
interface FieldDef { key: string; label: string; placeholder?: string; type?: string; secret?: boolean; }

const CHANNEL_DEFS: { channel: string; label: string; icon: typeof Mail; provider: string; fields: FieldDef[]; secretLabel: string }[] = [
  {
    channel: 'email', label: 'Email (SMTP)', icon: Mail, provider: 'Any SMTP server',
    secretLabel: 'SMTP password',
    fields: [
      { key: 'host', label: 'SMTP host', placeholder: 'smtp.example.com' },
      { key: 'port', label: 'Port', placeholder: '587', type: 'number' },
      { key: 'user', label: 'Username', placeholder: 'apikey or user@example.com' },
      { key: 'from', label: 'From address', placeholder: 'billing@yourcompany.co.ke' },
      { key: 'password', label: 'SMTP password', secret: true },
    ],
  },
  {
    channel: 'sms', label: 'SMS (Africa’s Talking)', icon: Smartphone, provider: 'Africa’s Talking',
    secretLabel: 'API key',
    fields: [
      { key: 'username', label: 'Username', placeholder: 'sandbox or your AT username' },
      { key: 'sender_id', label: 'Sender ID', placeholder: '(optional) e.g. ZAVORA' },
      { key: 'base_url', label: 'Base URL', placeholder: '(optional override)' },
      { key: 'api_key', label: 'API key', secret: true },
    ],
  },
  {
    channel: 'whatsapp', label: 'WhatsApp (Twilio)', icon: MessageSquare, provider: 'Twilio',
    secretLabel: 'Auth token',
    fields: [
      { key: 'account_sid', label: 'Account SID', placeholder: 'ACxxxxxxxx' },
      { key: 'from', label: 'From', placeholder: 'whatsapp:+14155238886' },
      { key: 'base_url', label: 'Base URL', placeholder: '(optional override)' },
      { key: 'auth_token', label: 'Auth token', secret: true },
    ],
  },
];

export default function NotificationProvidersTab() {
  const { data, isLoading, refetch } = useQuery<{ providers: MaskedProvider[]; encryption_available: boolean }>({
    queryKey: ['notification-providers'],
    queryFn: () => getNotificationProviders().then((r) => r.data),
  });

  // Per-channel local form state: { enabled, settings: {...}, secret: '' }
  const [form, setForm] = useState<Record<string, { enabled: boolean; settings: Record<string, any>; secret: string; hasSecret: boolean }>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [toast, setToast] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  useEffect(() => {
    if (!data) return;
    const next: typeof form = {};
    for (const def of CHANNEL_DEFS) {
      const existing = data.providers.find((p) => p.channel === def.channel);
      next[def.channel] = {
        enabled: existing?.enabled ?? false,
        settings: { ...(existing?.settings ?? {}) },
        secret: '',
        hasSecret: existing?.has_secret ?? false,
      };
    }
    setForm(next);
  }, [data]);

  const flash = (type: 'success' | 'error', message: string) => {
    setToast({ type, message });
    setTimeout(() => setToast(null), 4500);
  };

  const save = async (channel: string) => {
    const f = form[channel];
    setBusy(`save:${channel}`);
    try {
      await putNotificationProvider({
        channel,
        enabled: f.enabled,
        settings: f.settings,
        secret: f.secret.trim() ? f.secret.trim() : undefined,
      });
      flash('success', `${channel} provider saved`);
      await refetch();
    } catch (err: any) {
      flash('error', err?.response?.data?.error || 'Failed to save provider');
    } finally {
      setBusy(null);
    }
  };

  const test = async (channel: string) => {
    const recipient = window.prompt(
      channel === 'email' ? 'Send a test email to:' : 'Send a test message to (phone, e.g. 0712345678):',
    );
    if (!recipient) return;
    setBusy(`test:${channel}`);
    try {
      await testNotificationProvider(channel, recipient);
      flash('success', `Test ${channel} sent to ${recipient}`);
    } catch (err: any) {
      flash('error', err?.response?.data?.error || `Test ${channel} failed`);
    } finally {
      setBusy(null);
    }
  };

  if (isLoading) return <div className="card p-6 text-sm text-gray-500">Loading providers…</div>;

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-gray-900">Delivery providers</h3>
        <p className="text-sm text-gray-500 mt-1">
          Configure your own email/SMS/WhatsApp credentials. Secrets are encrypted at rest and never
          shown again — leave a secret field blank to keep the stored value. If a channel is left
          unconfigured, the deployment’s default provider is used.
        </p>
      </div>

      {data && !data.encryption_available && (
        <div className="flex items-start gap-2 p-3 rounded-lg bg-red-50 text-red-700 text-sm">
          <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
          <span>Secret storage is disabled: the server has no <code>NOTIF_ENC_KEY</code> set. You can edit
            non-secret fields, but secrets can’t be saved until an encryption key is configured.</span>
        </div>
      )}

      {toast && (
        <div className={`flex items-center gap-2 p-3 rounded-lg text-sm ${toast.type === 'success' ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700'}`}>
          {toast.type === 'success' ? <CheckCircle className="w-4 h-4" /> : <AlertCircle className="w-4 h-4" />}
          <span>{toast.message}</span>
        </div>
      )}

      {CHANNEL_DEFS.map((def) => {
        const f = form[def.channel];
        if (!f) return null;
        const Icon = def.icon;
        const update = (patch: Partial<typeof f>) => setForm((s) => ({ ...s, [def.channel]: { ...s[def.channel], ...patch } }));
        const setField = (key: string, value: any) => update({ settings: { ...f.settings, [key]: value } });
        return (
          <div key={def.channel} className="card p-5">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-2">
                <Icon className="w-5 h-5 text-indigo-600" />
                <div>
                  <p className="font-medium text-gray-900">{def.label}</p>
                  <p className="text-[12px] text-gray-400">{def.provider}</p>
                </div>
              </div>
              <label className="flex items-center gap-2 text-sm text-gray-600">
                <input type="checkbox" className="h-4 w-4 accent-indigo-600" checked={f.enabled} onChange={(e) => update({ enabled: e.target.checked })} />
                Enabled
              </label>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              {def.fields.map((field) => (
                <div key={field.key}>
                  <label className="label">{field.label}</label>
                  {field.secret ? (
                    <input
                      type="password"
                      className="input"
                      autoComplete="new-password"
                      placeholder={f.hasSecret ? '•••••••• (leave blank to keep)' : 'Enter secret'}
                      value={f.secret}
                      onChange={(e) => update({ secret: e.target.value })}
                    />
                  ) : (
                    <input
                      type={field.type ?? 'text'}
                      className="input"
                      placeholder={field.placeholder}
                      value={f.settings[field.key] ?? ''}
                      onChange={(e) => setField(field.key, field.type === 'number' ? (e.target.value === '' ? '' : Number(e.target.value)) : e.target.value)}
                    />
                  )}
                  {field.secret && f.hasSecret && (
                    <p className="text-[11px] text-green-600 mt-0.5">A secret is stored. Leave blank to keep it.</p>
                  )}
                </div>
              ))}
            </div>

            <div className="mt-4 flex items-center justify-end gap-2">
              <button
                onClick={() => test(def.channel)}
                disabled={busy !== null || !f.enabled}
                title={f.enabled ? 'Send a test message' : 'Enable and save first'}
                className="btn-secondary"
              >
                <Send className="w-4 h-4" /> {busy === `test:${def.channel}` ? 'Sending…' : 'Send test'}
              </button>
              <button onClick={() => save(def.channel)} disabled={busy !== null} className="btn-primary">
                <Save className="w-4 h-4" /> {busy === `save:${def.channel}` ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
