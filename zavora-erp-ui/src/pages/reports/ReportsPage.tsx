import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { generateReport, exportReport } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency } from '../../utils/format';
import { BarChart3, FileDown, CheckCircle2, AlertTriangle } from 'lucide-react';

const ZERO_ENTITY = '00000000-0000-0000-0000-000000000000';

type CtrlKind = 'asAt' | 'period' | 'account';

const reportTypes: { key: string; name: string; desc: string; controls: CtrlKind[]; comparable?: boolean }[] = [
  { key: 'TrialBalance', name: 'Trial Balance', desc: 'Account balances at a point in time', controls: ['asAt'] },
  { key: 'BalanceSheet', name: 'Balance Sheet', desc: 'Assets, liabilities, and equity', controls: ['asAt'], comparable: true },
  { key: 'ProfitAndLoss', name: 'Profit & Loss', desc: 'Revenue and expenses for a period', controls: ['period'], comparable: true },
  { key: 'CashFlow', name: 'Cash Flow Statement', desc: 'Cash movements (indirect method)', controls: ['period'] },
  { key: 'ArAgeing', name: 'AR Ageing', desc: 'Customer balances by age bucket', controls: ['asAt'] },
  { key: 'ApAgeing', name: 'AP Ageing', desc: 'Vendor balances by age bucket', controls: ['asAt'] },
  { key: 'VatReturn', name: 'VAT Return', desc: 'Output vs input VAT, net payable to KRA', controls: ['period'] },
  { key: 'GlDetail', name: 'General Ledger', desc: 'Transaction detail by account', controls: ['period', 'account'] },
];

const today = new Date().toISOString().split('T')[0];
const yearStart = `${new Date().getFullYear()}-01-01`;

export default function ReportsPage() {
  const [selected, setSelected] = useState<string>('TrialBalance');
  const [asAt, setAsAt] = useState(today);
  const [from, setFrom] = useState(yearStart);
  const [to, setTo] = useState(today);
  const [account, setAccount] = useState('1200');
  const [compare, setCompare] = useState(false);
  const [result, setResult] = useState<any>(null);

  const meta = reportTypes.find((r) => r.key === selected)!;

  const buildReq = () => ({
    entity_id: ZERO_ENTITY,
    report_type: selected,
    parameters: {
      as_at: meta.controls.includes('asAt') ? asAt : null,
      period_from: meta.controls.includes('period') ? from : null,
      period_to: meta.controls.includes('period') ? to : null,
      account_code: meta.controls.includes('account') ? account : null,
      comparative: meta.comparable ? compare : false,
    },
  });

  const mutation = useMutation({
    mutationFn: () => generateReport(buildReq()),
    onSuccess: (res) => setResult(res.data),
  });

  const exportMutation = useMutation({
    mutationFn: () => exportReport(buildReq()),
    onSuccess: (res) => {
      const url = URL.createObjectURL(new Blob([res.data], { type: 'text/csv' }));
      const a = document.createElement('a');
      a.href = url;
      a.download = `${selected}-${today}.csv`;
      a.click();
      URL.revokeObjectURL(url);
    },
  });

  const select = (key: string) => { setSelected(key); setResult(null); };

  return (
    <div>
      <PageHeader title="Reports" subtitle="Financial and compliance reports" />

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3 mb-5">
        {reportTypes.map((rt) => (
          <button
            key={rt.key}
            onClick={() => select(rt.key)}
            className={`card p-3 text-left transition-all ${selected === rt.key ? 'border-indigo-400 ring-1 ring-indigo-200' : 'hover:border-gray-300'}`}
          >
            <div className="flex items-start gap-2">
              <BarChart3 className={`w-4 h-4 mt-0.5 shrink-0 ${selected === rt.key ? 'text-indigo-600' : 'text-gray-400'}`} />
              <div>
                <p className="text-sm font-medium text-gray-900">{rt.name}</p>
                <p className="text-[11px] text-gray-500 mt-0.5">{rt.desc}</p>
              </div>
            </div>
          </button>
        ))}
      </div>

      {/* Controls */}
      <div className="card p-4 mb-5 flex flex-wrap items-end gap-4">
        {meta.controls.includes('asAt') && (
          <div>
            <label className="label">As at</label>
            <input type="date" className="input" value={asAt} onChange={(e) => setAsAt(e.target.value)} />
          </div>
        )}
        {meta.controls.includes('period') && (
          <>
            <div><label className="label">From</label><input type="date" className="input" value={from} onChange={(e) => setFrom(e.target.value)} /></div>
            <div><label className="label">To</label><input type="date" className="input" value={to} onChange={(e) => setTo(e.target.value)} /></div>
          </>
        )}
        {meta.controls.includes('account') && (
          <div><label className="label">Account code</label><input className="input w-32" value={account} onChange={(e) => setAccount(e.target.value)} placeholder="1200" /></div>
        )}
        {meta.comparable && (
          <label className="flex items-center gap-2 text-sm text-gray-600 cursor-pointer pb-2">
            <input type="checkbox" checked={compare} onChange={(e) => setCompare(e.target.checked)} className="rounded" />
            Compare to prior year
          </label>
        )}
        <div className="flex-1" />
        <button onClick={() => mutation.mutate()} className="btn-primary" disabled={mutation.isPending}>
          {mutation.isPending ? 'Generating…' : 'Generate'}
        </button>
        <button onClick={() => exportMutation.mutate()} className="btn-secondary" disabled={exportMutation.isPending}>
          <FileDown className="w-4 h-4" /> CSV
        </button>
      </div>

      {result && <ReportView result={result} />}
    </div>
  );
}

function Balanced({ ok, diff }: { ok: boolean; diff: number }) {
  return ok ? (
    <span className="inline-flex items-center gap-1 text-xs font-medium text-green-700 bg-green-50 px-2 py-1 rounded">
      <CheckCircle2 className="w-3.5 h-3.5" /> Balanced
    </span>
  ) : (
    <span className="inline-flex items-center gap-1 text-xs font-medium text-red-700 bg-red-50 px-2 py-1 rounded">
      <AlertTriangle className="w-3.5 h-3.5" /> Out of balance by {formatCurrency(Math.abs(diff))}
    </span>
  );
}

function ReportView({ result }: { result: any }) {
  const content = result.content ?? {};
  const key = Object.keys(content)[0];
  const c = content[key];

  return (
    <div className="card p-6">
      <div className="flex items-center justify-between mb-4">
        <h3 className="font-semibold text-gray-900">{result.title || key}</h3>
        {key === 'TrialBalance' && <Balanced ok={c.is_balanced} diff={c.difference} />}
        {key === 'BalanceSheet' && <Balanced ok={c.is_balanced} diff={c.difference} />}
      </div>

      {key === 'TrialBalance' && <TrialBalance c={c} />}
      {key === 'BalanceSheet' && <BalanceSheet c={c} />}
      {key === 'ProfitAndLoss' && <ProfitAndLoss c={c} />}
      {key === 'VatReturn' && <VatReturn c={c} />}
      {key === 'GlDetail' && <GlDetail c={c} />}
      {!['TrialBalance', 'BalanceSheet', 'ProfitAndLoss', 'VatReturn', 'GlDetail'].includes(key) && (
        <pre className="text-xs bg-gray-50 p-4 rounded-lg overflow-auto max-h-96">{JSON.stringify(c, null, 2)}</pre>
      )}
    </div>
  );
}

const num = (n: number) => <span className="tabular-nums">{formatCurrency(n)}</span>;

function TrialBalance({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Account</th><th className="text-right">Debit</th><th className="text-right">Credit</th></tr></thead>
      <tbody>
        {c.lines.map((l: any) => (
          <tr key={l.account_code} className="border-b border-gray-50">
            <td className="py-1.5"><span className="font-mono text-xs text-gray-400">{l.account_code}</span> {l.account_name}</td>
            <td className="text-right">{l.closing_debit ? num(l.closing_debit) : '—'}</td>
            <td className="text-right">{l.closing_credit ? num(l.closing_credit) : '—'}</td>
          </tr>
        ))}
      </tbody>
      <tfoot><tr className="font-bold border-t-2"><td className="py-2">Total</td><td className="text-right">{num(c.total_debits)}</td><td className="text-right">{num(c.total_credits)}</td></tr></tfoot>
    </table>
  );
}

function TwoColHead({ comparative, label }: { comparative?: string; label: string }) {
  return (
    <tr className="text-xs text-gray-500 uppercase border-b">
      <th className="text-left py-2">{label}</th>
      <th className="text-right">Amount</th>
      {comparative && <th className="text-right">{comparative}</th>}
    </tr>
  );
}

function Section({ title, section, comparative }: { title: string; section: any; comparative?: string }) {
  return (
    <>
      <tr className="bg-gray-50"><td className="py-1.5 font-semibold text-gray-700" colSpan={comparative ? 3 : 2}>{title}</td></tr>
      {section.lines.map((l: any) => (
        <tr key={l.account_code + l.account_name} className="border-b border-gray-50">
          <td className="py-1.5 pl-4"><span className="font-mono text-xs text-gray-400">{l.account_code}</span> {l.account_name}</td>
          <td className="text-right">{num(l.amount)}</td>
          {comparative && <td className="text-right text-gray-500">{l.comparative != null ? num(l.comparative) : '—'}</td>}
        </tr>
      ))}
      <tr className="font-medium border-b"><td className="py-1.5 pl-4">Total {title}</td><td className="text-right">{num(section.total)}</td>{comparative && <td />}</tr>
    </>
  );
}

function BalanceSheet({ c }: { c: any }) {
  const cmp = c.comparative_as_at as string | null;
  return (
    <table className="w-full text-sm">
      <thead><TwoColHead label={`As at ${c.as_at}`} comparative={cmp ?? undefined} /></thead>
      <tbody>
        {c.assets.map((s: any, i: number) => <Section key={'a' + i} title={s.name} section={s} comparative={cmp ?? undefined} />)}
        <tr className="font-bold"><td className="py-2">Total Assets</td><td className="text-right">{num(c.total_assets)}</td>{cmp && <td className="text-right">{c.total_assets_comparative != null ? num(c.total_assets_comparative) : '—'}</td>}</tr>
        {c.liabilities.map((s: any, i: number) => <Section key={'l' + i} title={s.name} section={s} comparative={cmp ?? undefined} />)}
        {c.equity.map((s: any, i: number) => <Section key={'e' + i} title={s.name} section={s} comparative={cmp ?? undefined} />)}
        <tr className="font-bold border-t-2"><td className="py-2">Total Liabilities + Equity</td><td className="text-right">{num(c.total_liabilities + c.total_equity)}</td>{cmp && <td className="text-right">{num((c.total_liabilities_comparative ?? 0) + (c.total_equity_comparative ?? 0))}</td>}</tr>
      </tbody>
    </table>
  );
}

function PnlRow({ label, amount, comparative, cmp, bold }: { label: string; amount: number; comparative?: number | null; cmp: boolean; bold?: boolean }) {
  return (
    <tr className={bold ? 'font-bold border-t' : 'border-b border-gray-50'}>
      <td className="py-1.5">{label}</td>
      <td className="text-right">{num(amount)}</td>
      {cmp && <td className="text-right text-gray-500">{comparative != null ? num(comparative) : '—'}</td>}
    </tr>
  );
}

function ProfitAndLoss({ c }: { c: any }) {
  const cmp = c.comparative_from != null;
  const cmpLabel = cmp ? `${c.comparative_from} – ${c.comparative_to}` : undefined;
  return (
    <table className="w-full text-sm">
      <thead><TwoColHead label={`${c.period_from} – ${c.period_to}`} comparative={cmpLabel} /></thead>
      <tbody>
        {c.revenue.map((s: any, i: number) => <Section key={'r' + i} title={s.name} section={s} comparative={cmpLabel} />)}
        {c.cost_of_sales.map((s: any, i: number) => <Section key={'c' + i} title={s.name} section={s} comparative={cmpLabel} />)}
        <PnlRow label="Gross Profit" amount={c.gross_profit} comparative={c.gross_profit_comparative} cmp={cmp} bold />
        {c.operating_expenses.map((s: any, i: number) => <Section key={'o' + i} title={s.name} section={s} comparative={cmpLabel} />)}
        <PnlRow label="Operating Profit" amount={c.operating_profit} comparative={c.operating_profit_comparative} cmp={cmp} bold />
        {c.other_income_expense.map((s: any, i: number) => <Section key={'x' + i} title={s.name} section={s} comparative={cmpLabel} />)}
        <PnlRow label="Net Profit" amount={c.net_profit} comparative={c.net_profit_comparative} cmp={cmp} bold />
      </tbody>
    </table>
  );
}

function VatReturn({ c }: { c: any }) {
  return (
    <div className="max-w-md space-y-2 text-sm">
      <p className="text-xs text-gray-500">Period {c.period_from} – {c.period_to}</p>
      <div className="flex justify-between border-b py-1.5"><span>Output VAT (on sales)</span>{num(c.output_vat)}</div>
      <div className="flex justify-between border-b py-1.5"><span>Input VAT (on purchases)</span>{num(c.input_vat)}</div>
      <div className="flex justify-between font-bold border-t-2 pt-2">
        <span>{c.is_payable ? 'Net VAT payable to KRA' : 'Net VAT credit carried forward'}</span>
        <span className={c.is_payable ? 'text-red-600' : 'text-green-600'}>{formatCurrency(Math.abs(c.net_vat))}</span>
      </div>
    </div>
  );
}

function GlDetail({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Date</th><th className="text-left">JE #</th><th className="text-left">Reference</th><th className="text-right">Debit</th><th className="text-right">Credit</th><th className="text-right">Balance</th></tr></thead>
      <tbody>
        <tr className="border-b border-gray-50 text-gray-500"><td className="py-1.5" colSpan={5}>Opening balance — {c.account_code} {c.account_name}</td><td className="text-right font-medium">{num(c.opening_balance)}</td></tr>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5">{l.date}</td><td className="font-mono text-xs">{l.journal_number}</td><td className="text-gray-500">{l.reference}</td>
            <td className="text-right">{l.debit ? num(l.debit) : '—'}</td><td className="text-right">{l.credit ? num(l.credit) : '—'}</td><td className="text-right">{num(l.balance)}</td>
          </tr>
        ))}
        <tr className="font-bold border-t-2"><td className="py-2" colSpan={5}>Closing balance</td><td className="text-right">{num(c.closing_balance)}</td></tr>
      </tbody>
    </table>
  );
}
