import { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApprovalLimits, setApprovalLimit } from '../../api/client';
import { formatCurrency } from '../../utils/format';
import PageHeader from '../../components/shared/PageHeader';

// Roles that can approve financial documents.
const ROLES = ['Owner', 'Admin', 'Approver', 'Accountant'];

export default function ApprovalLimitsPage() {
  const qc = useQueryClient();
  const { data: limits = [] } = useQuery<{ role: string; max_amount: string | null }[]>({
    queryKey: ['approval-limits'], queryFn: () => getApprovalLimits().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });
  const [draft, setDraft] = useState<Record<string, string>>({});

  useEffect(() => {
    const m: Record<string, string> = {};
    for (const l of limits) m[l.role] = l.max_amount == null ? '' : String(Number(l.max_amount));
    setDraft((d) => ({ ...m, ...d }));
  }, [limits]);

  const mut = useMutation({
    mutationFn: ({ role, amount }: { role: string; amount: number | null }) => setApprovalLimit(role, amount),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['approval-limits'] }),
  });

  const currentLimit = (role: string) => {
    const l = limits.find((x) => x.role === role);
    return l && l.max_amount != null ? formatCurrency(l.max_amount, 'KES') : 'Unlimited';
  };

  return (
    <div>
      <PageHeader title="Approval Limits" subtitle="Delegation of Authority — the maximum value each role may approve. Bills, requisitions and expense claims above a role's limit must go to higher authority. Blank = unlimited." />
      <div className="bg-white rounded-xl border border-gray-200 overflow-hidden max-w-2xl">
        <table className="w-full text-sm">
          <thead>
            <tr className="bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <th className="text-left px-4 py-2">Role</th>
              <th className="text-left px-4 py-2">Current limit</th>
              <th className="text-left px-4 py-2">Set limit (KES)</th>
              <th className="px-4 py-2"></th>
            </tr>
          </thead>
          <tbody>
            {ROLES.map((role) => (
              <tr key={role} className="border-b last:border-b-0">
                <td className="px-4 py-2 font-medium text-gray-900">{role}</td>
                <td className="px-4 py-2 text-gray-600">{currentLimit(role)}</td>
                <td className="px-4 py-2">
                  <input type="number" min="0" step="1000" placeholder="unlimited" className="input text-sm py-1.5 w-40"
                    value={draft[role] ?? ''} onChange={(e) => setDraft({ ...draft, [role]: e.target.value })} />
                </td>
                <td className="px-4 py-2 text-right">
                  <button className="btn-secondary text-xs py-1 px-3" disabled={mut.isPending}
                    onClick={() => mut.mutate({ role, amount: draft[role] === '' || draft[role] == null ? null : Number(draft[role]) })}>
                    Save
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
