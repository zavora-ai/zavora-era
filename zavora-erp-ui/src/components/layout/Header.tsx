import { Bell, Search } from 'lucide-react';
import UserMenu from './UserMenu';

export default function Header() {
  return (
    <header className="sticky top-0 z-40 h-14 bg-white/80 backdrop-blur-md border-b border-gray-100 flex items-center justify-between px-6">
      {/* Search */}
      <div className="flex-1 max-w-md">
        <div className="relative group">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-300 group-focus-within:text-indigo-500 transition-colors" />
          <input
            type="text"
            placeholder="Search anything..."
            className="w-full rounded-lg bg-gray-50 border-0 pl-9 pr-4 py-2 text-sm text-gray-700 placeholder:text-gray-400 focus:bg-white focus:ring-2 focus:ring-indigo-100 focus:outline-none transition-all"
          />
          <kbd className="absolute right-3 top-1/2 -translate-y-1/2 hidden sm:inline-flex items-center gap-0.5 text-[10px] text-gray-400 font-mono bg-gray-100 px-1.5 py-0.5 rounded">
            ⌘K
          </kbd>
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1.5">
        <button className="relative p-2 text-gray-400 hover:text-gray-600 rounded-lg hover:bg-gray-50 transition-colors">
          <Bell className="w-[18px] h-[18px]" />
          <span className="absolute top-1.5 right-1.5 w-1.5 h-1.5 bg-red-500 rounded-full ring-2 ring-white" />
        </button>
        <div className="w-px h-6 bg-gray-100 mx-1" />
        <UserMenu />
      </div>
    </header>
  );
}
