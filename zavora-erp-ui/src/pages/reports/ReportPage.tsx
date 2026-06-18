import { useEffect, useRef, useState } from 'react';
import { Navigate, useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { getSettings, getCustomers, getVendors } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { AlertTriangle, ArrowLeft } from 'lucide-react';
import { Link } from 'react-router-dom';
import { keyForSlug, metaForKey, slugFor, type ReportMeta, type ReportParams } from './lib/reportTypes';
import { useReport } from './hooks/useReport';
import ReportLayout from './components/ReportLayout';
import ReportFilters from './components/ReportFilters';
import ReportBody from './views/ReportBody';

const today = new Date().toISOString().split('T')[0];
const yearStart = `${new Date().getFullYear()}-01-01`;

// Route entry: resolves the report key from the slug, redirects unknown slugs to
// the launcher, and remounts the inner view per slug + query string so deep-link
// prefill is applied cleanly.
export default function ReportPage() {
  const { slug } = useParams();
  const [searchParams] = useSearchParams();
  const key = slug ? keyForSlug(slug) : undefined;
  const meta = key ? metaForKey(key) : undefined;

  if (!meta) return <Navigate to="/reports" replace />;

  return <ReportView key={(slug ?? '') + '?' + searchParams.toString()} meta={meta} />;
}

function ReportView({ meta }: { meta: ReportMeta }) {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();

  const qAccount = searchParams.get('account');
  const qFrom = searchParams.get('from');
  const qTo = searchParams.get('to');

  const [asAt, setAsAt] = useState(today);
  const [from, setFrom] = useState(qFrom ?? yearStart);
  const [to, setTo] = useState(qTo ?? today);
  const [account, setAccount] = useState(qAccount ?? '1200');
  const [partyId, setPartyId] = useState('');
  const [compare, setCompare] = useState(false);

  const params: ReportParams = { asAt, from, to, account, partyId, compare };
  const { result, generate, csvExport } = useReport(meta, params);

  const { data: settingsRes } = useQuery({ queryKey: ['settings'], queryFn: getSettings });
  const branding = settingsRes?.data?.branding;

  const { data: customersRes } = useQuery({ queryKey: ['customers'], queryFn: getCustomers });
  const { data: vendorsRes } = useQuery({ queryKey: ['vendors'], queryFn: getVendors });
  const parties: { id: string; name: string }[] =
    (meta.party === 'vendor' ? vendorsRes?.data : customersRes?.data) ?? [];

  // Deep-link prefill: when account/from/to are present, generate on load so a
  // drill-down link renders its report immediately.
  const didAuto = useRef(false);
  useEffect(() => {
    if (didAuto.current) return;
    if (qAccount || qFrom || qTo) {
      didAuto.current = true;
      generate.mutate();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Drill from an account on a statement into its General Ledger detail,
  // carrying the statement's period (or year-to-date for an as-at report).
  const onDrill = (accountCode: string) => {
    const content = result?.content ?? {};
    const cc = content[Object.keys(content)[0]] ?? {};
    let f = from, t = to;
    if (cc.as_at) { f = `${new Date(cc.as_at).getFullYear()}-01-01`; t = cc.as_at; }
    else if (cc.period_from && cc.period_to) { f = cc.period_from; t = cc.period_to; }
    navigate(`/reports/${slugFor('GlDetail')}?account=${encodeURIComponent(accountCode)}&from=${f}&to=${t}`);
  };

  return (
    <div>
      <div className="no-print">
        <PageHeader
          title={meta.name}
          subtitle={meta.desc}
          actions={
            <Link to="/reports" className="btn-secondary">
              <ArrowLeft className="w-4 h-4" /> All reports
            </Link>
          }
        />

        <ReportFilters
          meta={meta}
          params={params}
          setAsAt={setAsAt}
          setFrom={setFrom}
          setTo={setTo}
          setAccount={setAccount}
          setPartyId={setPartyId}
          setCompare={setCompare}
          parties={parties}
          result={result}
          onGenerate={() => generate.mutate()}
          isPending={generate.isPending}
          onExportCsv={() => csvExport.mutate()}
          csvPending={csvExport.isPending}
        />

        {generate.isError && (
          <div className="card p-4 mb-5 flex items-center justify-between gap-2 text-sm text-red-700 bg-red-50 border-red-200">
            <span className="flex items-center gap-2">
              <AlertTriangle className="w-4 h-4" /> Could not generate this report. Check the dates and try again.
            </span>
            <button onClick={() => generate.mutate()} className="btn-secondary" disabled={generate.isPending}>Retry</button>
          </div>
        )}

        {generate.isPending && (
          <div className="card p-12 mb-5 text-center">
            <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
            <p className="mt-3 text-sm text-gray-500">Generating report…</p>
          </div>
        )}

        {!result && !generate.isPending && !generate.isError && (
          <div className="card px-6 py-12 text-center text-sm text-gray-500">
            Generate to view this report.
          </div>
        )}
      </div>

      {result && (
        <ReportLayout result={result} branding={branding}>
          <ReportBody result={result} onDrill={onDrill} />
        </ReportLayout>
      )}
    </div>
  );
}
