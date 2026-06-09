import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { generateReport } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { BarChart3, FileDown } from 'lucide-react';

const reportTypes = [
  { key: 'TrialBalance', name: 'Trial Balance', desc: 'Account balances at a point in time' },
  { key: 'BalanceSheet', name: 'Balance Sheet', desc: 'Assets, liabilities, and equity' },
  { key: 'ProfitAndLoss', name: 'Profit & Loss', desc: 'Revenue and expenses for a period' },
  { key: 'CashFlow', name: 'Cash Flow Statement', desc: 'Cash movements (indirect method)' },
  { key: 'ArAgeing', name: 'AR Ageing', desc: 'Customer balances by age bucket' },
  { key: 'ApAgeing', name: 'AP Ageing', desc: 'Vendor balances by age bucket' },
  { key: 'VatReturn', name: 'VAT Return', desc: 'iTax-ready VAT data export' },
  { key: 'GlDetail', name: 'General Ledger Detail', desc: 'Transaction detail by account' },
  { key: 'CustomerStatement', name: 'Customer Statement', desc: 'Activity and balance per customer' },
  { key: 'PayrollSummary', name: 'Payroll Summary', desc: 'PAYE, NSSF, SHA totals per period' },
  { key: 'PayeP10', name: 'PAYE P10 Schedule', desc: 'KRA monthly PAYE filing data' },
  { key: 'WhtCertificate', name: 'WHT Certificate (P10A)', desc: 'Withholding tax certificates' },
  { key: 'SalesTaxSummary', name: 'Sales Tax Summary', desc: 'VAT by document and rate' },
  { key: 'BankReconSummary', name: 'Bank Reconciliation', desc: 'Statement vs GL matching' },
  { key: 'CustomerPaymentHistory', name: 'Payment History', desc: 'Customer payment timeline' },
];

export default function ReportsPage() {
  const [result, setResult] = useState<any>(null);
  const mutation = useMutation({ mutationFn: (data: any) => generateReport(data), onSuccess: (res) => setResult(res.data) });

  const handleGenerate = (reportType: string) => {
    mutation.mutate({
      entity_id: '00000000-0000-0000-0000-000000000001',
      report_type: reportType,
      parameters: {},
    });
  };

  return (
    <div>
      <PageHeader title="Reports" subtitle="Financial and compliance reports" />

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
        {reportTypes.map((rt) => (
          <button
            key={rt.key}
            onClick={() => handleGenerate(rt.key)}
            className="card p-4 text-left hover:border-blue-300 hover:shadow-md transition-all"
          >
            <div className="flex items-start gap-3">
              <BarChart3 className="w-5 h-5 text-blue-600 mt-0.5 shrink-0" />
              <div>
                <p className="font-medium text-gray-900">{rt.name}</p>
                <p className="text-xs text-gray-500 mt-0.5">{rt.desc}</p>
              </div>
            </div>
          </button>
        ))}
      </div>

      {result && (
        <div className="card p-6">
          <div className="flex items-center justify-between mb-4">
            <h3 className="font-medium">{result.title || result.report_type}</h3>
            <button className="btn-secondary text-sm"><FileDown className="w-4 h-4" /> Export PDF</button>
          </div>
          <pre className="text-xs bg-gray-50 p-4 rounded-lg overflow-auto max-h-96">{JSON.stringify(result.content, null, 2)}</pre>
        </div>
      )}
    </div>
  );
}
