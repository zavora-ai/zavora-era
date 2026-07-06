import { useState } from 'react';
import { useQuery, useMutation, useQueryClient, type QueryClient } from '@tanstack/react-query';
import {
  getCrmSettings, setCrmEnabled, getCrmPipelines, getCrmStages,
  getCrmLeads, createCrmLead, convertCrmLead,
  getCrmOpportunities, createCrmOpportunity, moveCrmOpportunity, winCrmOpportunity, loseCrmOpportunity,
  getCrmActivities, createCrmActivity, completeCrmActivity,
  getCrmAnalytics,
} from '../../api/client';
import { formatCurrency, formatDate } from '../../utils/format';
import { hasRole, ROLES_CREATE } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import StatCard from '../../components/shared/StatCard';
import DataTable, { type Column } from '../../components/shared/DataTable';
import Modal from '../../components/shared/Modal';
import {
  Plus, TrendingUp, Target, Trophy, Users, CheckCircle2, Trophy as Win, XCircle, ArrowRight, Power,
} from 'lucide-react';

interface Settings { enabled: boolean; default_pipeline_id?: string | null }
interface Pipeline { id: string; name: string; is_default: boolean }
interface Stage { id: string; name: string; sort_order: number; probability: string; is_won: boolean; is_lost: boolean }
interface Lead {
  id: string; name: string; company?: string; email?: string; phone?: string; source?: string;
  status: string; notes?: string; converted_opportunity_id?: string | null; created_at: string;
}
interface Opportunity {
  id: string; name: string; amount: string; currency: string; stage_id: string; status: string;
  probability: string; expected_close_date?: string | null; created_at: string;
}
interface Activity {
  id: string; kind: string; subject: string; notes?: string; due_date?: string | null; done: boolean; related_type?: string;
}

type Tab = 'overview' | 'pipeline' | 'leads' | 'activities';

/**
 * Refresh CRM queries so the view reflects a mutation immediately.
 * We CANCEL any in-flight fetches first, then invalidate: without the cancel,
 * React Query can "adopt" a request that started before the mutation committed
 * (e.g. create-then-convert in quick succession) and resolve the refetch with
 * stale pre-mutation data. Cancelling guarantees a fresh post-commit fetch.
 */
async function refreshCrm(qc: QueryClient, keys: string[][]) {
  await Promise.all(keys.map((k) => qc.cancelQueries({ queryKey: k })));
  await Promise.all(keys.map((k) => qc.invalidateQueries({ queryKey: k })));
}

export default function CrmPage() {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<Tab>('overview');
  const canWrite = hasRole(ROLES_CREATE);

  const { data: settings, isLoading: settingsLoading } = useQuery<Settings>({
    queryKey: ['crm-settings'],
    queryFn: () => getCrmSettings().then((r) => r.data),
  });

  const toggle = useMutation({
    mutationFn: (enabled: boolean) => setCrmEnabled(enabled),
    onSuccess: () => queryClient.invalidateQueries(),
  });

  if (settingsLoading) {
    return <div className="p-12 text-center text-sm text-gray-500">Loading CRM…</div>;
  }

  // Not enabled → opt-in call to action (the whole module is inert until then).
  if (!settings?.enabled) {
    return (
      <div>
        <PageHeader title="CRM" subtitle="Sales pipeline, leads, activities and a customer portal — an optional add-on." />
        <div className="card p-10 max-w-2xl mx-auto text-center">
          <div className="mx-auto w-14 h-14 rounded-2xl bg-indigo-50 text-indigo-600 flex items-center justify-center mb-4">
            <Target className="w-7 h-7" />
          </div>
          <h2 className="text-xl font-semibold text-gray-900">Turn on CRM for this workspace</h2>
          <p className="mt-2 text-sm text-gray-500">
            Manage leads, track opportunities through a sales pipeline, log activities, run support
            tickets and offer customers a self-service portal. This is fully optional and does not
            affect your accounting. You can turn it off at any time.
          </p>
          {canWrite ? (
            <button
              className="btn-primary mt-6 mx-auto"
              disabled={toggle.isPending}
              onClick={() => toggle.mutate(true)}
            >
              <Power className="w-4 h-4" /> {toggle.isPending ? 'Enabling…' : 'Enable CRM'}
            </button>
          ) : (
            <p className="mt-6 text-xs text-gray-400">Ask an administrator to enable the CRM module.</p>
          )}
        </div>
      </div>
    );
  }

  return (
    <div>
      <PageHeader
        title="CRM"
        subtitle="Sales pipeline, leads, activities and analytics."
        actions={canWrite ? (
          <button
            className="btn-secondary"
            disabled={toggle.isPending}
            onClick={() => { if (confirm('Disable CRM for this workspace? Data is kept but the module is hidden.')) toggle.mutate(false); }}
          >
            <Power className="w-4 h-4" /> Disable
          </button>
        ) : undefined}
      />

      <div className="flex gap-1 border-b border-gray-200 mb-6">
        {(['overview', 'pipeline', 'leads', 'activities'] as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`px-4 py-2 text-sm font-medium capitalize border-b-2 -mb-px transition-colors ${
              tab === t ? 'border-indigo-600 text-indigo-600' : 'border-transparent text-gray-500 hover:text-gray-700'
            }`}
          >
            {t}
          </button>
        ))}
      </div>

      {tab === 'overview' && <OverviewTab />}
      {tab === 'pipeline' && <PipelineTab canWrite={canWrite} />}
      {tab === 'leads' && <LeadsTab canWrite={canWrite} />}
      {tab === 'activities' && <ActivitiesTab canWrite={canWrite} />}
    </div>
  );
}

// ─── Overview / analytics ────────────────────────────────────────────────────

function OverviewTab() {
  const { data: a } = useQuery<any>({ queryKey: ['crm-analytics'], queryFn: () => getCrmAnalytics().then((r) => r.data) });
  if (!a) return <div className="text-sm text-gray-500">Loading analytics…</div>;
  const byStage: { stage: string; count: number; value: string }[] = a.pipeline_by_stage || [];
  const maxVal = Math.max(1, ...byStage.map((s) => Number(s.value) || 0));
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard title="Open pipeline" value={formatCurrency(a.open_value, 'KES')} subtitle={`${a.open_count} open deals`} icon={<Target className="w-5 h-5" />} />
        <StatCard title="Weighted forecast" value={formatCurrency(a.forecast, 'KES')} subtitle="Probability-adjusted" icon={<TrendingUp className="w-5 h-5" />} />
        <StatCard title="Win rate" value={`${a.win_rate}%`} subtitle={`${a.won} won · ${a.lost} lost`} icon={<Trophy className="w-5 h-5" />} />
        <StatCard title="Lead conversion" value={`${a.lead_conversion_rate}%`} subtitle={`${a.leads_converted}/${a.leads_total} converted`} icon={<Users className="w-5 h-5" />} />
      </div>
      <div className="card p-6">
        <h3 className="text-sm font-semibold text-gray-900 mb-4">Open pipeline by stage</h3>
        {byStage.length === 0 ? (
          <p className="text-sm text-gray-500">No open opportunities yet.</p>
        ) : (
          <div className="space-y-3">
            {byStage.map((s) => (
              <div key={s.stage} className="flex items-center gap-3">
                <div className="w-28 text-sm text-gray-600 shrink-0 truncate">{s.stage}</div>
                <div className="flex-1 bg-gray-100 rounded-full h-6 overflow-hidden">
                  <div className="h-full bg-indigo-500 rounded-full flex items-center justify-end px-2" style={{ width: `${Math.max(6, (Number(s.value) / maxVal) * 100)}%` }}>
                    <span className="text-xs font-medium text-white whitespace-nowrap">{formatCurrency(s.value, 'KES')}</span>
                  </div>
                </div>
                <div className="w-10 text-right text-xs text-gray-400">{s.count}</div>
              </div>
            ))}
          </div>
        )}
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <StatCard title="Avg. won deal" value={formatCurrency(a.avg_won_deal, 'KES')} />
        <StatCard title="Won value" value={formatCurrency(a.won_value, 'KES')} />
        <StatCard title="Open activities" value={String(a.open_activities)} icon={<CheckCircle2 className="w-5 h-5" />} />
      </div>
    </div>
  );
}

// ─── Pipeline kanban ─────────────────────────────────────────────────────────

function PipelineTab({ canWrite }: { canWrite: boolean }) {
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const { data: pipelines = [] } = useQuery<Pipeline[]>({ queryKey: ['crm-pipelines'], queryFn: () => getCrmPipelines().then((r) => r.data) });
  const pipeline = pipelines.find((p) => p.is_default) || pipelines[0];
  const { data: stages = [] } = useQuery<Stage[]>({
    queryKey: ['crm-stages', pipeline?.id], enabled: !!pipeline,
    queryFn: () => getCrmStages(pipeline!.id).then((r) => r.data),
  });
  const { data: opps = [], isLoading } = useQuery<Opportunity[]>({
    queryKey: ['crm-opportunities'], queryFn: () => getCrmOpportunities('Open').then((r) => r.data),
  });

  const invalidate = () => refreshCrm(queryClient, [['crm-opportunities'], ['crm-analytics']]);
  const move = useMutation({ mutationFn: ({ id, stage }: { id: string; stage: string }) => moveCrmOpportunity(id, stage), onSuccess: invalidate });
  const win = useMutation({ mutationFn: (id: string) => winCrmOpportunity(id), onSuccess: invalidate });
  const lose = useMutation({ mutationFn: (id: string) => loseCrmOpportunity(id, 'Lost from board'), onSuccess: invalidate });

  const openStages = stages.filter((s) => !s.is_won && !s.is_lost);

  return (
    <div>
      <div className="flex justify-end mb-4">
        {canWrite && <button className="btn-primary" onClick={() => setShowCreate(true)}><Plus className="w-4 h-4" /> New Opportunity</button>}
      </div>
      {isLoading ? (
        <div className="text-sm text-gray-500">Loading board…</div>
      ) : (
        <div className="flex gap-4 overflow-x-auto pb-4">
          {openStages.map((stage) => {
            const cards = opps.filter((o) => o.stage_id === stage.id);
            const total = cards.reduce((s, o) => s + (Number(o.amount) || 0), 0);
            return (
              <div key={stage.id} className="w-72 shrink-0">
                <div className="flex items-center justify-between px-1 mb-2">
                  <h4 className="text-sm font-semibold text-gray-700">{stage.name}</h4>
                  <span className="text-xs text-gray-400">{cards.length} · {formatCurrency(total, 'KES')}</span>
                </div>
                <div className="space-y-2 bg-gray-50 rounded-xl p-2 min-h-[120px]">
                  {cards.map((o) => (
                    <div key={o.id} className="bg-white rounded-lg border border-gray-200 p-3 shadow-sm">
                      <div className="font-medium text-sm text-gray-900">{o.name}</div>
                      <div className="text-sm text-gray-600 mt-0.5">{formatCurrency(o.amount, o.currency)}</div>
                      <div className="text-xs text-gray-400 mt-0.5">{o.probability}% · {o.expected_close_date ? formatDate(o.expected_close_date) : 'no close date'}</div>
                      {canWrite && (
                        <div className="mt-2 flex items-center gap-1.5">
                          <select
                            className="input text-xs py-1 flex-1"
                            value={o.stage_id}
                            onChange={(e) => move.mutate({ id: o.id, stage: e.target.value })}
                          >
                            {stages.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
                          </select>
                          <button title="Won" className="p-1 text-green-600 hover:bg-green-50 rounded" onClick={() => win.mutate(o.id)}><Win className="w-4 h-4" /></button>
                          <button title="Lost" className="p-1 text-red-600 hover:bg-red-50 rounded" onClick={() => lose.mutate(o.id)}><XCircle className="w-4 h-4" /></button>
                        </div>
                      )}
                    </div>
                  ))}
                  {cards.length === 0 && <p className="text-xs text-gray-400 text-center py-4">No deals</p>}
                </div>
              </div>
            );
          })}
        </div>
      )}
      {showCreate && <CreateOpportunityModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateOpportunityModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ name: '', amount: '', expected_close: '' });
  const [error, setError] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: () => createCrmOpportunity({ name: form.name, amount: Number(form.amount) || 0, expected_close: form.expected_close || undefined }),
    onSuccess: () => { refreshCrm(queryClient, [['crm-opportunities'], ['crm-analytics']]); onClose(); },
    onError: (e: any) => setError(e?.response?.data?.error || 'Could not create the opportunity.'),
  });
  return (
    <Modal open onClose={onClose} title="New Opportunity" subtitle="Opens in the pipeline's first stage.">
      <form onSubmit={(e) => { e.preventDefault(); if (!form.name.trim()) { setError('Enter a name.'); return; } mutation.mutate(); }} className="space-y-4">
        {error && <div className="text-sm text-red-600 bg-red-50 rounded-lg px-3 py-2">{error}</div>}
        <div><label className="label">Name</label><input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="Acme — annual subscription" /></div>
        <div><label className="label">Amount (KES)</label><input type="number" className="input" value={form.amount} onChange={(e) => setForm({ ...form, amount: e.target.value })} /></div>
        <div><label className="label">Expected close</label><input type="date" className="input" value={form.expected_close} onChange={(e) => setForm({ ...form, expected_close: e.target.value })} /></div>
        <div className="flex justify-end gap-2 pt-2"><button type="button" className="btn-secondary" onClick={onClose}>Cancel</button><button type="submit" className="btn-primary" disabled={mutation.isPending}>Create</button></div>
      </form>
    </Modal>
  );
}

// ─── Leads ───────────────────────────────────────────────────────────────────

function LeadsTab({ canWrite }: { canWrite: boolean }) {
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const { data: leads = [], isLoading } = useQuery<Lead[]>({ queryKey: ['crm-leads'], queryFn: () => getCrmLeads().then((r) => r.data) });
  const convert = useMutation({
    mutationFn: (id: string) => convertCrmLead(id),
    onSuccess: () => refreshCrm(queryClient, [['crm-leads'], ['crm-opportunities'], ['crm-analytics']]),
  });
  const columns: Column<Lead>[] = [
    { key: 'status', header: 'Status', render: (l) => <span className={`inline-block px-2 py-0.5 rounded-full text-xs font-medium ${l.status === 'Converted' ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-gray-600'}`}>{l.status}</span> },
    { key: 'name', header: 'Name', render: (l) => <span className="font-medium text-gray-900">{l.name}</span> },
    { key: 'company', header: 'Company', render: (l) => l.company || '—' },
    { key: 'email', header: 'Email', render: (l) => l.email || '—' },
    { key: 'source', header: 'Source', render: (l) => l.source || '—' },
    { key: 'created_at', header: 'Created', render: (l) => formatDate(l.created_at) },
    {
      key: 'actions', header: '', className: 'text-right', render: (l) => (
        canWrite && l.status !== 'Converted' ? (
          <button className="btn-secondary text-xs py-1" disabled={convert.isPending} onClick={() => convert.mutate(l.id)}>
            Convert <ArrowRight className="w-3 h-3" />
          </button>
        ) : l.converted_opportunity_id ? <span className="text-xs text-green-600">✓ Opportunity</span> : null
      ),
    },
  ];
  return (
    <div>
      <div className="flex justify-end mb-4">{canWrite && <button className="btn-primary" onClick={() => setShowCreate(true)}><Plus className="w-4 h-4" /> New Lead</button>}</div>
      <DataTable columns={columns} data={leads} loading={isLoading} emptyMessage="No leads yet. Add one or wait for portal sign-ups." />
      {showCreate && <CreateLeadModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateLeadModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ name: '', company: '', email: '', phone: '', source: '', notes: '' });
  const [error, setError] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: () => createCrmLead({ name: form.name, company: form.company || undefined, email: form.email || undefined, phone: form.phone || undefined, source: form.source || undefined, notes: form.notes || undefined }),
    onSuccess: () => { refreshCrm(queryClient, [['crm-leads']]); onClose(); },
    onError: (e: any) => setError(e?.response?.data?.error || 'Could not create the lead.'),
  });
  return (
    <Modal open onClose={onClose} title="New Lead" subtitle="Capture a prospect for the sales team.">
      <form onSubmit={(e) => { e.preventDefault(); if (!form.name.trim()) { setError('Enter a name.'); return; } mutation.mutate(); }} className="space-y-4">
        {error && <div className="text-sm text-red-600 bg-red-50 rounded-lg px-3 py-2">{error}</div>}
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Name</label><input className="input" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} /></div>
          <div><label className="label">Company</label><input className="input" value={form.company} onChange={(e) => setForm({ ...form, company: e.target.value })} /></div>
          <div><label className="label">Email</label><input type="email" className="input" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} /></div>
          <div><label className="label">Phone</label><input className="input" value={form.phone} onChange={(e) => setForm({ ...form, phone: e.target.value })} /></div>
        </div>
        <div><label className="label">Source</label><input className="input" value={form.source} onChange={(e) => setForm({ ...form, source: e.target.value })} placeholder="Referral, website, event…" /></div>
        <div><label className="label">Notes</label><textarea className="input" rows={2} value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} /></div>
        <div className="flex justify-end gap-2 pt-2"><button type="button" className="btn-secondary" onClick={onClose}>Cancel</button><button type="submit" className="btn-primary" disabled={mutation.isPending}>Create</button></div>
      </form>
    </Modal>
  );
}

// ─── Activities ──────────────────────────────────────────────────────────────

function ActivitiesTab({ canWrite }: { canWrite: boolean }) {
  const queryClient = useQueryClient();
  const [showCreate, setShowCreate] = useState(false);
  const { data: activities = [], isLoading } = useQuery<Activity[]>({ queryKey: ['crm-activities'], queryFn: () => getCrmActivities().then((r) => r.data) });
  const done = useMutation({ mutationFn: (id: string) => completeCrmActivity(id), onSuccess: () => refreshCrm(queryClient, [['crm-activities'], ['crm-analytics']]) });
  const columns: Column<Activity>[] = [
    { key: 'done', header: '', render: (act) => act.done ? <CheckCircle2 className="w-4 h-4 text-green-500" /> : <span className="w-4 h-4 inline-block rounded-full border border-gray-300" /> },
    { key: 'kind', header: 'Type', render: (act) => <span className="inline-block px-2 py-0.5 rounded-full text-xs bg-indigo-50 text-indigo-600">{act.kind}</span> },
    { key: 'subject', header: 'Subject', render: (act) => <span className={act.done ? 'text-gray-400 line-through' : 'text-gray-900'}>{act.subject}</span> },
    { key: 'due_date', header: 'Due', render: (act) => act.due_date ? formatDate(act.due_date) : '—' },
    { key: 'actions', header: '', className: 'text-right', render: (act) => (canWrite && !act.done ? <button className="btn-secondary text-xs py-1" disabled={done.isPending} onClick={() => done.mutate(act.id)}>Mark done</button> : null) },
  ];
  return (
    <div>
      <div className="flex justify-end mb-4">{canWrite && <button className="btn-primary" onClick={() => setShowCreate(true)}><Plus className="w-4 h-4" /> New Activity</button>}</div>
      <DataTable columns={columns} data={activities} loading={isLoading} emptyMessage="No activities logged." />
      {showCreate && <CreateActivityModal onClose={() => setShowCreate(false)} />}
    </div>
  );
}

function CreateActivityModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const [form, setForm] = useState({ kind: 'Task', subject: '', due_at: '', notes: '' });
  const [error, setError] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: () => createCrmActivity({ kind: form.kind, subject: form.subject, due_at: form.due_at ? new Date(form.due_at).toISOString() : undefined, notes: form.notes || undefined }),
    onSuccess: () => { refreshCrm(queryClient, [['crm-activities'], ['crm-analytics']]); onClose(); },
    onError: (e: any) => setError(e?.response?.data?.error || 'Could not create the activity.'),
  });
  return (
    <Modal open onClose={onClose} title="New Activity" subtitle="Log a task, call, meeting, email or note.">
      <form onSubmit={(e) => { e.preventDefault(); if (!form.subject.trim()) { setError('Enter a subject.'); return; } mutation.mutate(); }} className="space-y-4">
        {error && <div className="text-sm text-red-600 bg-red-50 rounded-lg px-3 py-2">{error}</div>}
        <div className="grid grid-cols-2 gap-3">
          <div><label className="label">Type</label>
            <select className="input" value={form.kind} onChange={(e) => setForm({ ...form, kind: e.target.value })}>
              {['Task', 'Call', 'Meeting', 'Email', 'Note'].map((k) => <option key={k} value={k}>{k}</option>)}
            </select>
          </div>
          <div><label className="label">Due</label><input type="datetime-local" className="input" value={form.due_at} onChange={(e) => setForm({ ...form, due_at: e.target.value })} /></div>
        </div>
        <div><label className="label">Subject</label><input className="input" value={form.subject} onChange={(e) => setForm({ ...form, subject: e.target.value })} /></div>
        <div><label className="label">Notes</label><textarea className="input" rows={2} value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} /></div>
        <div className="flex justify-end gap-2 pt-2"><button type="button" className="btn-secondary" onClick={onClose}>Cancel</button><button type="submit" className="btn-primary" disabled={mutation.isPending}>Create</button></div>
      </form>
    </Modal>
  );
}
