import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  getNotificationDelivery,
  getNotificationDeliveryStats,
  type DeliveryFilters,
} from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { Filter, Mail, MessageSquare, Smartphone, Bell, AlertTriangle, CheckCircle2, Clock } from 'lucide-react';

interface DeliveryRow {
  id: string;
  event_type: string;
  channel: string;
  recipient: string;
  subject: string | null;
  status: string;
  related_type: string | null;
  related_id: string | null;
  scheduled_at: string | null;
  sent_at: string | null;
  delivered_at: string | null;
  error: string | null;
  created_at: string;
}

interface DeliveryStats {
  total: number;
  failed: number;
  by_status: { status: string; count: number }[];
  by_channel: { channel: string; count: number }[];
}

const CHANNEL_ICON: Record<string, typeof Mail> = {
  email: Mail,
  sms: Smartphone,
  whatsapp: MessageSquare,
  in_app: Bell,
};

const STATUS_STYLE: Record<string, string> = {
  delivered: 'bg-green-100 text-green-700',
  sent: 'bg-blue-100 text-blue-700',
  read: 'bg-emerald-100 text-emerald-700',
  queued: 'bg-gray-100 text-gray-600',
  failed: 'bg-red-100 text-red-700',
};

function dateTime(ts: string | null): string {
  if (!ts) return '—';
  const d = new Date(ts);
  return isNaN(d.getTime())
    ? ts
    : d.toLocaleString(undefined, {
        month: 'short', day: '2-digit', hour: '2-digit', minute: '2-digit',
      });
}

function StatusBadge({ status }: { status: string }) {
  const style = STATUS_STYLE[status] || 'bg-gray-100 text-gray-600';
  return (
    <span className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs font-medium ${style}`}>
      {status === 'failed' && <AlertTriangle className="w-3 h-3" />}
      {(status === 'delivered' || status === 'read') && <CheckCircle2 className="w-3 h-3" />}
      {status === 'queued' && <Clock className="w-3 h-3" />}
      {status}
    </span>
  );
}

export default function NotificationDeliveryPage() {
  const [filters, setFilters] = useState<DeliveryFilters>({});
  const set = (k: keyof DeliveryFilters, v: string) =>
    setFilters((f) => ({ ...f, [k]: v || undefined }));
  const hasFilters = Object.values(filters).some(Boolean);

  const { data: stats } = useQuery<DeliveryStats>({
    queryKey: ['notif-delivery-stats'],
    queryFn: () => getNotificationDeliveryStats().then((r) => r.data),
  });

  const { data, isLoading } = useQuery<{ data: DeliveryRow[]; total_count: number }>({
    queryKey: ['notif-delivery', filters],
    queryFn: () => getNotificationDelivery({ ...filters, limit: 100 }).then((r) => r.data),
  });

  const rows = data?.data ?? [];

  const channelCount = (c: string) =>
    stats?.by_channel.find((x) => x.channel === c)?.count ?? 0;

  return (
    <div>
      <PageHeader
        title="Notification Delivery"
        subtitle="Delivery history across all channels — email, SMS, WhatsApp, and in-app"
      />

      {/* Stats cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4">
        <div className="card p-4">
          <p className="text-xs text-gray-500 uppercase tracking-wide">Total Sent</p>
          <p className="text-2xl font-bold text-gray-900 mt-1">{stats?.total ?? 0}</p>
        </div>
        <div className="card p-4">
          <p className="text-xs text-gray-500 uppercase tracking-wide">Failed</p>
          <p className={`text-2xl font-bold mt-1 ${stats && stats.failed > 0 ? 'text-red-600' : 'text-gray-900'}`}>
            {stats?.failed ?? 0}
          </p>
        </div>
        <div className="card p-4">
          <p className="text-xs text-gray-500 uppercase tracking-wide">Email</p>
          <p className="text-2xl font-bold text-gray-900 mt-1">{channelCount('email')}</p>
        </div>
        <div className="card p-4">
          <p className="text-xs text-gray-500 uppercase tracking-wide">SMS / WhatsApp</p>
          <p className="text-2xl font-bold text-gray-900 mt-1">
            {channelCount('sms') + channelCount('whatsapp')}
          </p>
        </div>
      </div>

      {/* Filters */}
      <div className="card mb-4 p-4">
        <div className="flex flex-wrap items-center gap-3">
          <Filter className="w-4 h-4 text-gray-400" />
          <select className="input py-1.5 text-sm w-auto" value={filters.channel ?? ''} onChange={(e) => set('channel', e.target.value)}>
            <option value="">All Channels</option>
            <option value="email">Email</option>
            <option value="sms">SMS</option>
            <option value="whatsapp">WhatsApp</option>
            <option value="in_app">In-App</option>
          </select>
          <select className="input py-1.5 text-sm w-auto" value={filters.status ?? ''} onChange={(e) => set('status', e.target.value)}>
            <option value="">All Statuses</option>
            <option value="queued">Queued</option>
            <option value="sent">Sent</option>
            <option value="delivered">Delivered</option>
            <option value="read">Read</option>
            <option value="failed">Failed</option>
          </select>
          <input
            className="input py-1.5 text-sm w-56"
            placeholder="Search recipient…"
            value={filters.search ?? ''}
            onChange={(e) => set('search', e.target.value)}
          />
          {hasFilters && (
            <button onClick={() => setFilters({})} className="text-xs text-blue-600 hover:underline">
              Clear filters
            </button>
          )}
        </div>
      </div>

      {/* Table */}
      {isLoading ? (
        <div className="card p-12 text-center">
          <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
          <p className="mt-3 text-sm text-gray-500">Loading delivery history…</p>
        </div>
      ) : rows.length === 0 ? (
        <div className="card p-12 text-center text-sm text-gray-500">
          No notifications match these filters.
        </div>
      ) : (
        <div className="card overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-xs text-gray-500 uppercase tracking-wide border-b border-gray-100">
                <th className="px-4 py-2.5 font-medium">Channel</th>
                <th className="px-4 py-2.5 font-medium">Event</th>
                <th className="px-4 py-2.5 font-medium">Recipient</th>
                <th className="px-4 py-2.5 font-medium">Status</th>
                <th className="px-4 py-2.5 font-medium">Created</th>
                <th className="px-4 py-2.5 font-medium">Sent</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100">
              {rows.map((r) => {
                const Icon = CHANNEL_ICON[r.channel] || Bell;
                return (
                  <tr key={r.id} className="hover:bg-gray-50">
                    <td className="px-4 py-2.5">
                      <span className="inline-flex items-center gap-1.5 text-gray-700">
                        <Icon className="w-4 h-4 text-gray-400" />
                        <span className="capitalize">{r.channel.replace('_', '-')}</span>
                      </span>
                    </td>
                    <td className="px-4 py-2.5 text-gray-700">{r.event_type}</td>
                    <td className="px-4 py-2.5 text-gray-700">{r.recipient}</td>
                    <td className="px-4 py-2.5">
                      <StatusBadge status={r.status} />
                      {r.error && (
                        <p className="text-[11px] text-red-500 mt-0.5 max-w-xs truncate" title={r.error}>
                          {r.error}
                        </p>
                      )}
                    </td>
                    <td className="px-4 py-2.5 text-gray-500 whitespace-nowrap">{dateTime(r.created_at)}</td>
                    <td className="px-4 py-2.5 text-gray-500 whitespace-nowrap">{dateTime(r.sent_at)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
