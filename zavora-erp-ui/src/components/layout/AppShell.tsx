import { useState } from 'react';
import { Outlet } from 'react-router-dom';
import Sidebar from './Sidebar';
import Header from './Header';
import CommandPalette from './CommandPalette';
import SupportSessionBanner from './SupportSessionBanner';

export default function AppShell() {
  // Mobile off-canvas drawer state. On lg+ the sidebar is always visible and
  // this flag is ignored (Sidebar pins itself with `lg:translate-x-0`).
  const [sidebarOpen, setSidebarOpen] = useState(false);

  return (
    <div className="min-h-screen bg-gray-50">
      <SupportSessionBanner />
      <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />
      <div className="lg:pl-[260px] print:pl-0">
        <Header onMenuClick={() => setSidebarOpen(true)} />
        <main className="p-4 sm:p-6 print:p-0">
          <Outlet />
        </main>
      </div>
      <CommandPalette />
    </div>
  );
}
