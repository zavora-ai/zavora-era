import { useState } from 'react';
import { useToast } from '../../components/toast/ToastProvider';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPosSession, getPosSessions, getZReport, closePosSession } from '../../api/client';
import { formatCurrency, formatDate } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';
import Modal from '../../components/shared/Modal';
import { Lock } from 'lucide-react';

export default function PosSessionsPage() {
  const { data: open } = useQuery({ queryKey: ['pos-session'], queryFn: () => getPosSession().then((r) => r.data) });
  const { data: sessions = [] } = useQuery<any[]>({ queryKey: ['pos-sessions'], queryFn: () => getPosSessions().then((r) => (Array.isArray(r.data) ? r.data : [])) });
  const [closing, setClosing] = useState(false);

  return (
    <div>
      <PageHeader title="Till Sessions" subtitle="Open shift, Z-report and cash reconciliation. Every POS sale posts a real invoice + payment." />

      {open ? (
        <div className="bg-white rounded-xl border border-emerald-200 p-4 mb-6">
          <div className="flex items-center justify-between">
            <div>
              <span className="inline-flex px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-100 text-emerald-700">OPEN</span>
              <p className="font-semibold text-gray-900 mt-1">{open.register_name}</p>
              <p className="text-sm text-gray-500">Opened {formatDate(open.opened_at)} · float {formatCurrency(open.opening_float, 'KES')}</p>
            </div>
            <button onClick={() => setClosing(true)} className="btn-primary"><Lock className="w-4 h-4" /> Close & reconcile</button>
          </div>
          <ZReport sessionId={open.id} />
        </div>
      ) : (
        <div className="bg-white rounded-xl border border-gray-200 p-6 mb-6 text-center text-gray-500">No till open. Go to <b>Sell</b> to open one.</div>
      )}

      <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
        <div className="px-4 py-3 border-b"><h3 className="font-semibold">Recent shifts</h3></div>
        <table className="w-full text-sm">
          <thead><tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
            <th className="text-left px-4 py-2">Register</th><th className="text-left px-4 py-2">Opened</th><th className="text-left px-4 py-2">Status</th>
            <th className="text-right px-4 py-2">Expected</th><th className="text-right px-4 py-2">Counted</th><th className="text-right px-4 py-2">Variance</th>
          </tr></thead>
          <tbody>
            {sessions.map((s) => (
              <tr key={s.id} className="border-b last:border-b-0">
                <td className="px-4 py-2 font-medium">{s.register_name}</td>
                <td className="px-4 py-2 text-gray-600">{formatDate(s.opened_at)}</td>
                <td className="px-4 py-2">{s.status}</td>
                <td className="px-4 py-2 text-right">{s.expected_cash != null ? formatCurrency(s.expected_cash, 'KES') : '—'}</td>
                <td className="px-4 py-2 text-right">{s.counted_cash != null ? formatCurrency(s.counted_cash, 'KES') : '—'}</td>
                <td className={`px-4 py-2 text-right font-medium ${Number(s.cash_variance) < 0 ? 'text-red-600' : Number(s.cash_variance) > 0 ? 'text-amber-600' : 'text-gray-500'}`}>{s.cash_variance != null ? formatCurrency(s.cash_variance, 'KES') : '—'}</td>
              </tr>
            ))}
            {sessions.length === 0 && <tr><td colSpan={6} className="px-4 py-8 text-center text-gray-400">No shifts yet.</td></tr>}
          </tbody>
        </table>
      </div>

      {closing && open && <CloseModal session={open} onClose={() => setClosing(false)} />}
    </div>
  );
}

function ZReport({ sessionId }: { sessionId: string }) {
  const { data } = useQuery({ queryKey: ['z-report', sessionId], queryFn: () => getZReport(sessionId).then((r) => r.data) });
  if (!data) return null;
  return (
    <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 mt-4">
      <Stat label="Sales" value={String(data.sales_count)} />
      <Stat label="Gross" value={formatCurrency(data.gross_total, 'KES')} />
      <Stat label="Cash sales" value={formatCurrency(data.cash_sales, 'KES')} />
      <Stat label="Expected in drawer" value={formatCurrency(data.expected_cash, 'KES')} highlight />
      {data.tenders?.map((t: any) => <Stat key={t.tender} label={`${t.tender} (${t.count})`} value={formatCurrency(t.amount, 'KES')} />)}
    </div>
  );
}
function Stat({ label, value, highlight }: { label: string; value: string; highlight?: boolean }) {
  return <div className={`rounded-lg border p-3 ${highlight ? 'border-indigo-200 bg-indigo-50' : 'border-gray-200 bg-gray-50'}`}><p className="text-xs text-gray-500 uppercase">{label}</p><p className="font-bold text-gray-900 mt-0.5">{value}</p></div>;
}

function CloseModal({ session, onClose }: { session: any; onClose: () => void }) {
  const qc = useQueryClient();
  const toast = useToast();
  const { data: z } = useQuery({ queryKey: ['z-report', session.id], queryFn: () => getZReport(session.id).then((r) => r.data) });
  const [counted, setCounted] = useState(0);
  const [result, setResult] = useState<any>(null);
  const expected = Number(z?.expected_cash ?? 0);
  const mut = useMutation({
    mutationFn: () => closePosSession(session.id, { counted_cash: Number(counted) }),
    onSuccess: (r) => { setResult(r.data); qc.invalidateQueries({ queryKey: ['pos-session'] }); qc.invalidateQueries({ queryKey: ['pos-sessions'] }); },
    onError: (e: any) => toast.fromError(e, 'Close failed.'),
  });

  if (result) {
    const v = Number(result.cash_variance);
    return (
      <Modal open={true} onClose={onClose} title="Shift closed" size="sm">
        <div className="text-center py-3">
          <p className="text-sm text-gray-500">Expected {formatCurrency(result.expected_cash, 'KES')} · Counted {formatCurrency(result.counted_cash, 'KES')}</p>
          <p className={`text-2xl font-bold my-2 ${v < 0 ? 'text-red-600' : v > 0 ? 'text-amber-600' : 'text-emerald-600'}`}>{v === 0 ? 'Balanced' : `${v > 0 ? 'Over' : 'Short'} ${formatCurrency(Math.abs(v), 'KES')}`}</p>
          <button onClick={onClose} className="btn-primary w-full justify-center mt-3">Done</button>
        </div>
      </Modal>
    );
  }
  return (
    <Modal open={true} onClose={onClose} title="Close till & count cash" size="sm">
      <div className="space-y-3">
        <p className="text-sm text-gray-500">Expected in drawer: <b>{formatCurrency(expected, 'KES')}</b> (float + cash sales). Count the physical cash and enter it.</p>
        <label className="label">Counted cash</label>
        <input type="number" min="0" className="input text-2xl text-center py-3" value={counted} onChange={(e) => setCounted(+e.target.value)} autoFocus />
        <p className="text-center text-sm">Variance: <b className={counted - expected < 0 ? 'text-red-600' : counted - expected > 0 ? 'text-amber-600' : ''}>{formatCurrency(counted - expected, 'KES')}</b></p>
        <button onClick={() => mut.mutate()} disabled={mut.isPending} className="btn-primary w-full justify-center py-3">{mut.isPending ? 'Closing…' : 'Close shift'}</button>
      </div>
    </Modal>
  );
}
