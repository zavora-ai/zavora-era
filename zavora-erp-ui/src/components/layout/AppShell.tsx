import { Outlet } from 'react-router-dom';
import Sidebar from './Sidebar';
import Header from './Header';
import CommandPalette from './CommandPalette';
import SupportSessionBanner from './SupportSessionBanner';

export default function AppShell() {
  return (
    <div className="min-h-screen bg-gray-50">
      <SupportSessionBanner />
      <Sidebar />
      <div className="pl-[260px] print:pl-0">
        <Header />
        <main className="p-6 print:p-0">
          <Outlet />
        </main>
      </div>
      <CommandPalette />
    </div>
  );
}
