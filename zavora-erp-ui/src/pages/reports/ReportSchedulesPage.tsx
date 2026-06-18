import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getReportSchedules, saveReportSchedule, deleteReportSchedule } from '../../api/client';
import { reportTypes } from './lib/reportTypes';
import PageHeader from '../../components/shared/PageHeader';
import { ArrowLeft, Plus, Trash2 } from 'lucide-react';

const CADENCES = ['daily', 'weekly', 'monthly'];

export default function ReportSchedulesPage() {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ['report-schedules'], queryFn: getReportSchedules });
  const schedules: any[] = data?.data ?? [];

  const [name, setName] = useState('');
  const [reportType, setReportType] = useState('TrialBalance');
  const [cadence, setCadence] = useState('monthly');
  const [recipients, setRecipients] = useState('');

  const invalidate = () => qc.invalidateQueries({ queryKey: ['report-schedules'] });
  const save = useMutation({
    mutationFn: () => saveReportSchedule({ name, report_type: reportType, cadence, recipients }),
    onSuccess: () => { setName(''); setRecipients(''); invalidate(); },
  });
  const remove = useMutation({ mutationFn: (id: string) => deleteReportSchedule(id), onSuccess: invalidate });
  const toggle = useMutation({
    mutationFn: (s: any) => saveReportSchedule({ id: s.id, name: s.name, report_type: s.report_type, cadence: s.cadence, recipients: s.recipients, is_active: !s.is_active }),
    onSuccess: invalidate,
  });

  const reportName = (key: string) => reportTypes.find((r) => r.key === key)?.name ?? key;

  return (
    <div>
      <PageHeader title="Scheduled Reports" subtitle="Email a report on a recurring schedule"
        actions={<Link to="/reports" className="btn-secondary"><ArrowLeft className="w-4 h-4" /> All reports</Link>} />

      <div className="card p-4 mb-5 flex flex-wrap items-end gap-3">
        <div><label className="label">Name</label><input className="input w-48" value={name} onChange={(e) => setName(e.target.value)} placeholder="Monthly P&L" /></div>
        <div>
          <label className="label">Report</label>
          <select className="input min-w-[12rem]" value={reportType} onChange={(e) => setReportType(e.target.value)}>
            {reportTypes.map((r) => <option key={r.key} value={r.key}>{r.name}</option>)}
          </select>
        </div>
        <div>
          <label className="label">Cadence</label>
          <select className="input" value={cadence} onChange={(e) => setCadence(e.target.value)}>
            {CADENCES.map((c) => <option key={c} value={c} className="capitalize">{c}</option>)}
          </select>
        </div>
        <div className="flex-1 min-w-[14rem]"><label className="label">Recipients (comma-separated)</label><input className="input w-full" value={recipients} onChange={(e) => setRecipients(e.target.value)} placeholder="finance@acme.co, ceo@acme.co" /></div>
        <button className="btn-primary" disabled={!name || !recipients || save.isPending} onClick={() => save.mutate()}>
          <Plus className="w-4 h-4" /> {save.isPending ? 'Saving…' : 'Add schedule'}
        </button>
      </div>

      <div className="card p-5">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-xs text-gray-500 uppercase border-b">
              <th className="text-left py-2">Name</th><th className="text-left">Report</th><th className="text-left">Cadence</th>
              <th className="text-left">Recipients</th><th className="text-left">Next run</th><th className="text-left">Active</th><th></th>
            </tr>
          </thead>
          <tbody>
            {schedules.map((s) => (
              <tr key={s.id} className="border-b border-gray-50">
                <td className="py-2">{s.name}</td>
                <td>{reportName(s.report_type)}</td>
                <td className="capitalize">{s.cadence}</td>
                <td className="text-gray-500">{s.recipients}</td>
                <td className="text-gray-500">{s.next_run_at ? new Date(s.next_run_at).toLocaleDateString() : 'next tick'}</td>
                <td><button onClick={() => toggle.mutate(s)} className={`text-xs px-2 py-0.5 rounded ${s.is_active ? 'bg-green-50 text-green-700' : 'bg-gray-100 text-gray-500'}`}>{s.is_active ? 'Active' : 'Paused'}</button></td>
                <td className="text-right"><button className="text-red-500 hover:text-red-700" onClick={() => remove.mutate(s.id)}><Trash2 className="w-4 h-4" /></button></td>
              </tr>
            ))}
            {schedules.length === 0 && <tr><td colSpan={7} className="py-4 text-center text-gray-400">No schedules yet.</td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );
}
