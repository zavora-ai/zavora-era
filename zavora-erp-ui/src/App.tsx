import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AppShell from './components/layout/AppShell';
import DashboardPage from './pages/dashboard/DashboardPage';
import InvoicesPage from './pages/invoicing/InvoicesPage';
import EstimatesPage from './pages/invoicing/EstimatesPage';
import BillsPage from './pages/bills/BillsPage';
import PaymentsPage from './pages/payments/PaymentsPage';
import CustomersPage from './pages/customers/CustomersPage';
import VendorsPage from './pages/vendors/VendorsPage';
import ProductsPage from './pages/products/ProductsPage';
import BankingPage from './pages/banking/BankingPage';
import TransactionsPage from './pages/banking/TransactionsPage';
import PayrollPage from './pages/payroll/PayrollPage';
import EmployeesPage from './pages/payroll/EmployeesPage';
import AccountsPage from './pages/accounts/AccountsPage';
import JournalEntriesPage from './pages/accounts/JournalEntriesPage';
import ReportsPage from './pages/reports/ReportsPage';
import SettingsPage from './pages/settings/SettingsPage';
import InventoryPage from './pages/inventory/InventoryPage';
import AssetsPage from './pages/assets/AssetsPage';
import FxRatesPage from './pages/settings/FxRatesPage';
import AuditPage from './pages/settings/AuditPage';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30000,
      retry: 1,
    },
  },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route index element={<DashboardPage />} />
            <Route path="invoices" element={<InvoicesPage />} />
            <Route path="estimates" element={<EstimatesPage />} />
            <Route path="bills" element={<BillsPage />} />
            <Route path="payments" element={<PaymentsPage />} />
            <Route path="customers" element={<CustomersPage />} />
            <Route path="vendors" element={<VendorsPage />} />
            <Route path="products" element={<ProductsPage />} />
            <Route path="banking" element={<BankingPage />} />
            <Route path="transactions" element={<TransactionsPage />} />
            <Route path="payroll" element={<PayrollPage />} />
            <Route path="employees" element={<EmployeesPage />} />
            <Route path="accounts" element={<AccountsPage />} />
            <Route path="journal-entries" element={<JournalEntriesPage />} />
            <Route path="reports" element={<ReportsPage />} />
            <Route path="settings" element={<SettingsPage />} />
            <Route path="inventory" element={<InventoryPage />} />
            <Route path="assets" element={<AssetsPage />} />
            <Route path="fx-rates" element={<FxRatesPage />} />
            <Route path="audit" element={<AuditPage />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
