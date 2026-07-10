import { useEffect, useState } from 'react';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { bootstrapAuth, getAccessToken } from './api/client';
import AppShell from './components/layout/AppShell';
import PublicInvoicePage from './pages/pay/PublicInvoicePage';
import { ToastProvider } from './components/toast/ToastProvider';
import LoginPage from './pages/auth/LoginPage';
import SetPasswordPage from './pages/auth/SetPasswordPage';
import ForgotPasswordPage from './pages/auth/ForgotPasswordPage';
import LandingPage from './pages/marketing/LandingPage';
import InfoPage from './pages/marketing/InfoPage';
import AmosAiPage from './pages/marketing/AmosAiPage';
import PortalsPage from './pages/marketing/PortalsPage';
import PlatformLoginPage from './pages/platform/PlatformLoginPage';
import PlatformTenantsPage from './pages/platform/PlatformTenantsPage';
import DashboardPage from './pages/dashboard/DashboardPage';
import AmosPage from './pages/amos/AmosPage';
import InvoicesPage from './pages/invoicing/InvoicesPage';
import InvoiceDetailPage from './pages/invoicing/InvoiceDetailPage';
import EstimatesPage from './pages/invoicing/EstimatesPage';
import RecurringInvoicesPage from './pages/invoicing/RecurringInvoicesPage';
import BillsPage from './pages/bills/BillsPage';
import SupplierCreditNotesPage from './pages/bills/SupplierCreditNotesPage';
import RequisitionsPage from './pages/procurement/RequisitionsPage';
import TendersPage from './pages/procurement/TendersPage';
import PurchaseOrdersPage from './pages/procurement/PurchaseOrdersPage';
import ProcurementAnalyticsPage from './pages/procurement/ProcurementAnalyticsPage';
import DebitNotesPage from './pages/procurement/DebitNotesPage';
import ExpenseClaimsPage from './pages/procurement/ExpenseClaimsPage';
import ApprovalLimitsPage from './pages/settings/ApprovalLimitsPage';
import RolesPage from './pages/settings/RolesPage';
import CrmPage from './pages/crm/CrmPage';
import VendorApplicationsPage from './pages/procurement/VendorApplicationsPage';
import PortalShell from './pages/portal/PortalShell';
import VendorLoginPage from './pages/portal/VendorLoginPage';
import VendorRegisterPage from './pages/portal/VendorRegisterPage';
import PortalTendersPage from './pages/portal/PortalTendersPage';
import PortalBidsPage from './pages/portal/PortalBidsPage';
import PortalPurchaseOrdersPage from './pages/portal/PortalPurchaseOrdersPage';
import PortalStatementPage from './pages/portal/PortalStatementPage';
import PaymentsPage from './pages/payments/PaymentsPage';
import CustomersPage from './pages/customers/CustomersPage';
import CustomerDetailPage from './pages/customers/CustomerDetailPage';
import VendorsPage from './pages/vendors/VendorsPage';
import VendorDetailPage from './pages/vendors/VendorDetailPage';
import ProductsPage from './pages/products/ProductsPage';
import BankingPage from './pages/banking/BankingPage';
import TransactionsPage from './pages/banking/TransactionsPage';
import PayrollPage from './pages/payroll/PayrollPage';
import PayrollSettingsPage from './pages/payroll/PayrollSettingsPage';
import PayrollReportsPage from './pages/payroll/PayrollReportsPage';
import LeavePage from './pages/leave/LeavePage';
import OnboardingPage from './pages/hr/OnboardingPage';
import StaffLoginPage from './pages/staff/StaffLoginPage';
import StaffSetPasswordPage from './pages/staff/StaffSetPasswordPage';
import CustomerLoginPage from './pages/customerportal/CustomerLoginPage';
import CustomerSetPasswordPage from './pages/customerportal/CustomerSetPasswordPage';
import CustomerPortal from './pages/customerportal/CustomerPortal';
import StaffPortal from './pages/staff/StaffPortal';
import EmployeesPage from './pages/payroll/EmployeesPage';
import AccountsPage from './pages/accounts/AccountsPage';
import JournalEntriesPage from './pages/accounts/JournalEntriesPage';
import JournalEntryDetailPage from './pages/accounts/JournalEntryDetailPage';
import RecurringJournalsPage from './pages/accounts/RecurringJournalsPage';
import BudgetsPage from './pages/budgets/BudgetsPage';
import DimensionsPage from './pages/dimensions/DimensionsPage';
import CustomReportsPage from './pages/reports/CustomReportsPage';
import ReportSchedulesPage from './pages/reports/ReportSchedulesPage';
import ConsolidationPage from './pages/consolidation/ConsolidationPage';
import WhtRatesPage from './pages/settings/WhtRatesPage';
import EtimsPage from './pages/settings/EtimsPage';
import TaxFilingsPage from './pages/settings/TaxFilingsPage';
import OpeningBalancesPage from './pages/settings/OpeningBalancesPage';
import ImportPage from './pages/settings/ImportPage';
import ReconciliationPage from './pages/banking/ReconciliationPage';
import CashForecastPage from './pages/banking/CashForecastPage';
import PeriodsPage from './pages/settings/PeriodsPage';
import ReportsPage from './pages/reports/ReportsPage';
import ReportPage from './pages/reports/ReportPage';
import SettingsPage from './pages/settings/SettingsPage';
import UsersPage from './pages/settings/UsersPage';
import InventoryPage from './pages/inventory/InventoryPage';
import PosSellPage from './pages/pos/PosSellPage';
import PosSessionsPage from './pages/pos/PosSessionsPage';
import MobileStockPage from './pages/pos/MobileStockPage';
import AssetsPage from './pages/assets/AssetsPage';
import AmortizationPage from './pages/accounting/AmortizationPage';
import FxRatesPage from './pages/settings/FxRatesPage';
import AuditPage from './pages/settings/AuditPage';
import NotificationDeliveryPage from './pages/settings/NotificationDeliveryPage';
import ReceiptCapturePage from './pages/receipts/ReceiptCapturePage';
import InvoicePreview from './pages/documents/InvoicePreview';
import EstimatePreview from './pages/documents/EstimatePreview';
import CreditNotePreview from './pages/documents/CreditNotePreview';
import BillPreview from './pages/documents/BillPreview';
import PaymentReceiptPreview from './pages/documents/PaymentReceiptPreview';
import RecurringPreview from './pages/documents/RecurringPreview';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30000,
      retry: 1,
    },
  },
});

function RequireAuth({ children }: { children: React.ReactNode }) {
  // Unauthenticated visitors get the public marketing site (not a bare login
  // form). The landing page's CTAs route to /login for sign-in / sign-up.
  if (!getAccessToken()) {
    return <LandingPage />;
  }
  return <>{children}</>;
}

export default function App() {
  // Restore the session from the httpOnly refresh cookie before routing, so a
  // hard refresh on a deep link doesn't bounce an authenticated user to /login.
  const [booting, setBooting] = useState(true);
  useEffect(() => {
    bootstrapAuth().finally(() => setBooting(false));
  }, []);

  if (booting) {
    return (
      <div className="min-h-screen flex items-center justify-center text-gray-500">
        Loading…
      </div>
    );
  }

  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          {/* Public invoice pay-link (no auth; shareable token). */}
          <Route path="/pay/:token" element={<PublicInvoicePage />} />
          <Route path="/set-password" element={<SetPasswordPage />} />
          <Route path="/forgot-password" element={<ForgotPasswordPage />} />
          {/* Platform Super Admin (Zavora ops) — separate plane from tenant ERP */}
          <Route path="/platform/login" element={<PlatformLoginPage />} />
          <Route path="/platform" element={<PlatformTenantsPage />} />
          <Route path="/platform/*" element={<PlatformTenantsPage />} />
          {/* Public marketing sub-pages (footer links). */}
          <Route path="/amos-ai" element={<AmosAiPage />} />
          <Route path="/portals" element={<PortalsPage />} />
          <Route path="/about" element={<InfoPage />} />
          <Route path="/updates" element={<InfoPage />} />
          <Route path="/careers" element={<InfoPage />} />
          <Route path="/contact" element={<InfoPage />} />
          <Route path="/privacy" element={<InfoPage />} />
          <Route path="/terms" element={<InfoPage />} />
          <Route path="/security" element={<InfoPage />} />
          <Route path="/data-protection" element={<InfoPage />} />
          {/* Vendor portal — a fully separate surface with its own auth (see
              portalClient.ts + PortalShell). Public auth pages, then the gated shell. */}
          <Route path="/vendorportal/login" element={<VendorLoginPage />} />
          <Route path="/vendorportal/register" element={<VendorRegisterPage />} />
          <Route path="/vendorportal" element={<PortalShell />}>
            <Route index element={<PortalTendersPage />} />
            <Route path="bids" element={<PortalBidsPage />} />
            <Route path="purchase-orders" element={<PortalPurchaseOrdersPage />} />
            <Route path="statement" element={<PortalStatementPage />} />
          </Route>
          <Route path="/staff/login" element={<StaffLoginPage />} />
          <Route path="/staff/set-password" element={<StaffSetPasswordPage />} />
          <Route path="/staff" element={<StaffPortal />} />
          <Route path="/customerportal/login" element={<CustomerLoginPage />} />
          <Route path="/customerportal/register" element={<CustomerLoginPage />} />
          <Route path="/customerportal/set-password" element={<CustomerSetPasswordPage />} />
          <Route path="/customerportal" element={<CustomerPortal />} />
          <Route
            element={
              <RequireAuth>
                <AppShell />
              </RequireAuth>
            }
          >
            <Route index element={<DashboardPage />} />
            <Route path="amos" element={<AmosPage />} />
            <Route path="invoices" element={<InvoicesPage />} />
            <Route path="invoices/:id" element={<InvoiceDetailPage />} />
            <Route path="estimates" element={<EstimatesPage />} />
            <Route path="recurring-invoices" element={<RecurringInvoicesPage />} />
            <Route path="bills" element={<BillsPage />} />
            <Route path="supplier-credit-notes" element={<SupplierCreditNotesPage />} />
            <Route path="requisitions" element={<RequisitionsPage />} />
            <Route path="tenders" element={<TendersPage />} />
            <Route path="purchase-orders" element={<PurchaseOrdersPage />} />
            <Route path="vendor-applications" element={<VendorApplicationsPage />} />
            <Route path="procurement-analytics" element={<ProcurementAnalyticsPage />} />
            <Route path="debit-notes" element={<DebitNotesPage />} />
            <Route path="expense-claims" element={<ExpenseClaimsPage />} />
            <Route path="approval-limits" element={<ApprovalLimitsPage />} />
            <Route path="crm" element={<CrmPage />} />
            <Route path="roles-admin" element={<RolesPage />} />
            <Route path="receipts/capture" element={<ReceiptCapturePage />} />
            <Route path="payments" element={<PaymentsPage />} />
            <Route path="customers" element={<CustomersPage />} />
            <Route path="customers/:id" element={<CustomerDetailPage />} />
            <Route path="vendors" element={<VendorsPage />} />
            <Route path="vendors/:id" element={<VendorDetailPage />} />
            <Route path="products" element={<ProductsPage />} />
            <Route path="banking" element={<BankingPage />} />
            <Route path="cash-forecast" element={<CashForecastPage />} />
            <Route path="reconciliation" element={<ReconciliationPage />} />
            <Route path="transactions" element={<TransactionsPage />} />
            <Route path="payroll" element={<PayrollPage />} />
            <Route path="payroll-settings" element={<PayrollSettingsPage />} />
            <Route path="payroll-reports" element={<PayrollReportsPage />} />
            <Route path="leave" element={<LeavePage />} />
            <Route path="onboarding" element={<OnboardingPage />} />
            <Route path="employees" element={<EmployeesPage />} />
            <Route path="accounts" element={<AccountsPage />} />
            <Route path="journal-entries" element={<JournalEntriesPage />} />
            <Route path="journal-entries/:id" element={<JournalEntryDetailPage />} />
            <Route path="recurring-journals" element={<RecurringJournalsPage />} />
            <Route path="periods" element={<PeriodsPage />} />
            <Route path="budgets" element={<BudgetsPage />} />
            <Route path="dimensions" element={<DimensionsPage />} />
            <Route path="wht-rates" element={<WhtRatesPage />} />
            <Route path="tax-filings" element={<TaxFilingsPage />} />
            <Route path="etims" element={<EtimsPage />} />
            <Route path="opening-balances" element={<OpeningBalancesPage />} />
            <Route path="import" element={<ImportPage />} />
            <Route path="consolidation" element={<ConsolidationPage />} />
            <Route path="reports" element={<ReportsPage />} />
            <Route path="reports/custom" element={<CustomReportsPage />} />
            <Route path="reports/schedules" element={<ReportSchedulesPage />} />
            <Route path="reports/:slug" element={<ReportPage />} />
            <Route path="settings" element={<SettingsPage />} />
            <Route path="users" element={<UsersPage />} />
            <Route path="inventory" element={<InventoryPage />} />
            <Route path="pos" element={<PosSellPage />} />
            <Route path="pos/sessions" element={<PosSessionsPage />} />
            <Route path="pos/stock" element={<MobileStockPage />} />
            <Route path="assets" element={<AssetsPage />} />
            <Route path="amortization" element={<AmortizationPage />} />
            <Route path="fx-rates" element={<FxRatesPage />} />
            <Route path="audit" element={<AuditPage />} />
            <Route path="notifications" element={<NotificationDeliveryPage />} />
            <Route path="documents/invoice/:id" element={<InvoicePreview />} />
            <Route path="documents/estimate/:id" element={<EstimatePreview />} />
            <Route path="documents/credit-note/:id" element={<CreditNotePreview />} />
            <Route path="documents/bill/:id" element={<BillPreview />} />
            <Route path="documents/receipt/:id" element={<PaymentReceiptPreview />} />
            <Route path="documents/recurring/:id" element={<RecurringPreview />} />
          </Route>
        </Routes>
      </BrowserRouter>
      </ToastProvider>
    </QueryClientProvider>
  );
}
