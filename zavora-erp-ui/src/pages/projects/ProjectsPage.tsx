import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { getProjects, type Project } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { usePermissions } from '../../hooks/usePermissions';
import ProjectFormModal from './ProjectFormModal';
import { Plus, FolderKanban } from 'lucide-react';

const money = (v: any) => Number(v ?? 0).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });

const STATUS_STYLE: Record<string, string> = {
  planning: 'bg-gray-100 text-gray-600', active: 'bg-green-50 text-green-700',
  on_hold: 'bg-amber-50 text-amber-700', completed: 'bg-blue-50 text-blue-700', closed: 'bg-gray-100 text-gray-500',
};
const BILLING_LABEL: Record<string, string> = {
  time_and_materials: 'T&M', fixed_fee: 'Fixed fee', milestone: 'Milestone', non_billable: 'Grant / non-billable',
};

export default function ProjectsPage() {
  const qc = useQueryClient();
  const nav = useNavigate();
  const { can } = usePermissions();
  const canWrite = can('project.manage');
  const [showNew, setShowNew] = useState(false);

  const { data: projects = [] } = useQuery<Project[]>({ queryKey: ['projects'], queryFn: () => getProjects().then((r) => r.data ?? []) });

  return (
    <div>
      <PageHeader title="Projects" subtitle="Job & project accounting — budget vs actual and profitability from the real ledger. Built for NGOs (grants) and construction (job costing)."
        actions={canWrite ? <button className="btn-primary" onClick={() => setShowNew(true)}><Plus className="w-4 h-4" /> New Project</button> : undefined} />

      {projects.length === 0 ? (
        <div className="card p-8 text-center text-sm text-gray-500">
          No projects yet. Create one, set its budget by cost category, then tag bills and invoices to it (via the Project dimension) — actuals roll up automatically.
        </div>
      ) : (
        <div className="card overflow-x-auto">
          <table className="w-full text-sm">
            <thead><tr className="text-xs text-gray-500 uppercase border-b">
              <th className="text-left py-2 px-3">Code</th><th className="text-left">Project</th><th className="text-left">Client / donor</th>
              <th className="text-left">Billing</th><th className="text-right">Budget</th><th className="text-center">Status</th>
            </tr></thead>
            <tbody>
              {projects.map((p) => (
                <tr key={p.id} className="border-b border-gray-50 hover:bg-gray-50 cursor-pointer" onClick={() => nav(`/projects/${p.id}`)}>
                  <td className="py-2 px-3 font-mono text-xs">{p.code}</td>
                  <td className="font-medium text-gray-900">{p.name}</td>
                  <td className="text-gray-500">{p.client_name ?? p.donor ?? '—'}</td>
                  <td className="text-gray-500">{BILLING_LABEL[p.billing_method] ?? p.billing_method}</td>
                  <td className="text-right tabular-nums">{money(Number(p.budget_amount) > 0 ? p.budget_amount : p.budget_lines.reduce((s, l) => s + Number(l.amount), 0))}</td>
                  <td className="text-center"><span className={`text-[10px] font-medium px-2 py-0.5 rounded ${STATUS_STYLE[p.status] ?? 'bg-gray-100'}`}>{p.status.replace('_', ' ')}</span></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {projects.length === 0 && (
        <div className="mt-6 flex items-center justify-center text-gray-300"><FolderKanban className="w-10 h-10" /></div>
      )}

      {showNew && <ProjectFormModal project={null} onClose={() => setShowNew(false)} onDone={() => { qc.invalidateQueries({ queryKey: ['projects'] }); setShowNew(false); }} />}
    </div>
  );
}
