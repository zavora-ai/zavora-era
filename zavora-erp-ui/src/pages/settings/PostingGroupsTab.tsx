import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getPostingGroups, createPostingGroup, upsertGeneralMatrix, upsertVatMatrix, upsertBusinessControl, getAccounts } from '../../api/client';
import type { Account } from '../../types';
import { Plus, Info } from 'lucide-react';

interface Group { id: string; code: string; name: string }
interface BizGroup extends Group { receivables_account?: string; payables_account?: string }
interface GenCell { gen_biz_group_id: string; gen_prod_group_id: string; sales_account?: string; purchase_account?: string; cogs_account?: string }
interface VatCell { vat_biz_group_id: string; vat_prod_group_id: string; vat_rate: string | number; vat_output_account?: string; vat_input_account?: string }
interface PostingGroups {
  vat_business: Group[]; vat_product: Group[]; vat_matrix: VatCell[];
  general_business: BizGroup[]; general_product: Group[]; general_matrix: GenCell[];
}

export default function PostingGroupsTab() {
  const qc = useQueryClient();
  const { data, isLoading } = useQuery<PostingGroups>({ queryKey: ['posting-groups'], queryFn: () => getPostingGroups().then(r => r.data) });
  const { data: accounts = [] } = useQuery<Account[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then(r => Array.isArray(r.data) ? r.data : []) });

  const invalidate = () => qc.invalidateQueries({ queryKey: ['posting-groups'] });
  const genMut = useMutation({ mutationFn: upsertGeneralMatrix, onSuccess: invalidate });
  const vatMut = useMutation({ mutationFn: upsertVatMatrix, onSuccess: invalidate });
  const ctrlMut = useMutation({ mutationFn: upsertBusinessControl, onSuccess: invalidate });
  const groupMut = useMutation({ mutationFn: createPostingGroup, onSuccess: invalidate });

  if (isLoading || !data) return <div className="p-6 text-sm text-gray-400">Loading posting groups…</div>;

  const AccountSelect = ({ value, onChange }: { value?: string; onChange: (v: string) => void }) => (
    <select className="input text-sm py-1" value={value || ''} onChange={(e) => onChange(e.target.value)}>
      <option value="">—</option>
      {accounts.map((a) => <option key={a.code} value={a.code}>{a.code} · {a.name}</option>)}
    </select>
  );

  const genCell = (biz: string, prod: string) => data.general_matrix.find(c => c.gen_biz_group_id === biz && c.gen_prod_group_id === prod) || { gen_biz_group_id: biz, gen_prod_group_id: prod } as GenCell;
  const vatCell = (biz: string, prod: string) => data.vat_matrix.find(c => c.vat_biz_group_id === biz && c.vat_prod_group_id === prod) || { vat_biz_group_id: biz, vat_prod_group_id: prod, vat_rate: 0 } as VatCell;

  const saveGen = (cell: GenCell, patch: Partial<GenCell>) =>
    genMut.mutate({ gen_biz_group_id: cell.gen_biz_group_id, gen_prod_group_id: cell.gen_prod_group_id, sales_account: cell.sales_account, purchase_account: cell.purchase_account, cogs_account: cell.cogs_account, ...patch });
  const saveVat = (cell: VatCell, patch: Partial<VatCell>) =>
    vatMut.mutate({ vat_biz_group_id: cell.vat_biz_group_id, vat_prod_group_id: cell.vat_prod_group_id, vat_rate: Number((patch.vat_rate ?? cell.vat_rate) || 0), vat_output_account: cell.vat_output_account, vat_input_account: cell.vat_input_account, ...patch as any });

  const AddGroup = ({ kind, label }: { kind: string; label: string }) => {
    const [open, setOpen] = useState(false);
    const [code, setCode] = useState(''); const [name, setName] = useState('');
    if (!open) return <button onClick={() => setOpen(true)} className="text-xs text-blue-600 hover:underline inline-flex items-center gap-1"><Plus className="w-3 h-3" /> {label}</button>;
    return (
      <span className="inline-flex items-center gap-1">
        <input className="input text-xs py-0.5 w-20" placeholder="CODE" value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} />
        <input className="input text-xs py-0.5 w-28" placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} />
        <button onClick={() => { if (code && name) { groupMut.mutate({ kind, code, name }); setOpen(false); setCode(''); setName(''); } }} className="btn-primary text-xs py-0.5 px-2">Add</button>
        <button onClick={() => setOpen(false)} className="text-xs text-gray-400">✕</button>
      </span>
    );
  };

  return (
    <div className="space-y-8">
      <div className="flex items-start gap-2 p-3 bg-blue-50 border border-blue-200 rounded-lg text-sm text-blue-800">
        <Info className="w-4 h-4 mt-0.5 shrink-0" />
        <span>
          Posting groups derive GL accounts automatically from a party's <b>business group</b> and a product's <b>product group</b>,
          so you don't pick an account on every line. A line can still override the derived account.
        </span>
      </div>

      {/* Control accounts per business group (A/R, A/P) */}
      <section>
        <div className="flex items-center justify-between mb-2">
          <h3 className="font-semibold text-gray-900">Control Accounts <span className="text-gray-400 font-normal text-sm">(receivables / payables per business group)</span></h3>
          <AddGroup kind="general_business" label="Business group" />
        </div>
        <p className="text-xs text-gray-500 mb-2">Customers and vendors post their A/R and A/P to the control account of their business group, so you can keep e.g. domestic and export balances on separate accounts. Blank = default account.</p>
        <div className="overflow-x-auto border rounded-lg">
          <table className="w-full text-sm">
            <thead><tr className="bg-gray-50 text-xs uppercase text-gray-500">
              <th className="px-3 py-2 text-left">Business group</th>
              <th className="px-3 py-2 text-left">Receivables (A/R)</th><th className="px-3 py-2 text-left">Payables (A/P)</th>
            </tr></thead>
            <tbody className="divide-y">
              {data.general_business.map(b => (
                <tr key={b.id}>
                  <td className="px-3 py-1.5 font-medium">{b.code} <span className="text-gray-400 font-normal">{b.name}</span></td>
                  <td className="px-3 py-1.5"><AccountSelect value={b.receivables_account} onChange={(v) => ctrlMut.mutate({ gen_biz_group_id: b.id, receivables_account: v, payables_account: b.payables_account })} /></td>
                  <td className="px-3 py-1.5"><AccountSelect value={b.payables_account} onChange={(v) => ctrlMut.mutate({ gen_biz_group_id: b.id, receivables_account: b.receivables_account, payables_account: v })} /></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {/* General matrix */}
      <section>
        <div className="flex items-center justify-between mb-2">
          <h3 className="font-semibold text-gray-900">General Posting Matrix <span className="text-gray-400 font-normal text-sm">(sales / purchase / COGS)</span></h3>
          <div className="flex gap-3"><AddGroup kind="general_business" label="Business group" /><AddGroup kind="general_product" label="Product group" /></div>
        </div>
        <div className="overflow-x-auto border rounded-lg">
          <table className="w-full text-sm">
            <thead><tr className="bg-gray-50 text-xs uppercase text-gray-500">
              <th className="px-3 py-2 text-left">Business</th><th className="px-3 py-2 text-left">Product</th>
              <th className="px-3 py-2 text-left">Sales</th><th className="px-3 py-2 text-left">Purchase</th><th className="px-3 py-2 text-left">COGS</th>
            </tr></thead>
            <tbody className="divide-y">
              {data.general_business.flatMap(b => data.general_product.map(p => {
                const c = genCell(b.id, p.id);
                return (
                  <tr key={b.id + p.id}>
                    <td className="px-3 py-1.5 font-medium">{b.code}</td>
                    <td className="px-3 py-1.5">{p.code}</td>
                    <td className="px-3 py-1.5"><AccountSelect value={c.sales_account} onChange={(v) => saveGen(c, { sales_account: v })} /></td>
                    <td className="px-3 py-1.5"><AccountSelect value={c.purchase_account} onChange={(v) => saveGen(c, { purchase_account: v })} /></td>
                    <td className="px-3 py-1.5"><AccountSelect value={c.cogs_account} onChange={(v) => saveGen(c, { cogs_account: v })} /></td>
                  </tr>
                );
              }))}
            </tbody>
          </table>
        </div>
      </section>

      {/* VAT matrix */}
      <section>
        <div className="flex items-center justify-between mb-2">
          <h3 className="font-semibold text-gray-900">VAT Posting Matrix <span className="text-gray-400 font-normal text-sm">(rate / output / input)</span></h3>
          <div className="flex gap-3"><AddGroup kind="vat_business" label="VAT business group" /><AddGroup kind="vat_product" label="VAT product group" /></div>
        </div>
        <div className="overflow-x-auto border rounded-lg">
          <table className="w-full text-sm">
            <thead><tr className="bg-gray-50 text-xs uppercase text-gray-500">
              <th className="px-3 py-2 text-left">Business</th><th className="px-3 py-2 text-left">Product</th>
              <th className="px-3 py-2 text-left">Rate %</th><th className="px-3 py-2 text-left">Output VAT</th><th className="px-3 py-2 text-left">Input VAT</th>
            </tr></thead>
            <tbody className="divide-y">
              {data.vat_business.flatMap(b => data.vat_product.map(p => {
                const c = vatCell(b.id, p.id);
                return (
                  <tr key={b.id + p.id}>
                    <td className="px-3 py-1.5 font-medium">{b.code}</td>
                    <td className="px-3 py-1.5">{p.code}</td>
                    <td className="px-3 py-1.5">
                      <input type="number" step="0.01" min="0" className="input text-sm py-1 w-20" defaultValue={Number(c.vat_rate) * 100}
                        onBlur={(e) => saveVat(c, { vat_rate: (Number(e.target.value) || 0) / 100 })} />
                    </td>
                    <td className="px-3 py-1.5"><AccountSelect value={c.vat_output_account} onChange={(v) => saveVat(c, { vat_output_account: v })} /></td>
                    <td className="px-3 py-1.5"><AccountSelect value={c.vat_input_account} onChange={(v) => saveVat(c, { vat_input_account: v })} /></td>
                  </tr>
                );
              }))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}
