import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getAuditEvents } from '../../api/client';
import type { AuditEventEntry } from '../../types';
import { formatDate } from '../../utils/format';
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

              return (
                <div key={event.id} className="hover:bg-gray-50 transition-colors">
                  <div
                    className="flex items-center gap-4 px-6 py-4 cursor-pointer"
                    onClick={() => setExpandedId(isExpanded ? null : event.id)}
                  >
                    {/* Expand toggle */}
                    <div className="text-gray-400">
                      {(event.before_state || event.after_state) ? (
                        isExpanded ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />
                      ) : (
                        <div className="w-4 h-4" />
                      )}
                    </div>

                    {/* Event icon */}
                    <div className={`w-8 h-8 rounded-lg flex items-center justify-center ${color}`}>
                      <Icon className="w-4 h-4" />
                    </div>

                    {/* Event details */}
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium text-gray-900">
                        <span className={`inline-block px-1.5 py-0.5 rounded text-xs font-medium mr-2 ${color}`}>
                          {event.event_type}
                        </span>
                        {event.object_type}
                        <span className="text-gray-400 font-mono text-xs ml-1">#{event.object_id.slice(0, 8)}</span>
                      </p>
                    </div>

                    {/* Actor */}
                    <div className="text-sm text-gray-500">
                      {typeof event.actor === 'string' ? event.actor : event.actor?.name || 'System'}
                    </div>

                    {/* Timestamp */}
                    <div className="text-xs text-gray-400 whitespace-nowrap">
                      {formatDate(event.timestamp)}
                    </div>
                  </div>

                  {/* Expanded: before/after */}
                  {isExpanded && (event.before_state || event.after_state) && (
                    <div className="px-6 pb-4 pl-[4.5rem]">
                      <div className="grid grid-cols-2 gap-4">
                        {event.before_state && (
                          <div>
                            <p className="text-xs font-semibold text-gray-500 uppercase mb-1">Before</p>
                            <pre className="text-xs bg-red-50 border border-red-100 rounded-lg p-3 overflow-x-auto max-h-48">
                              {JSON.stringify(event.before_state, null, 2)}
                            </pre>
                          </div>
                        )}
                        {event.after_state && (
                          <div>
                            <p className="text-xs font-semibold text-gray-500 uppercase mb-1">After</p>
                            <pre className="text-xs bg-green-50 border border-green-100 rounded-lg p-3 overflow-x-auto max-h-48">
                              {JSON.stringify(event.after_state, null, 2)}
                            </pre>
                          </div>
                        )}
                      </div>
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
