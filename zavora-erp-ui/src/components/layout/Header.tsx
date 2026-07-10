import { Menu, Search } from 'lucide-react';
import UserMenu from './UserMenu';
import NotificationInbox from './NotificationInbox';
import TenantSwitcher from './TenantSwitcher';
import { OPEN_COMMAND_PALETTE } from './CommandPalette';

export default function Header({ onMenuClick }: { onMenuClick: () => void }) {
  const openPalette = () => window.dispatchEvent(new Event(OPEN_COMMAND_PALETTE));
  return (
    <header className="sticky top-0 z-40 h-14 bg-white/80 backdrop-blur-md border-b border-gray-100 flex items-center justify-between px-4 lg:px-6 gap-3 lg:gap-4">
      {/* Menu (mobile) + tenant switcher + search */}
      <div className="flex items-center gap-2 sm:gap-3 flex-1 min-w-0">
        <button
          type="button"
          onClick={onMenuClick}
          className="lg:hidden -ml-1 p-1.5 rounded-lg text-gray-500 hover:bg-gray-100 hover:text-gray-700 transition-colors shrink-0"
          aria-label="Open navigation menu"
        >
          <Menu className="w-5 h-5" />
        </button>
        <TenantSwitcher />
        <div className="hidden sm:block w-px h-6 bg-gray-100" />
        <div className="hidden sm:block flex-1 max-w-md">
          <button
            type="button"
            onClick={openPalette}
            className="relative group w-full text-left"
            aria-label="Search and jump to a page (⌘K)"
          >
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-300 group-hover:text-indigo-500 transition-colors" />
            <span className="block w-full rounded-lg bg-gray-50 border-0 pl-9 pr-4 py-2 text-sm text-gray-400 group-hover:bg-white group-hover:ring-2 group-hover:ring-indigo-100 transition-all">
              Search anything…
            </span>
            <kbd className="absolute right-3 top-1/2 -translate-y-1/2 hidden sm:inline-flex items-center gap-0.5 text-[10px] text-gray-400 font-mono bg-gray-100 px-1.5 py-0.5 rounded">
              ⌘K
            </kbd>
          </button>
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1.5 shrink-0">
        {/* Search icon shortcut on mobile (palette has no ⌘K on touch) */}
        <button
          type="button"
          onClick={openPalette}
          className="sm:hidden p-1.5 rounded-lg text-gray-500 hover:bg-gray-100 hover:text-gray-700 transition-colors"
          aria-label="Search"
        >
          <Search className="w-5 h-5" />
        </button>
        <NotificationInbox />
        <div className="w-px h-6 bg-gray-100 mx-1" />
        <UserMenu />
      </div>
    </header>
  );
}
