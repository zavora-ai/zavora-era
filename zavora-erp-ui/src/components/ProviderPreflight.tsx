import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { AlertTriangle } from 'lucide-react';
import { getNotificationProviders, type MaskedProvider } from '../api/client';

const CHANNEL_LABEL: Record<string, string> = {
  email: 'Email',
  sms: 'SMS',
  whatsapp: 'WhatsApp',
};

/**
 * Pre-flight notice for send flows. When the delivery provider for `channel`
 * isn't configured, the backend degrades to "mark as sent" WITHOUT actually
 * delivering — so a send silently looks successful. This warns the user and
 * links them straight to Settings → Providers.
 *
 * Renders nothing when the provider is ready (enabled + has a stored secret),
 * and nothing if the provider status can't be read (e.g. a non-admin gets 403 —
 * they can't configure it anyway, so a scary banner would only confuse).
 */
export default function ProviderPreflight({ channel }: { channel: 'email' | 'sms' | 'whatsapp' }) {
  const { data, isError } = useQuery<{ providers: MaskedProvider[] }>({
    queryKey: ['notification-providers'],
    queryFn: () => getNotificationProviders().then((r) => r.data),
    staleTime: 5 * 60 * 1000,
    retry: false,
  });

  if (isError || !data) return null; // can't determine (loading or forbidden) → stay quiet

  const provider = data.providers?.find((p) => p.channel === channel);
  const ready = !!(provider?.enabled && provider?.has_secret);
  if (ready) return null;

  const label = CHANNEL_LABEL[channel] ?? channel;
  return (
    <div className="flex items-start gap-2 bg-amber-50 border border-amber-200 text-amber-800 text-xs p-3 rounded-lg">
      <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
      <span className="flex-1">
        {label} delivery isn’t set up, so this will be recorded as sent but{' '}
        <strong>not actually delivered</strong>.{' '}
        <Link to="/settings?tab=providers" className="underline font-medium">
          Set up {label.toLowerCase()} delivery
        </Link>
        .
      </span>
    </div>
  );
}
