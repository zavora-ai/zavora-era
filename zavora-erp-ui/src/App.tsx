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
import AccountsPage from './pages/accounts/AccountsPage';
import ReportsPage from './pages/reports/ReportsPage';
import SettingsPage from './pages/settings/SettingsPage';

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
            <Route path="accounts" element={<AccountsPage />} />
            <Route path="reports" element={<ReportsPage />} />
            <Route path="settings" element={<SettingsPage />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
