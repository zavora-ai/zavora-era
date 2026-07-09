import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getTaxFilings, fileTaxReturn, remitTaxFiling, getAccounts, getCitEstimate } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency, formatDate } from '../../utils/format';
import { Plus } from 'lucide-react';

const today = new Date().toISOString().split('T')[0];
const monthStart = `${new Date().getFullYear()}-${String(new Date().getMonth() + 1).padStart(2, '0')}-01`;

export default function TaxFilingsPage() {
  const qc = useQueryClient();
  const { data: listRes } = useQuery({ queryKey: ['tax-filings'], queryFn: getTaxFilings });
  const filings: any[] = listRes?.data ?? [];
  const { data: accountsRes } = useQuery({ queryKey: ['accounts'], queryFn: getAccounts });
  const accounts: any[] = accountsRes?.data ?? [];
  const liabilities = accounts.filter((a) => a.account_type === 'Liability');
  const assets = accounts.filter((a) => a.account_type === 'Asset');

  const [taxType, setTaxType] = useState('VAT');
  const [from, setFrom] = useState(monthStart);
  const [to, setTo] = useState(today);
  const [amount, setAmount] = useState('');
  const invalidate = () => qc.invalidateQueries({ queryKey: ['tax-filings'] });

  const file = useMutation({
    mutationFn: () => fileTaxReturn({ tax_type: taxType, period_from: from, period_to: to, amount: Number(amount) }),
    onSuccess: () => { setAmount(''); invalidate(); },
  });

  const [remitId, setRemitId] = useState<string | null>(null);

  return (
    <div>
      <PageHeader title="Tax Filing & Remittance" subtitle="Record VAT/PAYE/WHT returns as filed and the payment to KRA, so the ledger reflects tax paid." />

      <CitCard />

      <div className="card p-4 mb-5 flex flex-wrap items-end gap-3">
        <div><label className="label">Tax</label><select className="input" value={taxType} onChange={(e) => setTaxType(e.target.value)}><option>VAT</option><option>PAYE</option><option>WHT</option></select></div>
        <div><label className="label">Period from</label><input type="date" className="input" value={from} onChange={(e) => setFrom(e.target.value)} /></div>
        <div><label className="label">Period to</label><input type="date" className="input" value={to} onChange={(e) => setTo(e.target.value)} /></div>
        <div><label className="label">Amount due</label><input type="number" step="0.01" className="input" value={amount} onChange={(e) => setAmount(e.target.value)} placeholder="from the return report" /></div>
        <button className="btn-primary" disabled={!amount || file.isPending} onClick={() => file.mutate()}><Plus className="w-4 h-4" /> Record filing</button>
      </div>

      <div className="card p-5">
        <table className="w-full text-sm">
          <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Tax</th><th className="text-left">Period</th><th className="text-right">Amount</th><th className="text-left pl-4">Status</th><th></th></tr></thead>
          <tbody>
            {filings.map((f) => (
              <>
                <tr key={f.id} className="border-b border-gray-50">
                  <td className="py-2 font-medium">{f.tax_type}</td>
                  <td>{f.period_from} → {f.period_to}</td>
                  <td className="text-right tabular-nums">{formatCurrency(Number(f.amount))}</td>
                  <td className="pl-4">
                    {f.status === 'remitted'
                      ? <span className="text-xs text-green-700 bg-green-50 px-2 py-0.5 rounded">Remitted {formatDate(f.remitted_at)}</span>
                      : <span className="text-xs text-amber-700 bg-amber-50 px-2 py-0.5 rounded">Filed {formatDate(f.filed_at)}</span>}
                  </td>
                  <td className="text-right">
                    {f.status !== 'remitted' && <button className="btn-secondary text-xs py-1" onClick={() => setRemitId(remitId === f.id ? null : f.id)}>Record remittance</button>}
                  </td>
                </tr>
                {remitId === f.id && (
                  <tr key={f.id + '-r'}><td colSpan={5} className="bg-gray-50 px-4 py-3">
                    <RemitForm filing={f} liabilities={liabilities} assets={assets} onDone={() => { setRemitId(null); invalidate(); }} />
                  </td></tr>
                )}
              </>
            ))}
            {filings.length === 0 && <tr><td colSpan={5} className="py-4 text-center text-gray-400">No filings yet.</td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function RemitForm({ filing, liabilities, assets, onDone }: { filing: any; liabilities: any[]; assets: any[]; onDone: () => void }) {
  const [liability, setLiability] = useState('');
  const [bank, setBank] = useState('');
  const [date, setDate] = useState(today);
  const mut = useMutation({
    mutationFn: () => remitTaxFiling(filing.id, { liability_account: liability, bank_account_code: bank, payment_date: date }),
    onSuccess: onDone,
  });
  return (
    <div className="flex flex-wrap items-end gap-3">
      <div><label className="label">Liability account (DR)</label><select className="input" value={liability} onChange={(e) => setLiability(e.target.value)}><option value="">Select…</option>{liabilities.map((a) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}</select></div>
      <div><label className="label">Bank (CR)</label><select className="input" value={bank} onChange={(e) => setBank(e.target.value)}><option value="">Select…</option>{assets.map((a) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}</select></div>
      <div><label className="label">Payment date</label><input type="date" className="input" value={date} onChange={(e) => setDate(e.target.value)} /></div>
      <button className="btn-primary" disabled={!liability || !bank || mut.isPending} onClick={() => mut.mutate()}>{mut.isPending ? 'Posting…' : `Pay ${formatCurrency(Number(filing.amount))}`}</button>
      {mut.isError && <span className="text-xs text-red-600">{(mut.error as any)?.response?.data?.error ?? 'Failed'}</span>}
    </div>
  );
}

/** Corporation tax: the ledger-true estimate + the installment calendar
 * (decision support — iTax is the filing of record). */
function CitCard() {
  const { data, isLoading, isError } = useQuery({ queryKey: ['cit-estimate'], queryFn: () => getCitEstimate() });
  const est: any = data?.data;
  if (isLoading) return <div className="card p-5 mb-6 text-sm text-slate-500">Computing corporation-tax estimate…</div>;
  if (isError || !est) return null;
  const badge = (st: string) =>
    st === 'paid' ? 'bg-emerald-100 text-emerald-700' : st === 'due' ? 'bg-red-100 text-red-700' : 'bg-slate-100 text-slate-600';
  return (
    <div className="card p-5 mb-6">
      <div className="flex flex-wrap items-baseline justify-between gap-2 mb-3">
        <h2 className="font-semibold text-slate-800">Corporation tax — FY ending {formatDate(est.fiscal_year_end)}</h2>
        <span className="text-xs text-slate-500">estimate · iTax is the filing of record</span>
      </div>
      <div className="grid grid-cols-2 md:grid-cols-5 gap-4 text-sm mb-4">
        <div><div className="text-xs text-slate-500">Accounting profit</div><div className="font-semibold">{formatCurrency(Number(est.accounting_profit))}</div></div>
        <div><div className="text-xs text-slate-500">+ Depreciation</div><div className="font-semibold">{formatCurrency(Number(est.depreciation_add_back))}</div></div>
        <div><div className="text-xs text-slate-500">− Capital allowances</div><div className="font-semibold">{formatCurrency(Number(est.capital_allowances))}</div></div>
        <div><div className="text-xs text-slate-500">Taxable (est.)</div><div className="font-semibold">{formatCurrency(Number(est.taxable_profit_estimate))}</div></div>
        <div><div className="text-xs text-slate-500">CIT @ {est.cit_rate_percent}%</div><div className="font-semibold text-slate-900">{formatCurrency(Number(est.estimated_tax))}</div></div>
      </div>
      <table className="w-full text-sm">
        <thead><tr className="text-left text-xs text-slate-500 border-b"><th className="py-1.5">Installment</th><th>Due</th><th className="text-right">Amount</th><th className="text-right">Status</th></tr></thead>
        <tbody>
          {(est.installments ?? []).map((i: any) => (
            <tr key={i.label} className="border-b border-slate-100">
              <td className="py-1.5">{i.label}</td>
              <td>{formatDate(i.due_date)}</td>
              <td className="text-right">{formatCurrency(Number(i.amount))}</td>
              <td className="text-right"><span className={`px-2 py-0.5 rounded-full text-xs font-medium ${badge(i.status)}`}>{i.status}</span></td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="mt-3 text-xs text-slate-500">
        Paid to date {formatCurrency(Number(est.paid_to_date))} · balance of tax due {formatDate(est.balance_due_date)} ·
        record installments below as tax type <span className="font-mono">CIT-installment</span>.
      </p>
    </div>
  );
}
