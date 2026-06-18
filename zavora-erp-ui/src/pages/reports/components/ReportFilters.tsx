// Controls bar driven by a report's `controls` array, plus the
// Generate / Print / Excel / CSV actions. Extracted faithfully from the monolith.
import { FileDown, FileSpreadsheet, Printer } from 'lucide-react';
import type { ReportMeta, ReportParams } from '../lib/reportTypes';
import { exportDomAsExcel } from '../lib/exportHelpers';

interface Props {
  meta: ReportMeta;
  params: ReportParams;
  setAsAt: (v: string) => void;
  setFrom: (v: string) => void;
  setTo: (v: string) => void;
  setAccount: (v: string) => void;
  setPartyId: (v: string) => void;
  setCompare: (v: boolean) => void;
  setDimensionType: (v: string) => void;
  parties: { id: string; name: string }[];
  dimensionTypes: { code: string; name: string }[];
  result: any;
  onGenerate: () => void;
  isPending: boolean;
  onExportCsv: () => void;
  csvPending: boolean;
}

export default function ReportFilters({
  meta, params, setAsAt, setFrom, setTo, setAccount, setPartyId, setCompare, setDimensionType,
  parties, dimensionTypes, result, onGenerate, isPending, onExportCsv, csvPending,
}: Props) {
  const needsParty = meta.controls.includes('party');
  const needsDimension = meta.controls.includes('dimension');
  const exportExcel = () => exportDomAsExcel(result?.title || meta.name);

  return (
    <div className="card p-4 mb-5 flex flex-wrap items-end gap-4">
      {needsParty && (
        <div>
          <label className="label">{meta.party === 'vendor' ? 'Vendor' : 'Customer'}</label>
          <select className="input min-w-[12rem]" value={params.partyId} onChange={(e) => setPartyId(e.target.value)}>
            <option value="">Select {meta.party}…</option>
            {parties.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
          </select>
        </div>
      )}
      {needsDimension && (
        <div>
          <label className="label">Dimension</label>
          <select className="input min-w-[12rem]" value={params.dimensionType} onChange={(e) => setDimensionType(e.target.value)}>
            <option value="">Select dimension…</option>
            {dimensionTypes.map((d) => <option key={d.code} value={d.code}>{d.name}</option>)}
          </select>
        </div>
      )}
      {meta.controls.includes('asAt') && (
        <div>
          <label className="label">As at</label>
          <input type="date" className="input" value={params.asAt} onChange={(e) => setAsAt(e.target.value)} />
        </div>
      )}
      {meta.controls.includes('period') && (
        <>
          <div><label className="label">From</label><input type="date" className="input" value={params.from} onChange={(e) => setFrom(e.target.value)} /></div>
          <div><label className="label">To</label><input type="date" className="input" value={params.to} onChange={(e) => setTo(e.target.value)} /></div>
        </>
      )}
      {meta.controls.includes('account') && (
        <div><label className="label">Account code</label><input className="input w-32" value={params.account} onChange={(e) => setAccount(e.target.value)} placeholder="1200" /></div>
      )}
      {meta.comparable && (
        <label className="flex items-center gap-2 text-sm text-gray-600 cursor-pointer pb-2">
          <input type="checkbox" checked={params.compare} onChange={(e) => setCompare(e.target.checked)} className="rounded" />
          Compare to prior year
        </label>
      )}
      <div className="flex-1" />
      <button onClick={onGenerate} className="btn-primary" disabled={isPending || (needsParty && !params.partyId) || (needsDimension && !params.dimensionType)}>
        {isPending ? 'Generating…' : 'Generate'}
      </button>
      <button onClick={() => window.print()} className="btn-secondary" disabled={!result} title="Print / save as PDF">
        <Printer className="w-4 h-4" /> Print
      </button>
      <button onClick={exportExcel} className="btn-secondary" disabled={!result} title="Export to Excel">
        <FileSpreadsheet className="w-4 h-4" /> Excel
      </button>
      <button onClick={onExportCsv} className="btn-secondary" disabled={csvPending} title="Export to CSV">
        <FileDown className="w-4 h-4" /> CSV
      </button>
    </div>
  );
}
