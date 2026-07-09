import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getCashForecast } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency, formatDate } from '../../utils/format';
import { AlertTriangle } from 'lucide-react';

/** 13-week rolling cash forecast — the deterministic, non-AI view built from
 * open AR/AP due dates, unremitted statutory filings and the payroll cycle.
 * (Amos's cash-forecast skill assembles the same picture conversationally.) */
export default function CashForecastPage() {
  const [weeks, setWeeks] = useState(13);
  const { data, isLoading, isError } = useQuery({
    queryKey: ['cash-forecast', weeks],
    queryFn: () => getCashForecast(weeks),
  });
  const f: any = data?.data;

  return (
    <div>
      <PageHeader title="Cash Forecast" subtitle="Rolling weekly outlook from open invoices, bills, statutory filings and payroll" />

      <div className="flex items-center gap-3 mb-4">
        <label className="text-sm text-slate-600">Horizon</label>
        <select className="input w-auto" value={weeks} onChange={(e) => setWeeks(Number(e.target.value))}>
          <option value={4}>4 weeks</option>
          <option value={13}>13 weeks</option>
          <option value={26}>26 weeks</option>
        </select>
        {f && <span className="text-sm text-slate-500">Opening cash: <strong className="text-slate-800">{formatCurrency(Number(f.opening_cash))}</strong> as of {formatDate(f.as_of)}</span>}
      </div>

      {isLoading && <div className="card p-6 text-sm text-slate-500">Building forecast…</div>}
      {isError && <div className="card p-6 text-sm text-red-600">Could not build the forecast.</div>}

      {f && (
        <>
          {f.first_negative_week && (
            <div className="mb-4 flex items-start gap-3 rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-800">
              <AlertTriangle className="w-5 h-5 shrink-0 mt-0.5" />
              <div>
                <strong>Cash goes negative in the week of {formatDate(f.first_negative_week)}.</strong>{' '}
                Review the payment run for that week, chase collectable receivables, or arrange cover before then.
              </div>
            </div>
          )}

          <div className="card overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-slate-500 border-b">
                  <th className="p-3">Week ending</th>
                  <th className="p-3 text-right">Inflows</th>
                  <th className="p-3 text-right">Outflows</th>
                  <th className="p-3 text-right">Net</th>
                  <th className="p-3 text-right">Closing</th>
                </tr>
              </thead>
              <tbody>
                {f.weeks.map((w: any) => {
                  const closing = Number(w.closing);
                  return (
                    <tr key={w.week_start} className={`border-b border-slate-100 ${closing < 0 ? 'bg-red-50' : ''}`}>
                      <td className="p-3">{formatDate(w.week_end)}</td>
                      <td className="p-3 text-right text-emerald-700">{formatCurrency(Number(w.inflows))}</td>
                      <td className="p-3 text-right text-slate-700">{formatCurrency(Number(w.outflows))}</td>
                      <td className={`p-3 text-right ${Number(w.net) < 0 ? 'text-red-600' : 'text-emerald-700'}`}>{formatCurrency(Number(w.net))}</td>
                      <td className={`p-3 text-right font-semibold ${closing < 0 ? 'text-red-700' : 'text-slate-900'}`}>{formatCurrency(closing)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          <div className="mt-4 grid gap-3 md:grid-cols-2">
            {Number(f.excluded_overdue_ar) > 0 && (
              <div className="card p-4 text-sm text-slate-600">
                <strong className="text-slate-800">{formatCurrency(Number(f.excluded_overdue_ar))}</strong> of receivables overdue
                more than 90 days is <em>excluded</em> from this forecast — collect it and the picture improves.
              </div>
            )}
            <div className="card p-4 text-xs text-slate-500">
              <div className="font-semibold text-slate-600 mb-1">Assumptions</div>
              <ul className="list-disc pl-4 space-y-0.5">
                {f.assumptions.map((a: string) => <li key={a}>{a}</li>)}
              </ul>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
