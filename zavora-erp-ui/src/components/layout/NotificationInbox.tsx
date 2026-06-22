import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getNotifications, getUnreadCount, markNotificationRead, markAllNotificationsRead } from '../../api/client';
import { Bell, CheckCheck } from 'lucide-react';

interface Notification {
  id: string;
  event_type: string;
  subject?: string;
  body: string;
  related_type?: string;
  related_id?: string;
  read_at?: string | null;
  created_at: string;
}

// Map a notification's related resource to an in-app route.
function relatedPath(n: Notification): string | null {
  if (!n.related_id) return null;
  switch (n.related_type) {
    case 'invoice': return `/invoices/${n.related_id}`;
    case 'bill': return `/bills`;
    case 'estimate': return `/documents/estimate/${n.related_id}`;
    case 'customer': return `/customers/${n.related_id}`;
    case 'vendor': return `/vendors/${n.related_id}`;
    default: return null;
  }
}

function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}

export default function NotificationInbox() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // Poll the unread count every 30s; the list itself loads when the drawer opens.
  const { data: countResp } = useQuery({
    queryKey: ['notifications', 'unread-count'],
    queryFn: () => getUnreadCount().then(r => r.data),
    refetchInterval: 30000,
  });
  const unread: number = countResp?.count ?? 0;

  const { data: listResp } = useQuery({
    queryKey: ['notifications', 'list'],
    queryFn: () => getNotifications({ limit: 20 }).then(r => r.data),
    enabled: open,
  });
  const notifications: Notification[] = listResp?.data ?? [];

  const readMutation = useMutation({
    mutationFn: (id: string) => markNotificationRead(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notifications'] });
    },
  });
  const readAllMutation = useMutation({
    mutationFn: () => markAllNotificationsRead(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['notifications'] }),
  });

  // Close the drawer on outside click.
  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onClick);
    return () => document.removeEventListener('mousedown', onClick);
  }, [open]);

  const handleClick = (n: Notification) => {
    if (!n.read_at) readMutation.mutate(n.id);
    const path = relatedPath(n);
    if (path) {
      setOpen(false);
      navigate(path);
    }
  };

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen(o => !o)}
        className="relative p-2 text-gray-400 hover:text-gray-600 rounded-lg hover:bg-gray-50 transition-colors"
        title="Notifications"
      >
        <Bell className="w-[18px] h-[18px]" />
        {unread > 0 && (
          <span className="absolute -top-0.5 -right-0.5 min-w-[16px] h-4 px-1 flex items-center justify-center text-[10px] font-semibold text-white bg-red-500 rounded-full ring-2 ring-white">
            {unread > 99 ? '99+' : unread}
          </span>
        )}
      </button>

      {open && (
        <div className="absolute right-0 mt-2 w-80 bg-white rounded-xl shadow-lg border border-gray-100 z-50 overflow-hidden">
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-100">
            <h3 className="text-sm font-semibold text-gray-900">Notifications</h3>
            {unread > 0 && (
              <button
                onClick={() => readAllMutation.mutate()}
                className="text-xs text-blue-600 hover:text-blue-800 flex items-center gap-1"
                disabled={readAllMutation.isPending}
              >
                <CheckCheck className="w-3.5 h-3.5" /> Mark all read
              </button>
            )}
          </div>

          <div className="max-h-96 overflow-y-auto">
            {notifications.length === 0 ? (
              <div className="px-4 py-10 text-center text-sm text-gray-400">
                You're all caught up.
              </div>
            ) : (
              notifications.map((n) => (
                <button
                  key={n.id}
                  onClick={() => handleClick(n)}
                  className={`w-full text-left px-4 py-3 border-b border-gray-50 hover:bg-gray-50 transition-colors flex gap-3 ${n.read_at ? '' : 'bg-blue-50/40'}`}
                >
                  <span className={`mt-1.5 w-2 h-2 rounded-full shrink-0 ${n.read_at ? 'bg-transparent' : 'bg-blue-500'}`} />
                  <div className="min-w-0 flex-1">
                    {n.subject && <p className="text-sm font-medium text-gray-900 truncate">{n.subject}</p>}
                    <p className="text-xs text-gray-600 line-clamp-2">{n.body}</p>
                    <p className="text-[11px] text-gray-400 mt-0.5">{relativeTime(n.created_at)}</p>
                  </div>
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
