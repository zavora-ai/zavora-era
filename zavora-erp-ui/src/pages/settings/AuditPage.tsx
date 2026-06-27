import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getAuditEvents } from '../../api/client';
import type { AuditEventEntry } from '../../types';

import PageHeader from '../../components/shared/PageHeader';
import { ChevronDown, ChevronRight, FileEdit, FilePlus, CheckCircle, RotateCcw, ShieldCheck, Filter } from 'lucide-react';

const EVENT_ICONS: Record<string, typeof FilePlus> = {
  Created: FilePlus,
  Updated: FileEdit,
  Posted: CheckCircle,
  Reversed: RotateCcw,
  Approved: ShieldCheck,
};

const EVENT_COLORS: Record<string, string> = {
  Created: 'bg-green-100 text-green-700',
  Updated: 'bg-blue-100 text-blue-700',
  Posted: 'bg-indigo-100 text-indigo-700',
  Reversed: 'bg-red-100 text-red-700',
  Approved: 'bg-emerald-100 text-emerald-700',
};

// Precise timestamp for an audit (date + time).
function dateTime(ts: string): string {
  const d = new Date(ts);
  return isNaN(d.getTime()) ? ts : d.toLocaleString(undefined, {
    year: 'numeric', month: 'short', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit',
  });
}

// Human label for the affected object (its document number / reference), not the UUID.
function objectLabel(e: AuditEventEntry): { primary: string; secondary?: string } {
  const s = e.after ?? e.before ?? {};
  const num = s.number || s.invoice_number || s.bill_number || s.credit_note_number || s.code;
  const ref = s.reference || s.name || s.description;
  if (num) return { primary: String(num), secondary: ref ? String(ref) : undefined };
  if (ref) return { primary: String(ref) };
  return { primary: `#${e.object_id.slice(0, 8)}` };
}

function actorOf(e: AuditEventEntry): { name: string; email?: string } {
  if (e.actor_name) return { name: e.actor_name, email: e.actor_email || undefined };
  if (typeof e.actor === 'string') return { name: e.actor };
  return { name: 'System' };
}

export default function AuditPage() {
  const [filterObjectType, setFilterObjectType] = useState('');
  const [filterEventType, setFilterEventType] = useState('');
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const { data: events = [], isLoading } = useQuery<AuditEventEntry[]>({
    queryKey: ['audit', filterObjectType, filterEventType],
    queryFn: () => getAuditEvents({
      object_type: filterObjectType || undefined,
      event_type: filterEventType || undefined,
    }).then(r => r.data.events ?? r.data),
  });

  const objectTypes = ['Invoice', 'Bill', 'Payment', 'JournalEntry', 'Account', 'Customer', 'Vendor', 'Employee', 'Asset', 'Inventory'];
  const eventTypes = ['Created', 'Updated', 'Posted', 'Reversed', 'Approved'];

  return (
    <div>
      <PageHeader
        title="Audit Trail"
        subtitle="Complete history of changes across the system"
      />

      {/* Filters */}
      <div className="card mb-4 p-4">
        <div className="flex items-center gap-4">
          <Filter className="w-4 h-4 text-gray-400" />
          <div>
            <select
              className="input py-1.5 text-sm"
              value={filterObjectType}
              onChange={(e) => setFilterObjectType(e.target.value)}
            >
              <option value="">All Object Types</option>
              {objectTypes.map(t => <option key={t} value={t}>{t}</option>)}
            </select>
          </div>
          <div>
            <select
              className="input py-1.5 text-sm"
              value={filterEventType}
              onChange={(e) => setFilterEventType(e.target.value)}
            >
              <option value="">All Events</option>
              {eventTypes.map(t => <option key={t} value={t}>{t}</option>)}
            </select>
          </div>
          {(filterObjectType || filterEventType) && (
            <button
              onClick={() => { setFilterObjectType(''); setFilterEventType(''); }}
              className="text-xs text-blue-600 hover:underline"
            >
              Clear filters
            </button>
          )}
        </div>
      </div>

      {/* Timeline */}
      {isLoading ? (
        <div className="card p-12 text-center">
          <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
          <p className="mt-3 text-sm text-gray-500">Loading audit events...</p>
        </div>
      ) : events.length === 0 ? (
        <div className="card p-12 text-center text-sm text-gray-500">
          No audit events found.
        </div>
      ) : (
        <div className="card overflow-hidden">
          <div className="divide-y divide-gray-100">
            {events.map((event) => {
              const Icon = EVENT_ICONS[event.event_type] || FileEdit;
              const color = EVENT_COLORS[event.event_type] || 'bg-gray-100 text-gray-700';
              const isExpanded = expandedId === event.id;
              const label = objectLabel(event);
              const actor = actorOf(event);
              const hasDetail = !!(event.before || event.after || event.metadata);

              return (
                <div key={event.id} className="hover:bg-gray-50 transition-colors">
                  <div
                    className="flex items-center gap-4 px-6 py-3.5 cursor-pointer"
                    onClick={() => setExpandedId(isExpanded ? null : event.id)}
                  >
                    <div className="text-gray-400">
                      {hasDetail ? (isExpanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />) : <div className="w-4 h-4" />}
                    </div>

                    <div className={`w-8 h-8 rounded-lg flex items-center justify-center shrink-0 ${color}`}>
                      <Icon className="w-4 h-4" />
                    </div>

                    {/* What */}
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium text-gray-900 flex items-center gap-2">
                        <span className={`inline-block px-1.5 py-0.5 rounded text-xs font-medium ${color}`}>{event.event_type}</span>
                        <span className="text-gray-500">{event.object_type.replace(/_/g, ' ')}</span>
                        <span className="font-semibold">{label.primary}</span>
                        {label.secondary && <span className="text-gray-400 truncate">· {label.secondary}</span>}
                      </p>
                      <p className="text-[11px] text-gray-400 font-mono mt-0.5">{event.object_type}/{event.object_id}</p>
                    </div>

                    {/* Who */}
                    <div className="text-right shrink-0">
                      <p className="text-sm text-gray-700">{actor.name}</p>
                      {actor.email && <p className="text-[11px] text-gray-400">{actor.email}</p>}
                    </div>

                    {/* When */}
                    <div className="text-xs text-gray-400 whitespace-nowrap w-44 text-right shrink-0">
                      {dateTime(event.timestamp)}
                    </div>
                  </div>

                  {isExpanded && hasDetail && (
                    <div className="px-6 pb-4 pl-[4.5rem] space-y-3">
                      <div className="grid grid-cols-2 gap-4">
                        {event.before && (
                          <div>
                            <p className="text-xs font-semibold text-gray-500 uppercase mb-1">Before</p>
                            <pre className="text-xs bg-red-50 border border-red-100 rounded-lg p-3 overflow-x-auto max-h-56">{JSON.stringify(event.before, null, 2)}</pre>
                          </div>
                        )}
                        {event.after && (
                          <div>
                            <p className="text-xs font-semibold text-gray-500 uppercase mb-1">After</p>
                            <pre className="text-xs bg-green-50 border border-green-100 rounded-lg p-3 overflow-x-auto max-h-56">{JSON.stringify(event.after, null, 2)}</pre>
                          </div>
                        )}
                      </div>
                      {event.metadata && (
                        <div>
                          <p className="text-xs font-semibold text-gray-500 uppercase mb-1">Metadata</p>
                          <pre className="text-xs bg-gray-50 border border-gray-100 rounded-lg p-3 overflow-x-auto max-h-40">{JSON.stringify(event.metadata, null, 2)}</pre>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
