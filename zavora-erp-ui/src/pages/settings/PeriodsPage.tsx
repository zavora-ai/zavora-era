import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPeriods, generatePeriods, closePeriod, reopenPeriod } from '../../api/client';
import type { FiscalPeriod } from '../../types';
import { formatDate, statusColor } from '../../utils/format';
import { hasRole, ROLES_CLOSE_PERIOD } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import { CalendarClock, Lock, Unlock, Plus, AlertCircle } from 'lucide-react';

const STATUS_LABELS: Record<FiscalPeriod['status'], string> = {
  open: 'Open',
  soft_closed: 'Soft-closed',
  hard_closed: 'Hard-closed',
  future: 'Future',
};

const MONTHS = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

export default function PeriodsPage() {
  const [showGenerate, setShowGenerate] = useState(false);
  const [reopenTarget, setReopenTarget] = useState<FiscalPeriod | null>(null);
  const [error, setError] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const { data: periods = [], isLoading } = useQuery<FiscalPeriod[]>({
    queryKey: ['periods'],
    queryFn: () => getPeriods().then(r => r.data),
  });

  const closeMutation = useMutation({
    mutationFn: ({ id, close_type }: { id: string; close_type: 'Soft' | 'Hard' }) =>
      closePeriod(id, { close_type }),
    onSuccess: () => {
      setError(null);
      queryClient.invalidateQueries({ queryKey: ['periods'] });
    },
    onError: (e: any) => {
      setError(e?.response?.data?.error || e?.response?.data?.message || 'Failed to close period.');
    },
  });

  // Sort by fiscal_year then period_number
  const sorted = [...periods].sort(
    (a, b) => a.fiscal_year - b.fiscal_year || a.period_number - b.period_number
  );

  // Group by fiscal year for display
  const groups = sorted.reduce<Record<number, FiscalPeriod[]>>((acc, p) => {
    (acc[p.fiscal_year] ??= []).push(p);
    return acc;
  }, {});
  const years = Object.keys(groups).map(Number).sort((a, b) => a - b);

  return (
    <div>
      <PageHeader
        title="Fiscal Periods"
        subtitle="Close periods to lock the books — soft close warns, hard close prevents all postings"
        actions={
          hasRole(ROLES_CLOSE_PERIOD) ? (
            <button onClick={() => setShowGenerate(true)} className="btn-primary">
              <Plus className="w-4 h-4" /> Generate Periods
            </button>
          ) : undefined
        }
      />

      {error && (
        <div className="mb-4 flex items-center gap-2 p-3 rounded-lg bg-red-50 text-red-700 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {isLoading ? (
        <div className="card">
          <div className="p-12 text-center">
            <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
            <p className="mt-3 text-sm text-gray-500">Loading...</p>
          </div>
        </div>
      ) : periods.length === 0 ? (
        <div className="card">
          <div className="px-6 py-12 text-center text-sm text-gray-500">
            No fiscal periods yet. Generate a fiscal year's periods to get started.
          </div>
        </div>
      ) : (
        <div className="space-y-6">
          {years.map((year) => (
            <div key={year} className="card overflow-hidden">
              <div className="px-6 py-3 border-b border-gray-200 bg-gray-50 flex items-center gap-2">
                <CalendarClock className="w-4 h-4 text-gray-500" />
                <h2 className="text-sm font-semibold text-gray-900">FY {year}</h2>
              </div>
              <div className="overflow-x-auto">
                <table className="w-full">
                  <thead>
                    <tr className="border-b border-gray-200 bg-gray-50">
                      <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Status</th>
                      <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Period</th>
                      <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Date Range</th>
                      <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Closed</th>
                      <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">Actions</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-gray-200">
                    {groups[year].map((p) => (
                      <tr key={p.id}>
                        <td className="px-6 py-4 text-sm">
                          <span className={statusColor(p.status)}>{STATUS_LABELS[p.status]}</span>
                        </td>
                        <td className="px-6 py-4 text-sm font-medium text-gray-900">{p.name}</td>
                        <td className="px-6 py-4 text-sm text-gray-600">
                          {formatDate(p.start_date)} — {formatDate(p.end_date)}
                        </td>
                        <td className="px-6 py-4 text-sm text-gray-600">
                          {p.closed_at ? formatDate(p.closed_at) : '—'}
                        </td>
                        <td className="px-6 py-4 text-sm">
                          <div className="flex items-center justify-end gap-1">
                            {p.status === 'open' && hasRole(ROLES_CLOSE_PERIOD) && (
                              <button
                                onClick={() => closeMutation.mutate({ id: p.id, close_type: 'Soft' })}
                                className="btn-secondary text-xs py-1 px-2"
                                disabled={closeMutation.isPending}
                                title="Soft close — warns on postings"
                              >
                                <Lock className="w-3 h-3" /> Soft Close
                              </button>
                            )}
                            {p.status === 'soft_closed' && hasRole(ROLES_CLOSE_PERIOD) && (
                              <>
                                <button
                                  onClick={() => closeMutation.mutate({ id: p.id, close_type: 'Hard' })}
                                  className="btn-success text-xs py-1 px-2"
                                  disabled={closeMutation.isPending}
                                  title="Hard close — permanently locks the period"
                                >
                                  <Lock className="w-3 h-3" /> Hard Close
                                </button>
                                <button
                                  onClick={() => { setError(null); setReopenTarget(p); }}
                                  className="btn-secondary text-xs py-1 px-2"
                                  title="Reopen this period"
                                >
                                  <Unlock className="w-3 h-3" /> Reopen
                                </button>
                              </>
                            )}
                            {p.status === 'hard_closed' && (
                              <span className="inline-flex items-center gap-1 text-xs text-gray-400" title="Permanently locked">
                                <Lock className="w-3.5 h-3.5" /> Locked
                              </span>
                            )}
                            {p.status === 'future' && (
                              <span className="text-xs text-gray-300">—</span>
                            )}
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          ))}
        </div>
      )}

      {showGenerate && <GeneratePeriodsModal onClose={() => setShowGenerate(false)} onError={setError} />}
      {reopenTarget && (
        <ReopenPeriodModal
          period={reopenTarget}
          onClose={() => setReopenTarget(null)}
          onError={setError}
        />
      )}
    </div>
  );
}

function GeneratePeriodsModal({ onClose, onError }: { onClose: () => void; onError: (msg: string | null) => void }) {
  const queryClient = useQueryClient();
  const [fiscalYear, setFiscalYear] = useState(new Date().getFullYear());
  const [startMonth, setStartMonth] = useState(1);

  const mutation = useMutation({
    mutationFn: () => generatePeriods({ fiscal_year: fiscalYear, year_start_month: startMonth }),
    onSuccess: () => {
      onError(null);
      queryClient.invalidateQueries({ queryKey: ['periods'] });
      onClose();
    },
    onError: (e: any) => {
      onError(e?.response?.data?.error || e?.response?.data?.message || 'Failed to generate periods.');
      onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate();
  };

  return (
    <Modal open={true} onClose={onClose} title="Generate Periods" subtitle="Create the fiscal periods for a year" size="sm">
      <form onSubmit={handleSubmit} className="space-y-5">
        <div>
          <label className="label">Fiscal Year *</label>
          <input
            type="number"
            className="input"
            value={fiscalYear}
            onChange={(e) => setFiscalYear(+e.target.value)}
            min="2000"
            max="2100"
            required
          />
        </div>
        <div>
          <label className="label">Year Start Month *</label>
          <select className="input" value={startMonth} onChange={(e) => setStartMonth(+e.target.value)}>
            {MONTHS.map((m, i) => (
              <option key={m} value={i + 1}>{m}</option>
            ))}
          </select>
        </div>
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending}>
            {mutation.isPending ? 'Generating...' : 'Generate'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function ReopenPeriodModal({
  period,
  onClose,
  onError,
}: {
  period: FiscalPeriod;
  onClose: () => void;
  onError: (msg: string | null) => void;
}) {
  const queryClient = useQueryClient();
  const [reason, setReason] = useState('');

  const mutation = useMutation({
    mutationFn: () => reopenPeriod(period.id, { reason }),
    onSuccess: () => {
      onError(null);
      queryClient.invalidateQueries({ queryKey: ['periods'] });
      onClose();
    },
    onError: (e: any) => {
      onError(e?.response?.data?.error || e?.response?.data?.message || 'Failed to reopen period.');
      onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!reason.trim()) return;
    mutation.mutate();
  };

  return (
    <Modal open={true} onClose={onClose} title={`Reopen ${period.name}`} subtitle="Reopening is audited — a reason is required" size="sm">
      <form onSubmit={handleSubmit} className="space-y-5">
        <div>
          <label className="label">Reason *</label>
          <textarea
            className="input"
            rows={3}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            placeholder="Why is this period being reopened?"
            required
          />
        </div>
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !reason.trim()}>
            {mutation.isPending ? 'Reopening...' : 'Reopen Period'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
