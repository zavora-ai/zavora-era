import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getTaxFilings, fileTaxReturn, remitTaxFiling, getAccounts } from '../../api/client';
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
