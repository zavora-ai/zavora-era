import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useParams, useNavigate } from 'react-router-dom';
import { getProject, getProjectSummary, type Project, type ProjectSummary } from '../../api/client';
import { usePermissions } from '../../hooks/usePermissions';
import ProjectFormModal from './ProjectFormModal';
import { ArrowLeft, Pencil } from 'lucide-react';

const money = (v: any) => Number(v ?? 0).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const BILLING_LABEL: Record<string, string> = {
  time_and_materials: 'Time & materials', fixed_fee: 'Fixed fee', milestone: 'Milestone', non_billable: 'Grant / non-billable',
};

export default function ProjectDetailPage() {
  const { id = '' } = useParams();
  const nav = useNavigate();
  const qc = useQueryClient();
  const { can } = usePermissions();
  const canWrite = can('project.manage');
  const [edit, setEdit] = useState(false);

  const { data: project } = useQuery<Project>({ queryKey: ['project', id], queryFn: () => getProject(id).then((r) => r.data), enabled: !!id });
  const { data: summary } = useQuery<ProjectSummary>({ queryKey: ['project-summary', id], queryFn: () => getProjectSummary(id).then((r) => r.data), enabled: !!id });

  if (!project) return <div className="card p-8 text-center text-gray-500">Loading…</div>;

  const budgetTotal = Number(summary?.budget_total ?? project.budget_amount);
  const cost = Number(summary?.cost ?? 0);
  const revenue = Number(summary?.revenue ?? 0);
  const margin = Number(summary?.margin ?? 0);
  const pct = Number(summary?.budget_used_pct ?? 0);

  return (
    <div>
      <button onClick={() => nav('/projects')} className="text-sm text-gray-500 hover:text-gray-800 mb-3 flex items-center gap-1"><ArrowLeft className="w-4 h-4" /> All projects</button>

      <div className="flex items-start justify-between mb-5">
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-bold text-gray-900">{project.name}</h1>
            <span className="text-xs font-mono text-gray-400">{project.code}</span>
          </div>
          <p className="text-sm text-gray-500 mt-1">
            {(project.client_name || project.donor) && <>{project.client_name ?? project.donor} · </>}
            {BILLING_LABEL[project.billing_method] ?? project.billing_method} · <span className="capitalize">{project.status.replace('_', ' ')}</span>
            {project.start_date && <> · {project.start_date}{project.end_date ? ` → ${project.end_date}` : ''}</>}
          </p>
        </div>
        {canWrite && <button className="btn-secondary" onClick={() => setEdit(true)}><Pencil className="w-4 h-4" /> Edit</button>}
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 mb-5">
        <div className="card p-4"><p className="text-xs text-gray-500">Budget</p><p className="text-xl font-semibold tabular-nums">{money(budgetTotal)}</p></div>
        <div className="card p-4"><p className="text-xs text-gray-500">Cost to date</p><p className="text-xl font-semibold tabular-nums">{money(cost)}</p>
          {budgetTotal > 0 && <p className={`text-xs mt-0.5 ${pct > 100 ? 'text-red-600' : 'text-gray-400'}`}>{pct}% of budget</p>}</div>
        <div className="card p-4"><p className="text-xs text-gray-500">Revenue</p><p className="text-xl font-semibold tabular-nums">{money(revenue)}</p></div>
        <div className="card p-4"><p className="text-xs text-gray-500">Margin</p><p className={`text-xl font-semibold tabular-nums ${margin < 0 ? 'text-red-600' : 'text-green-700'}`}>{money(margin)}</p></div>
      </div>

      {budgetTotal > 0 && (
        <div className="mb-5">
          <div className="h-2 w-full bg-gray-100 rounded-full overflow-hidden">
            <div className={`h-full ${pct > 100 ? 'bg-red-500' : pct > 85 ? 'bg-amber-500' : 'bg-indigo-500'}`} style={{ width: `${Math.min(pct, 100)}%` }} />
          </div>
          <p className="text-xs text-gray-400 mt-1">{money(cost)} spent of {money(budgetTotal)} budget</p>
        </div>
      )}

      <div className="grid lg:grid-cols-2 gap-4">
        {/* Budget vs actual */}
        <div className="card p-4">
          <h3 className="font-medium mb-3">Budget vs actual</h3>
          {(summary?.budget_vs_actual?.length ?? 0) === 0 ? (
            <p className="text-sm text-gray-400 py-3 text-center">No budget lines. Edit the project to add budget by cost category (map each to a GL account for automatic actuals).</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-1.5">Category</th><th className="text-right">Budget</th><th className="text-right">Actual</th><th className="text-right">Variance</th></tr></thead>
                <tbody>
                  {summary!.budget_vs_actual.map((l, i) => {
                    const v = Number(l.variance);
                    return (
                      <tr key={i} className="border-b border-gray-50">
                        <td className="py-1.5">{l.category}{l.account_code ? <span className="text-xs text-gray-400 font-mono ml-1">{l.account_code}</span> : ''}</td>
                        <td className="text-right tabular-nums">{money(l.budgeted)}</td>
                        <td className="text-right tabular-nums">{money(l.actual)}</td>
                        <td className={`text-right tabular-nums ${v < 0 ? 'text-red-600' : 'text-gray-500'}`}>{money(v)}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {/* Actuals by account (from the GL) */}
        <div className="card p-4">
          <h3 className="font-medium mb-3">Ledger actuals by account</h3>
          {(summary?.actuals_by_account?.length ?? 0) === 0 ? (
            <p className="text-sm text-gray-400 py-3 text-center">Nothing tagged to this project yet. On a bill or invoice line, pick this project under the <span className="font-medium">Project</span> dimension — its cost/revenue appears here.</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-1.5">Account</th><th className="text-left">Type</th><th className="text-right">Amount</th></tr></thead>
                <tbody>
                  {summary!.actuals_by_account.map((a, i) => (
                    <tr key={i} className="border-b border-gray-50">
                      <td className="py-1.5"><span className="font-mono text-xs text-gray-400">{a.account_code}</span> {a.account_name}</td>
                      <td className="text-gray-500">{a.account_type}</td>
                      <td className="text-right tabular-nums">{money(a.amount)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {/* Tasks */}
      {project.tasks.length > 0 && (
        <div className="card p-4 mt-4">
          <h3 className="font-medium mb-3">Tasks / phases</h3>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-1.5">Task</th><th className="text-right">Budget</th><th className="text-center">Status</th></tr></thead>
              <tbody>
                {project.tasks.map((t) => (
                  <tr key={t.id} className="border-b border-gray-50">
                    <td className="py-1.5">{t.name}</td>
                    <td className="text-right tabular-nums">{money(t.budget_amount)}</td>
                    <td className="text-center"><span className="text-[10px] font-medium px-2 py-0.5 rounded bg-gray-100 text-gray-600">{t.status}</span></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {edit && <ProjectFormModal project={project} onClose={() => setEdit(false)} onDone={() => { qc.invalidateQueries({ queryKey: ['project', id] }); qc.invalidateQueries({ queryKey: ['project-summary', id] }); qc.invalidateQueries({ queryKey: ['projects'] }); setEdit(false); }} />}
    </div>
  );
}
