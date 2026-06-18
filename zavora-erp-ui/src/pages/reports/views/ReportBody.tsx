// Resolves the report body view from the backend content key — exactly the
// dispatch the original ReportDocument performed. Several report types share a
// renderer (CustomerStatement/VendorStatement -> PartyStatement,
// IncomeByCustomer/ExpenseByVendor -> PartyRanking, SalesTaxSummary -> VatDetail,
// WhtCertificate -> WhtReport) and a few have no dedicated renderer
// (CashFlow, ArAgeing, ApAgeing) and fall back to a raw JSON dump.
import TrialBalanceView from './TrialBalanceView';
import BalanceSheetView from './BalanceSheetView';
import ProfitAndLossView from './ProfitAndLossView';
import VatReturnView from './VatReturnView';
import PartyStatementView from './PartyStatementView';
import PayrollSummaryView from './PayrollSummaryView';
import PayeP10View from './PayeP10View';
import WhtReportView from './WhtReportView';
import VatDetailView from './VatDetailView';
import PartyRankingView from './PartyRankingView';
import InventoryValuationView from './InventoryValuationView';
import FixedAssetRegisterView from './FixedAssetRegisterView';
import BankReconSummaryView from './BankReconSummaryView';
import GlDetailView from './GlDetailView';
import RawJsonView from './RawJsonView';

export default function ReportBody({ result, onDrill }: { result: any; onDrill?: (code: string) => void }) {
  const content = result?.content ?? {};
  const key = Object.keys(content)[0];
  const c = content[key];

  switch (key) {
    case 'TrialBalance':
      return <TrialBalanceView c={c} onDrill={onDrill} />;
    case 'BalanceSheet':
      return <BalanceSheetView c={c} onDrill={onDrill} />;
    case 'ProfitAndLoss':
      return <ProfitAndLossView c={c} onDrill={onDrill} />;
    case 'VatReturn':
      return <VatReturnView c={c} />;
    case 'PartyStatement':
      return <PartyStatementView c={c} />;
    case 'PayrollSummary':
      return <PayrollSummaryView c={c} />;
    case 'PayeP10':
      return <PayeP10View c={c} />;
    case 'WhtReport':
      return <WhtReportView c={c} />;
    case 'VatDetail':
      return <VatDetailView c={c} />;
    case 'PartyRanking':
      return <PartyRankingView c={c} />;
    case 'InventoryValuation':
      return <InventoryValuationView c={c} />;
    case 'FixedAssetRegister':
      return <FixedAssetRegisterView c={c} />;
    case 'BankReconSummary':
      return <BankReconSummaryView c={c} />;
    case 'GlDetail':
      return <GlDetailView c={c} />;
    default:
      return <RawJsonView c={c} />;
  }
}
