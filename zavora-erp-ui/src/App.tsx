import { useEffect, useState } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { bootstrapAuth, getAccessToken } from './api/client';
import AppShell from './components/layout/AppShell';
import LoginPage from './pages/auth/LoginPage';
import DashboardPage from './pages/dashboard/DashboardPage';
import InvoicesPage from './pages/invoicing/InvoicesPage';
import InvoiceDetailPage from './pages/invoicing/InvoiceDetailPage';
import EstimatesPage from './pages/invoicing/EstimatesPage';
import RecurringInvoicesPage from './pages/invoicing/RecurringInvoicesPage';
import BillsPage from './pages/bills/BillsPage';
import SupplierCreditNotesPage from './pages/bills/SupplierCreditNotesPage';
import PaymentsPage from './pages/payments/PaymentsPage';
import CustomersPage from './pages/customers/CustomersPage';
import CustomerDetailPage from './pages/customers/CustomerDetailPage';
import VendorsPage from './pages/vendors/VendorsPage';
import VendorDetailPage from './pages/vendors/VendorDetailPage';
import ProductsPage from './pages/products/ProductsPage';
import BankingPage from './pages/banking/BankingPage';
import TransactionsPage from './pages/banking/TransactionsPage';
import PayrollPage from './pages/payroll/PayrollPage';
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
import TaxFilingsPage from './pages/settings/TaxFilingsPage';
import OpeningBalancesPage from './pages/settings/OpeningBalancesPage';
import ImportPage from './pages/settings/ImportPage';
import ReconciliationPage from './pages/banking/ReconciliationPage';
import PeriodsPage from './pages/settings/PeriodsPage';
import ReportsPage from './pages/reports/ReportsPage';
import ReportPage from './pages/reports/ReportPage';
import SettingsPage from './pages/settings/SettingsPage';
import UsersPage from './pages/settings/UsersPage';
import InventoryPage from './pages/inventory/InventoryPage';
import AssetsPage from './pages/assets/AssetsPage';
import FxRatesPage from './pages/settings/FxRatesPage';
import AuditPage from './pages/settings/AuditPage';
import ReceiptCapturePage from './pages/receipts/ReceiptCapturePage';
import InvoicePreview from './pages/documents/InvoicePreview';
import EstimatePreview from './pages/documents/EstimatePreview';
import CreditNotePreview from './pages/documents/CreditNotePreview';
import BillPreview from './pages/documents/BillPreview';
import PaymentReceiptPreview from './pages/documents/PaymentReceiptPreview';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30000,
      retry: 1,
    },
  },
});

function RequireAuth({ children }: { children: React.ReactNode }) {
  if (!getAccessToken()) {
    return <Navigate to="/login" replace />;
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
      <BrowserRouter>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route
            element={
              <RequireAuth>
                <AppShell />
              </RequireAuth>
            }
          >
            <Route index element={<DashboardPage />} />
            <Route path="invoices" element={<InvoicesPage />} />
            <Route path="invoices/:id" element={<InvoiceDetailPage />} />
            <Route path="estimates" element={<EstimatesPage />} />
            <Route path="recurring-invoices" element={<RecurringInvoicesPage />} />
            <Route path="bills" element={<BillsPage />} />
            <Route path="supplier-credit-notes" element={<SupplierCreditNotesPage />} />
            <Route path="receipts/capture" element={<ReceiptCapturePage />} />
            <Route path="payments" element={<PaymentsPage />} />
            <Route path="customers" element={<CustomersPage />} />
            <Route path="customers/:id" element={<CustomerDetailPage />} />
            <Route path="vendors" element={<VendorsPage />} />
            <Route path="vendors/:id" element={<VendorDetailPage />} />
            <Route path="products" element={<ProductsPage />} />
            <Route path="banking" element={<BankingPage />} />
            <Route path="reconciliation" element={<ReconciliationPage />} />
            <Route path="transactions" element={<TransactionsPage />} />
            <Route path="payroll" element={<PayrollPage />} />
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
            <Route path="assets" element={<AssetsPage />} />
            <Route path="fx-rates" element={<FxRatesPage />} />
            <Route path="audit" element={<AuditPage />} />
            <Route path="documents/invoice/:id" element={<InvoicePreview />} />
            <Route path="documents/estimate/:id" element={<EstimatePreview />} />
            <Route path="documents/credit-note/:id" element={<CreditNotePreview />} />
            <Route path="documents/bill/:id" element={<BillPreview />} />
            <Route path="documents/receipt/:id" element={<PaymentReceiptPreview />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
