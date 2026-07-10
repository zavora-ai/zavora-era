import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getConsolidationEntities, getSettings,
  getCompanyGroups, createCompanyGroup, getGroupMembers, addGroupMember, removeGroupMember,
  postIntercompanyCharge, getIntercompany, runGroupConsolidation,
} from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { useToast } from '../../components/toast/ToastProvider';
import { formatCurrency } from '../../utils/format';
import { CheckCircle2, AlertTriangle, Layers, Printer, Plus, Building2, ArrowRightLeft, Trash2, Crown } from 'lucide-react';

interface Entity { entity_id: string; name: string; currency: string }
interface Group { id: string; name: string; presentation_currency: string }
interface Member { entity_id: string; name: string; base_currency: string; is_parent: boolean; ownership_pct: string }

export default function ConsolidationPage() {
  const today = new Date().toISOString().split('T')[0];
  const qc = useQueryClient();
  const toast = useToast();

  const { data: entitiesRes } = useQuery({ queryKey: ['consolidation-entities'], queryFn: getConsolidationEntities });
  const entities: Entity[] = entitiesRes?.data ?? [];
  const { data: settingsRes } = useQuery({ queryKey: ['settings'], queryFn: getSettings });
  const branding = settingsRes?.data?.branding ?? {};

  const { data: groupsRes } = useQuery({ queryKey: ['company-groups'], queryFn: getCompanyGroups });
  const groups: Group[] = groupsRes?.data ?? [];
  const [groupId, setGroupId] = useState<string>('');
  const activeGroup = groups.find((g) => g.id === groupId);

  const { data: membersRes } = useQuery({
    queryKey: ['group-members', groupId], queryFn: () => getGroupMembers(groupId).then((r) => r.data), enabled: !!groupId,
  });
  const members: Member[] = membersRes ?? [];

  const { data: icRes } = useQuery({ queryKey: ['intercompany'], queryFn: () => getIntercompany().then((r) => r.data) });
  const icTxns: any[] = icRes ?? [];
  const entityName = (id: string) => entities.find((e) => e.entity_id === id)?.name ?? members.find((m) => m.entity_id === id)?.name ?? id.slice(0, 8);

  const createGrp = useMutation({
    mutationFn: (name: string) => createCompanyGroup({ name }),
    onSuccess: (r) => { qc.invalidateQueries({ queryKey: ['company-groups'] }); setGroupId(r.data.id); toast.success('Group created.'); },
    onError: (e: any) => toast.fromError(e, 'Could not create group.'),
  });
  const addMember = useMutation({
    mutationFn: (v: { entity_id: string; is_parent: boolean; ownership_pct: number }) => addGroupMember(groupId, v),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['group-members', groupId] }); toast.success('Company added to group.'); },
    onError: (e: any) => toast.fromError(e, 'Could not add company.'),
  });
  const removeMember = useMutation({
    mutationFn: (entityId: string) => removeGroupMember(groupId, entityId),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['group-members', groupId] }); toast.success('Company removed.'); },
    onError: (e: any) => toast.fromError(e, 'Could not remove company.'),
  });

  const [newGroupName, setNewGroupName] = useState('');
  const [addEntityId, setAddEntityId] = useState('');
  const [addParent, setAddParent] = useState(false);
  const [addOwnership, setAddOwnership] = useState('100');
  const nonMembers = entities.filter((e) => !members.some((m) => m.entity_id === e.entity_id));

  const [icFrom, setIcFrom] = useState('');
  const [icTo, setIcTo] = useState('');
  const [icAmount, setIcAmount] = useState('');
  const [icDesc, setIcDesc] = useState('');
  const postIc = useMutation({
    mutationFn: () => postIntercompanyCharge({ group_id: groupId || undefined, from_entity_id: icFrom, to_entity_id: icTo, amount: Number(icAmount), description: icDesc || undefined }),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['intercompany'] }); toast.success('Intercompany charge posted to both companies.'); setIcAmount(''); setIcDesc(''); },
    onError: (e: any) => toast.fromError(e, 'Could not post intercompany charge.'),
  });

  const [asAt, setAsAt] = useState(today);
  const consolidate = useMutation({
    mutationFn: () => runGroupConsolidation({ group_id: groupId, as_at: asAt }).then((r) => r.data),
    onError: (e: any) => toast.fromError(e, 'Could not consolidate.'),
  });
  const r = consolidate.data;

  return (
    <div>
      <div className="no-print">
        <PageHeader title="Multi-Company & Consolidation" subtitle="Group your companies, post intercompany charges, and consolidate with automatic eliminations" />

        <div className="card p-4 mb-5">
          <label className="label mb-1">Company group</label>
          <div className="flex flex-wrap items-end gap-3">
            <select className="input max-w-xs" value={groupId} onChange={(e) => setGroupId(e.target.value)}>
              <option value="">Select a group…</option>
              {groups.map((g) => <option key={g.id} value={g.id}>{g.name} ({g.presentation_currency})</option>)}
            </select>
            <div className="flex items-end gap-2">
              <div><label className="label">New group</label><input className="input" placeholder="e.g. Zavora Group" value={newGroupName} onChange={(e) => setNewGroupName(e.target.value)} /></div>
              <button className="btn-secondary" disabled={!newGroupName.trim() || createGrp.isPending} onClick={() => { createGrp.mutate(newGroupName.trim()); setNewGroupName(''); }}>
                <Plus className="w-4 h-4" /> Create
              </button>
            </div>
          </div>
        </div>

        {activeGroup && (
          <>
            <div className="card p-4 mb-5">
              <h3 className="font-semibold text-gray-900 mb-3 flex items-center gap-2"><Building2 className="w-4 h-4" /> Companies in {activeGroup.name}</h3>
              {members.length === 0 && <p className="text-sm text-gray-400 mb-3">No companies yet — add the ones you belong to.</p>}
              <div className="space-y-2 mb-4">
                {members.map((m) => (
                  <div key={m.entity_id} className="flex items-center gap-2 text-sm border border-gray-100 rounded-lg p-2">
                    {m.is_parent && <Crown className="w-4 h-4 text-amber-500" aria-label="Parent" />}
                    <span className="flex-1 font-medium">{m.name}</span>
                    <span className="text-xs text-gray-400">{m.base_currency}</span>
                    <span className="text-xs text-gray-500">{Number(m.ownership_pct)}% owned</span>
                    <button className="text-gray-300 hover:text-red-500" onClick={() => removeMember.mutate(m.entity_id)} title="Remove"><Trash2 className="w-4 h-4" /></button>
                  </div>
                ))}
              </div>
              {nonMembers.length > 0 && (
                <div className="flex flex-wrap items-end gap-2 border-t pt-3">
                  <div><label className="label">Add company</label>
                    <select className="input" value={addEntityId} onChange={(e) => setAddEntityId(e.target.value)}>
                      <option value="">Select…</option>
                      {nonMembers.map((e) => <option key={e.entity_id} value={e.entity_id}>{e.name}</option>)}
                    </select>
                  </div>
                  <div><label className="label">Ownership %</label><input className="input w-24" type="number" value={addOwnership} onChange={(e) => setAddOwnership(e.target.value)} /></div>
                  <label className="flex items-center gap-1.5 text-sm mb-2"><input type="checkbox" checked={addParent} onChange={(e) => setAddParent(e.target.checked)} /> Parent</label>
                  <button className="btn-secondary mb-0.5" disabled={!addEntityId || addMember.isPending} onClick={() => { addMember.mutate({ entity_id: addEntityId, is_parent: addParent, ownership_pct: Number(addOwnership) }); setAddEntityId(''); setAddParent(false); setAddOwnership('100'); }}>
                    <Plus className="w-4 h-4" /> Add
                  </button>
                </div>
              )}
            </div>

            <div className="card p-4 mb-5">
              <h3 className="font-semibold text-gray-900 mb-3 flex items-center gap-2"><ArrowRightLeft className="w-4 h-4" /> Post an intercompany charge</h3>
              <p className="text-xs text-gray-500 mb-3">Posts to both companies at once — the charging company books an intercompany receivable + income; the charged company books intercompany charges + a payable. These net to zero on consolidation.</p>
              <div className="flex flex-wrap items-end gap-2">
                <div><label className="label">From (charges)</label>
                  <select className="input" value={icFrom} onChange={(e) => setIcFrom(e.target.value)}>
                    <option value="">Company…</option>
                    {members.map((m) => <option key={m.entity_id} value={m.entity_id}>{m.name}</option>)}
                  </select>
                </div>
                <div><label className="label">To (charged)</label>
                  <select className="input" value={icTo} onChange={(e) => setIcTo(e.target.value)}>
                    <option value="">Company…</option>
                    {members.filter((m) => m.entity_id !== icFrom).map((m) => <option key={m.entity_id} value={m.entity_id}>{m.name}</option>)}
                  </select>
                </div>
                <div><label className="label">Amount</label><input className="input w-32" type="number" value={icAmount} onChange={(e) => setIcAmount(e.target.value)} /></div>
                <div className="flex-1 min-w-[10rem]"><label className="label">Description</label><input className="input" placeholder="e.g. Q3 management fee" value={icDesc} onChange={(e) => setIcDesc(e.target.value)} /></div>
                <button className="btn-primary mb-0.5" disabled={!icFrom || !icTo || !(Number(icAmount) > 0) || postIc.isPending} onClick={() => postIc.mutate()}>
                  <ArrowRightLeft className="w-4 h-4" /> {postIc.isPending ? 'Posting…' : 'Post charge'}
                </button>
              </div>
              {icTxns.length > 0 && (
                <div className="mt-4 border-t pt-3">
                  <p className="text-xs text-gray-500 uppercase mb-1">Recent intercompany transactions</p>
                  <div className="space-y-1 text-sm">
                    {icTxns.slice(0, 6).map((t) => (
                      <div key={t.id} className="flex items-center gap-2">
                        <span className="text-xs text-gray-400 tabular-nums">{t.tx_date}</span>
                        <span className="flex-1">{entityName(t.from_entity_id)} → {entityName(t.to_entity_id)}{t.description ? ` · ${t.description}` : ''}</span>
                        <span className="tabular-nums font-medium">{formatCurrency(Number(t.amount))}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            <div className="card p-4 mb-5 flex items-end gap-3">
              <div><label className="label">As at</label><input type="date" className="input" value={asAt} onChange={(e) => setAsAt(e.target.value)} /></div>
              <button className="btn-primary" disabled={members.length === 0 || consolidate.isPending} onClick={() => consolidate.mutate()}>
                <Layers className="w-4 h-4" /> {consolidate.isPending ? 'Consolidating…' : 'Consolidate group'}
              </button>
              {r && <button className="btn-secondary" onClick={() => window.print()}><Printer className="w-4 h-4" /> Print</button>}
            </div>
          </>
        )}
      </div>

      {r && (
        <div className="print-area mx-auto max-w-3xl bg-white border border-gray-200 rounded-lg shadow-sm">
          <div className="px-10 pt-10 pb-6 border-b">
            <div className="flex items-start justify-between">
              <h1 className="text-xl font-bold text-gray-900">{activeGroup?.name || branding.company_name || 'Group'}</h1>
              {r.balanced
                ? <span className="inline-flex items-center gap-1 text-xs font-medium text-green-700 bg-green-50 px-2 py-1 rounded"><CheckCircle2 className="w-3.5 h-3.5" /> Balanced</span>
                : <span className="inline-flex items-center gap-1 text-xs font-medium text-red-700 bg-red-50 px-2 py-1 rounded"><AlertTriangle className="w-3.5 h-3.5" /> Out of balance</span>}
            </div>
            <div className="text-center mt-4">
              <h2 className="text-lg font-semibold">Consolidated Trial Balance</h2>
              <p className="text-sm text-gray-500">As at {r.as_at} · {r.presentation_currency} · {r.members.length} compan{r.members.length === 1 ? 'y' : 'ies'}: {r.members.map((e: any) => e.name).join(', ')}</p>
              {r.members.some((m: any) => !m.translated && m.base_currency !== r.presentation_currency) && <p className="text-xs text-amber-700 mt-1">⚠ Some companies had no FX rate on file and were included at 1:1.</p>}
            </div>
          </div>
          <div className="px-10 py-8">
            <table className="w-full text-sm">
              <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Account</th><th className="text-right">Debit</th><th className="text-right">Credit</th></tr></thead>
              <tbody>
                {r.lines.map((l: any) => (
                  <tr key={l.account_code} className="border-b border-gray-50">
                    <td className="py-1.5"><span className="font-mono text-xs text-gray-400">{l.account_code}</span> {l.account_name}</td>
                    <td className="text-right tabular-nums">{Number(l.debit) ? formatCurrency(Number(l.debit)) : '—'}</td>
                    <td className="text-right tabular-nums">{Number(l.credit) ? formatCurrency(Number(l.credit)) : '—'}</td>
                  </tr>
                ))}
              </tbody>
              <tfoot><tr className="font-bold border-t-2"><td className="py-2">Total</td><td className="text-right tabular-nums">{formatCurrency(Number(r.total_debit))}</td><td className="text-right tabular-nums">{formatCurrency(Number(r.total_credit))}</td></tr></tfoot>
            </table>

            {r.eliminations.length > 0 && (
              <div className="mt-6">
                <h3 className="text-sm font-semibold text-gray-700 mb-2">Intercompany eliminations</h3>
                <table className="w-full text-sm">
                  <tbody>
                    {r.eliminations.map((e: any) => (
                      <tr key={e.account_code} className="border-b border-gray-50 text-gray-500">
                        <td className="py-1"><span className="font-mono text-xs text-gray-400">{e.account_code}</span> {e.account_name}</td>
                        <td className="text-right tabular-nums">({formatCurrency(Number(e.debit_removed) || Number(e.credit_removed))})</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <p className="text-xs text-gray-400 mt-1">Intercompany balances between group companies are removed so the group is not overstated.</p>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
