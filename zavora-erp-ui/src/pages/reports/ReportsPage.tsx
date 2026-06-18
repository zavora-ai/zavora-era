import { useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { generateReport, exportReport, getSettings, getCustomers, getVendors } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency } from '../../utils/format';
import { BarChart3, FileDown, FileSpreadsheet, Printer, CheckCircle2, AlertTriangle } from 'lucide-react';

const ZERO_ENTITY = '00000000-0000-0000-0000-000000000000';

type CtrlKind = 'asAt' | 'period' | 'account' | 'party';

const reportTypes: { key: string; name: string; desc: string; controls: CtrlKind[]; comparable?: boolean; party?: 'customer' | 'vendor' }[] = [
  { key: 'TrialBalance', name: 'Trial Balance', desc: 'Account balances at a point in time', controls: ['asAt'] },
  { key: 'BalanceSheet', name: 'Balance Sheet', desc: 'Assets, liabilities, and equity', controls: ['asAt'], comparable: true },
  { key: 'ProfitAndLoss', name: 'Profit & Loss', desc: 'Revenue and expenses for a period', controls: ['period'], comparable: true },
  { key: 'CashFlow', name: 'Cash Flow Statement', desc: 'Cash movements (indirect method)', controls: ['period'] },
  { key: 'ArAgeing', name: 'AR Ageing', desc: 'Customer balances by age bucket', controls: ['asAt'] },
  { key: 'ApAgeing', name: 'AP Ageing', desc: 'Vendor balances by age bucket', controls: ['asAt'] },
  { key: 'VatReturn', name: 'VAT Return', desc: 'Output vs input VAT, net payable to KRA', controls: ['period'] },
  { key: 'CustomerStatement', name: 'Customer Statement', desc: 'Account activity & balance for one customer', controls: ['party', 'period'], party: 'customer' },
  { key: 'VendorStatement', name: 'Vendor Statement', desc: 'Account activity & balance for one vendor', controls: ['party', 'period'], party: 'vendor' },
  { key: 'PayrollSummary', name: 'Payroll Summary', desc: 'Gross, PAYE, NSSF, SHA, levy & net by employee', controls: ['period'] },
  { key: 'PayeP10', name: 'PAYE Return (P10)', desc: 'KRA monthly PAYE schedule by employee', controls: ['period'] },
  { key: 'WhtCertificate', name: 'WHT Schedule', desc: 'Withholding tax withheld from suppliers', controls: ['period'] },
  { key: 'SalesTaxSummary', name: 'VAT by Rate', desc: 'Output & input VAT broken down by rate band', controls: ['period'] },
  { key: 'IncomeByCustomer', name: 'Income by Customer', desc: 'Net revenue ranked by customer', controls: ['period'] },
  { key: 'ExpenseByVendor', name: 'Expense by Vendor', desc: 'Net spend ranked by vendor', controls: ['period'] },
  { key: 'InventoryValuation', name: 'Inventory Valuation', desc: 'On-hand quantity, cost & value by item', controls: ['asAt'] },
  { key: 'FixedAssetRegister', name: 'Fixed-Asset Register', desc: 'Cost, depreciation & net book value', controls: ['asAt'] },
  { key: 'BankReconSummary', name: 'Bank Reconciliation', desc: 'Statement vs GL balance, matched & unmatched', controls: ['asAt'] },
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
  const [partyId, setPartyId] = useState('');
  const [compare, setCompare] = useState(false);
  const [result, setResult] = useState<any>(null);

  const meta = reportTypes.find((r) => r.key === selected)!;
  const { data: settingsRes } = useQuery({ queryKey: ['settings'], queryFn: getSettings });
  const branding = settingsRes?.data?.branding;

  const { data: customersRes } = useQuery({ queryKey: ['customers'], queryFn: getCustomers });
  const { data: vendorsRes } = useQuery({ queryKey: ['vendors'], queryFn: getVendors });
  const parties: { id: string; name: string }[] = (meta.party === 'vendor' ? vendorsRes?.data : customersRes?.data) ?? [];
  const needsParty = meta.controls.includes('party');

  const buildReq = () => ({
    entity_id: ZERO_ENTITY,
    report_type: selected,
    parameters: {
      as_at: meta.controls.includes('asAt') ? asAt : null,
      period_from: meta.controls.includes('period') ? from : null,
      period_to: meta.controls.includes('period') ? to : null,
      account_code: meta.controls.includes('account') ? account : null,
      customer_id: meta.party === 'customer' ? partyId || null : null,
      vendor_id: meta.party === 'vendor' ? partyId || null : null,
      comparative: meta.comparable ? compare : false,
    },
  });

  const mutation = useMutation({
    mutationFn: () => generateReport(buildReq()),
    onSuccess: (res) => setResult(res.data),
  });

  const exportMutation = useMutation({
    mutationFn: () => exportReport(buildReq()),
    onSuccess: (res) => downloadBlob(new Blob([res.data], { type: 'text/csv' }), `${selected}-${today}.csv`),
  });

  const select = (key: string) => { setSelected(key); setResult(null); };
  const exportExcel = () => exportDomAsExcel(result?.title || meta.name);

  return (
    <div>
      <div className="no-print">
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
          {needsParty && (
            <div>
              <label className="label">{meta.party === 'vendor' ? 'Vendor' : 'Customer'}</label>
              <select className="input min-w-[12rem]" value={partyId} onChange={(e) => setPartyId(e.target.value)}>
                <option value="">Select {meta.party}…</option>
                {parties.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
              </select>
            </div>
          )}
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
          <button onClick={() => mutation.mutate()} className="btn-primary" disabled={mutation.isPending || (needsParty && !partyId)}>
            {mutation.isPending ? 'Generating…' : 'Generate'}
          </button>
          <button onClick={() => window.print()} className="btn-secondary" disabled={!result} title="Print / save as PDF">
            <Printer className="w-4 h-4" /> Print
          </button>
          <button onClick={exportExcel} className="btn-secondary" disabled={!result} title="Export to Excel">
            <FileSpreadsheet className="w-4 h-4" /> Excel
          </button>
          <button onClick={() => exportMutation.mutate()} className="btn-secondary" disabled={exportMutation.isPending} title="Export to CSV">
            <FileDown className="w-4 h-4" /> CSV
          </button>
        </div>

        {mutation.isError && (
          <div className="card p-4 mb-5 flex items-center gap-2 text-sm text-red-700 bg-red-50 border-red-200">
            <AlertTriangle className="w-4 h-4" /> Could not generate this report. Check the dates and try again.
          </div>
        )}
      </div>

      {result && <ReportDocument result={result} branding={branding} />}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Document shell — full-page statement with header + footer          */
/* ------------------------------------------------------------------ */

function periodLabel(c: any): string {
  if (c?.as_at) return `As at ${c.as_at}`;
  if (c?.period_from && c?.period_to) return `For the period ${c.period_from} to ${c.period_to}`;
  return '';
}

function ReportDocument({ result, branding }: { result: any; branding: any }) {
  const content = result.content ?? {};
  const key = Object.keys(content)[0];
  const c = content[key];
  const b = branding ?? {};
  const generatedAt = new Date().toLocaleString('en-KE', { dateStyle: 'medium', timeStyle: 'short' });

  return (
    <div id="report-document" className="print-area mx-auto max-w-4xl bg-white border border-gray-200 rounded-lg shadow-sm">
      {/* Letterhead */}
      <div className="px-10 pt-10 pb-6 border-b border-gray-200">
        <div className="flex items-start justify-between gap-6">
          <div className="flex items-center gap-3">
            {b.logo_url && <img src={b.logo_url} alt="" className="h-12 w-auto object-contain" />}
            <div>
              <h1 className="text-xl font-bold text-gray-900 leading-tight">{b.company_name || 'Your Company'}</h1>
              <p className="text-xs text-gray-500 mt-0.5">
                {[b.address, b.phone, b.email].filter(Boolean).join('  ·  ')}
              </p>
              <p className="text-xs text-gray-500">
                {[b.kra_pin && `KRA PIN: ${b.kra_pin}`, b.vat_number && `VAT: ${b.vat_number}`].filter(Boolean).join('  ·  ')}
              </p>
            </div>
          </div>
          <div className="text-right shrink-0">
            {key === 'TrialBalance' && <Balanced ok={c.is_balanced} diff={c.difference} />}
            {key === 'BalanceSheet' && <Balanced ok={c.is_balanced} diff={c.difference} />}
          </div>
        </div>
        <div className="text-center mt-6">
          <h2 className="text-lg font-semibold text-gray-900">{result.title || key}</h2>
          <p className="text-sm text-gray-500 mt-0.5">{periodLabel(c)}</p>
        </div>
      </div>

      {/* Body */}
      <div className="px-10 py-8">
        {key === 'TrialBalance' && <TrialBalance c={c} />}
        {key === 'BalanceSheet' && <BalanceSheet c={c} />}
        {key === 'ProfitAndLoss' && <ProfitAndLoss c={c} />}
        {key === 'VatReturn' && <VatReturn c={c} />}
        {key === 'PartyStatement' && <PartyStatement c={c} />}
        {key === 'PayrollSummary' && <PayrollSummary c={c} />}
        {key === 'PayeP10' && <PayeP10 c={c} />}
        {key === 'WhtReport' && <WhtReport c={c} />}
        {key === 'VatDetail' && <VatDetail c={c} />}
        {key === 'PartyRanking' && <PartyRanking c={c} />}
        {key === 'InventoryValuation' && <InventoryValuation c={c} />}
        {key === 'FixedAssetRegister' && <FixedAssetRegister c={c} />}
        {key === 'BankReconSummary' && <BankReconSummary c={c} />}
        {key === 'GlDetail' && <GlDetail c={c} />}
        {!['TrialBalance', 'BalanceSheet', 'ProfitAndLoss', 'VatReturn', 'PartyStatement', 'PayrollSummary', 'PayeP10', 'WhtReport', 'VatDetail', 'PartyRanking', 'InventoryValuation', 'FixedAssetRegister', 'BankReconSummary', 'GlDetail'].includes(key) && (
          <pre className="text-xs bg-gray-50 p-4 rounded-lg overflow-auto max-h-96">{JSON.stringify(c, null, 2)}</pre>
        )}
      </div>

      {/* Footer */}
      <div className="px-10 py-4 border-t border-gray-200 text-[11px] text-gray-400 flex justify-between">
        <span>{b.footer_text || `${b.company_name || ''}`}</span>
        <span>Generated {generatedAt}</span>
      </div>
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
    <table className="w-full max-w-lg text-sm">
      <tbody>
        <tr className="border-b border-gray-50"><td className="py-1.5">Output VAT (on sales)</td><td className="text-right">{num(c.output_vat)}</td></tr>
        <tr className="border-b border-gray-50"><td className="py-1.5">Input VAT (on purchases)</td><td className="text-right">{num(c.input_vat)}</td></tr>
        <tr className="font-bold border-t-2">
          <td className="py-2">{c.is_payable ? 'Net VAT payable to KRA' : 'Net VAT credit carried forward'}</td>
          <td className={`text-right ${c.is_payable ? 'text-red-600' : 'text-green-600'}`}>{formatCurrency(Math.abs(c.net_vat))}</td>
        </tr>
      </tbody>
    </table>
  );
}

function PartyStatement({ c }: { c: any }) {
  return (
    <div>
      <p className="text-sm mb-3"><span className="text-gray-500">Statement for:</span> <span className="font-semibold text-gray-900">{c.party_name}</span></p>
      <table className="w-full text-sm">
        <thead>
          <tr className="text-xs text-gray-500 uppercase border-b">
            <th className="text-left py-2">Date</th>
            <th className="text-left">Type</th>
            <th className="text-left">Reference</th>
            <th className="text-right">Charge</th>
            <th className="text-right">Payment</th>
            <th className="text-right">Balance</th>
          </tr>
        </thead>
        <tbody>
          <tr className="border-b border-gray-50 text-gray-500">
            <td className="py-1.5" colSpan={5}>Opening balance</td>
            <td className="text-right font-medium">{num(c.opening_balance)}</td>
          </tr>
          {c.lines.map((l: any, i: number) => (
            <tr key={i} className="border-b border-gray-50">
              <td className="py-1.5">{l.date}</td>
              <td>{l.doc_type}</td>
              <td className="text-gray-500">{l.reference}</td>
              <td className="text-right">{Number(l.charge) ? num(l.charge) : '—'}</td>
              <td className="text-right">{Number(l.payment) ? num(l.payment) : '—'}</td>
              <td className="text-right">{num(l.balance)}</td>
            </tr>
          ))}
          {c.lines.length === 0 && (
            <tr><td colSpan={6} className="py-4 text-center text-gray-400">No activity in this period</td></tr>
          )}
        </tbody>
        <tfoot>
          <tr className="font-bold border-t-2">
            <td className="py-2" colSpan={3}>Closing balance</td>
            <td className="text-right">{num(c.total_charges)}</td>
            <td className="text-right">{num(c.total_payments)}</td>
            <td className="text-right">{num(c.closing_balance)}</td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}

function PayrollSummary({ c }: { c: any }) {
  const t = c.totals;
  const cols = ['Gross', 'PAYE', 'NSSF', 'SHA', 'Housing Levy', 'HELB', 'Net'];
  return (
    <div>
      <p className="text-sm text-gray-500 mb-3">
        {c.run_count} pay run{c.run_count === 1 ? '' : 's'} · {c.employee_count} employee{c.employee_count === 1 ? '' : 's'}
      </p>
      <table className="w-full text-sm">
        <thead>
          <tr className="text-xs text-gray-500 uppercase border-b">
            <th className="text-left py-2">Employee</th>
            {cols.map((h) => <th key={h} className="text-right">{h}</th>)}
          </tr>
        </thead>
        <tbody>
          {c.employees.map((e: any) => (
            <tr key={e.employee_id} className="border-b border-gray-50">
              <td className="py-1.5">{e.employee_name}</td>
              <td className="text-right">{num(e.gross)}</td>
              <td className="text-right">{num(e.paye)}</td>
              <td className="text-right">{num(e.nssf)}</td>
              <td className="text-right">{num(e.sha)}</td>
              <td className="text-right">{num(e.housing_levy)}</td>
              <td className="text-right">{num(e.helb)}</td>
              <td className="text-right font-medium">{num(e.net)}</td>
            </tr>
          ))}
          {c.employees.length === 0 && (
            <tr><td colSpan={8} className="py-4 text-center text-gray-400">No payroll in this period</td></tr>
          )}
        </tbody>
        <tfoot>
          <tr className="font-bold border-t-2">
            <td className="py-2">Total</td>
            <td className="text-right">{num(t.gross)}</td>
            <td className="text-right">{num(t.paye)}</td>
            <td className="text-right">{num(t.nssf)}</td>
            <td className="text-right">{num(t.sha)}</td>
            <td className="text-right">{num(t.housing_levy)}</td>
            <td className="text-right">{num(t.helb)}</td>
            <td className="text-right">{num(t.net)}</td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}

function PayeP10({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Employee</th>
          <th className="text-left">KRA PIN</th>
          <th className="text-right">Gross</th>
          <th className="text-right">Taxable</th>
          <th className="text-right">Tax</th>
          <th className="text-right">Relief</th>
          <th className="text-right">PAYE Due</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5">{l.employee_name} <span className="text-xs text-gray-400">{l.staff_number}</span></td>
            <td className="font-mono text-xs">{l.kra_pin}</td>
            <td className="text-right">{num(l.gross_pay)}</td>
            <td className="text-right">{num(l.taxable_pay)}</td>
            <td className="text-right">{num(l.tax)}</td>
            <td className="text-right text-gray-500">{num(Number(l.personal_relief) + Number(l.insurance_relief))}</td>
            <td className="text-right font-medium">{num(l.paye_payable)}</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={7} className="py-4 text-center text-gray-400">No payroll in this period</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={2}>Total</td>
          <td className="text-right">{num(c.total_gross)}</td>
          <td className="text-right">{num(c.total_taxable)}</td>
          <td className="text-right">{num(c.total_paye)}</td>
          <td className="text-right">{num(c.total_relief)}</td>
          <td className="text-right">{num(c.total_payable)}</td>
        </tr>
      </tfoot>
    </table>
  );
}

function WhtReport({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Date</th>
          <th className="text-left">Bill</th>
          <th className="text-left">Vendor</th>
          <th className="text-left">KRA PIN</th>
          <th className="text-left">Category</th>
          <th className="text-right">Base</th>
          <th className="text-right">WHT</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5">{l.date}</td>
            <td className="font-mono text-xs">{l.document_number}</td>
            <td>{l.vendor_name}</td>
            <td className="font-mono text-xs">{l.kra_pin || '—'}</td>
            <td className="text-gray-500">{l.wht_category || '—'}</td>
            <td className="text-right">{num(l.base_amount)}</td>
            <td className="text-right font-medium">{num(l.wht_amount)}</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={7} className="py-4 text-center text-gray-400">No withholding tax in this period</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={5}>Total</td>
          <td className="text-right">{num(c.total_base)}</td>
          <td className="text-right">{num(c.total_wht)}</td>
        </tr>
      </tfoot>
    </table>
  );
}

function VatBands({ title, bands, totalTaxable, totalVat }: { title: string; bands: any[]; totalTaxable: number; totalVat: number }) {
  return (
    <>
      <tr className="bg-gray-50"><td className="py-1.5 font-semibold text-gray-700" colSpan={4}>{title}</td></tr>
      {bands.map((b: any) => (
        <tr key={b.treatment} className="border-b border-gray-50">
          <td className="py-1.5 pl-4">{b.treatment} <span className="text-xs text-gray-400">({b.document_count} doc{b.document_count === 1 ? '' : 's'})</span></td>
          <td />
          <td className="text-right">{num(b.taxable_amount)}</td>
          <td className="text-right">{num(b.vat_amount)}</td>
        </tr>
      ))}
      {bands.length === 0 && <tr><td className="py-1.5 pl-4 text-gray-400" colSpan={4}>None</td></tr>}
      <tr className="font-medium border-b"><td className="py-1.5 pl-4">Total {title}</td><td /><td className="text-right">{num(totalTaxable)}</td><td className="text-right">{num(totalVat)}</td></tr>
    </>
  );
}

function VatDetail({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Rate band</th><th /><th className="text-right">Taxable</th><th className="text-right">VAT</th>
        </tr>
      </thead>
      <tbody>
        <VatBands title="Output VAT (Sales)" bands={c.output} totalTaxable={c.total_output_taxable} totalVat={c.total_output_vat} />
        <VatBands title="Input VAT (Purchases)" bands={c.input} totalTaxable={c.total_input_taxable} totalVat={c.total_input_vat} />
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={3}>{c.is_payable ? 'Net VAT payable to KRA' : 'Net VAT credit carried forward'}</td>
          <td className={`text-right ${c.is_payable ? 'text-red-600' : 'text-green-600'}`}>{formatCurrency(Math.abs(c.net_vat))}</td>
        </tr>
      </tbody>
    </table>
  );
}

function PartyRanking({ c }: { c: any }) {
  const party = c.party_kind === 'vendor' ? 'Vendor' : 'Customer';
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">{party}</th>
          <th className="text-right">Documents</th>
          <th className="text-right">Amount</th>
          <th className="text-right">% of Total</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any) => (
          <tr key={l.party_id} className="border-b border-gray-50">
            <td className="py-1.5">{l.party_name}</td>
            <td className="text-right text-gray-500">{l.document_count}</td>
            <td className="text-right">{num(l.amount)}</td>
            <td className="text-right text-gray-500">{Number(l.percent).toFixed(1)}%</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={4} className="py-4 text-center text-gray-400">No activity in this period</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2">Total</td>
          <td />
          <td className="text-right">{num(c.total)}</td>
          <td className="text-right">100.0%</td>
        </tr>
      </tfoot>
    </table>
  );
}

function InventoryValuation({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">SKU</th>
          <th className="text-left">Description</th>
          <th className="text-left">UoM</th>
          <th className="text-right">On Hand</th>
          <th className="text-right">Unit Cost</th>
          <th className="text-right">Total Value</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5 font-mono text-xs">{l.sku}</td>
            <td>{l.description}</td>
            <td className="text-gray-500">{l.uom}</td>
            <td className="text-right">{Number(l.on_hand).toLocaleString()}</td>
            <td className="text-right">{num(l.unit_cost)}</td>
            <td className="text-right font-medium">{num(l.total_value)}</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={6} className="py-4 text-center text-gray-400">No stock items</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={5}>Total ({c.item_count} item{c.item_count === 1 ? '' : 's'})</td>
          <td className="text-right">{num(c.total_value)}</td>
        </tr>
      </tfoot>
    </table>
  );
}

function FixedAssetRegister({ c }: { c: any }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-xs text-gray-500 uppercase border-b">
          <th className="text-left py-2">Asset</th>
          <th className="text-left">Category</th>
          <th className="text-left">Acquired</th>
          <th className="text-right">Cost</th>
          <th className="text-right">Accum. Depr.</th>
          <th className="text-right">Net Book Value</th>
        </tr>
      </thead>
      <tbody>
        {c.lines.map((l: any, i: number) => (
          <tr key={i} className="border-b border-gray-50">
            <td className="py-1.5">{l.description} <span className="text-xs text-gray-400">{l.asset_number}</span></td>
            <td className="text-gray-500">{l.category}</td>
            <td>{l.acquisition_date}</td>
            <td className="text-right">{num(l.cost)}</td>
            <td className="text-right text-gray-500">{num(l.accumulated_depreciation)}</td>
            <td className="text-right font-medium">{num(l.net_book_value)}</td>
          </tr>
        ))}
        {c.lines.length === 0 && <tr><td colSpan={6} className="py-4 text-center text-gray-400">No assets on register</td></tr>}
      </tbody>
      <tfoot>
        <tr className="font-bold border-t-2">
          <td className="py-2" colSpan={3}>Total</td>
          <td className="text-right">{num(c.total_cost)}</td>
          <td className="text-right">{num(c.total_accumulated_depreciation)}</td>
          <td className="text-right">{num(c.total_net_book_value)}</td>
        </tr>
      </tfoot>
    </table>
  );
}

function BankReconSummary({ c }: { c: any }) {
  return (
    <div className="space-y-6">
      {c.accounts.map((a: any) => (
        <div key={a.bank_account_id} className="report-section">
          <div className="flex items-center justify-between mb-2">
            <div>
              <p className="font-semibold text-gray-900">{a.account_name} <span className="text-xs font-normal text-gray-400">· {a.bank_name} · GL {a.gl_account}</span></p>
            </div>
            {a.is_reconciled ? (
              <span className="inline-flex items-center gap-1 text-xs font-medium text-green-700 bg-green-50 px-2 py-1 rounded">
                <CheckCircle2 className="w-3.5 h-3.5" /> Reconciled
              </span>
            ) : (
              <span className="inline-flex items-center gap-1 text-xs font-medium text-amber-700 bg-amber-50 px-2 py-1 rounded">
                <AlertTriangle className="w-3.5 h-3.5" /> Unreconciled
              </span>
            )}
          </div>
          <table className="w-full max-w-xl text-sm">
            <tbody>
              <tr className="border-b border-gray-50"><td className="py-1.5">Balance per bank statement</td><td className="text-right">{num(a.statement_balance)}</td></tr>
              <tr className="border-b border-gray-50"><td className="py-1.5">Balance per general ledger</td><td className="text-right">{num(a.gl_balance)}</td></tr>
              <tr className="border-b border-gray-50"><td className="py-1.5 text-gray-500">Difference</td><td className="text-right text-gray-500">{num(a.difference)}</td></tr>
              <tr className="border-b border-gray-50"><td className="py-1.5">Unreconciled feed items ({a.unmatched_count})</td><td className="text-right">{num(a.unreconciled_amount)}</td></tr>
              <tr className="border-b border-gray-50"><td className="py-1.5 text-gray-500">Matched feed items</td><td className="text-right text-gray-500">{a.matched_count}</td></tr>
            </tbody>
          </table>
        </div>
      ))}
      {c.accounts.length === 0 && <p className="py-4 text-center text-gray-400">No bank accounts</p>}
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

/* ------------------------------------------------------------------ */
/* Export helpers                                                     */
/* ------------------------------------------------------------------ */

function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

// Serialize the rendered statement (tables) to an Excel-readable .xls workbook.
// Excel opens HTML tables natively, so this preserves layout with zero deps.
function exportDomAsExcel(title: string) {
  const node = document.getElementById('report-document');
  if (!node) return;
  const tables = Array.from(node.querySelectorAll('table'));
  const body = tables.map((t) => t.outerHTML).join('<br/>');
  const html =
    `<html xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel" xmlns="http://www.w3.org/TR/REC-html40">` +
    `<head><meta charset="utf-8"><style>td,th{border:1px solid #ddd;padding:4px 8px;}</style></head>` +
    `<body><h3>${title}</h3>${body}</body></html>`;
  downloadBlob(new Blob([html], { type: 'application/vnd.ms-excel' }), `${title.replace(/\s+/g, '-')}-${today}.xls`);
}
