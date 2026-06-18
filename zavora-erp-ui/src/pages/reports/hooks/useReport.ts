import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { generateReport, exportReport } from '../../../api/client';
import { buildReportRequest, type ReportMeta, type ReportParams } from '../lib/reportTypes';
import { downloadBlob } from '../lib/exportHelpers';

const today = new Date().toISOString().split('T')[0];

// Encapsulates the generate + CSV-export mutations and the latest result.
// Behaviour is identical to the original ReportsPage mutations.
export function useReport(meta: ReportMeta, params: ReportParams) {
  const [result, setResult] = useState<any>(null);

  const generate = useMutation({
    mutationFn: () => generateReport(buildReportRequest(meta, params)),
    onSuccess: (res) => setResult(res.data),
  });

  const csvExport = useMutation({
    mutationFn: () => exportReport(buildReportRequest(meta, params)),
    onSuccess: (res) => downloadBlob(new Blob([res.data], { type: 'text/csv' }), `${meta.key}-${today}.csv`),
  });

  return { result, setResult, generate, csvExport };
}
