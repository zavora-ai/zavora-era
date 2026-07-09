import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Search } from 'lucide-react';
import { navigation } from './Sidebar';

// Flatten the sidebar navigation into a searchable command list, tagging each
// destination with its section so results read "Invoices · Sales". This is the
// working ⌘K palette behind the previously decorative header search.
interface Cmd {
  name: string;
  href: string;
  section: string;
  Icon: any;
}

function buildCommands(): Cmd[] {
  const cmds: Cmd[] = [];
  let section = '';
  for (const item of navigation as any[]) {
    if (item.divider) {
      section = item.label ?? '';
      continue;
    }
    cmds.push({ name: item.name, href: item.href, section, Icon: item.icon });
  }
  // Amos isn't in the nav array (it's a standalone pinned link) — add it.
  cmds.unshift({ name: 'Amos — AI Accountant', href: '/amos', section: 'AI', Icon: Search });
  return cmds;
}

/** Fires when the header search (or ⌘K) wants to open the palette. */
export const OPEN_COMMAND_PALETTE = 'zavora:open-command-palette';

export default function CommandPalette() {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const commands = useMemo(buildCommands, []);

  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands.slice(0, 8);
    return commands
      .filter((c) => c.name.toLowerCase().includes(q) || c.section.toLowerCase().includes(q))
      .slice(0, 12);
  }, [query, commands]);

  // Open on ⌘K / Ctrl+K, or when the header search dispatches the event.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOpen((o) => !o);
      }
      if (e.key === 'Escape') setOpen(false);
    };
    const onOpen = () => setOpen(true);
    window.addEventListener('keydown', onKey);
    window.addEventListener(OPEN_COMMAND_PALETTE, onOpen);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener(OPEN_COMMAND_PALETTE, onOpen);
    };
  }, []);

  useEffect(() => {
    if (open) {
      setQuery('');
      setActive(0);
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  useEffect(() => setActive(0), [query]);

  if (!open) return null;

  const go = (href: string) => {
    setOpen(false);
    navigate(href);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, results.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === 'Enter' && results[active]) {
      e.preventDefault();
      go(results[active].href);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[100] flex items-start justify-center bg-black/30 backdrop-blur-sm pt-[12vh] px-4"
      onClick={() => setOpen(false)}
    >
      <div
        className="w-full max-w-lg bg-white rounded-xl shadow-2xl border border-gray-100 overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 px-4 border-b border-gray-100">
          <Search className="w-4 h-4 text-gray-400 shrink-0" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="Go to… (type a page name)"
            className="flex-1 py-3.5 text-sm text-gray-800 placeholder:text-gray-400 focus:outline-none"
          />
          <kbd className="text-[10px] text-gray-400 font-mono bg-gray-100 px-1.5 py-0.5 rounded">esc</kbd>
        </div>
        <ul className="max-h-80 overflow-y-auto py-2">
          {results.length === 0 && (
            <li className="px-4 py-6 text-center text-sm text-gray-400">No matching pages</li>
          )}
          {results.map((c, i) => {
            const Icon = c.Icon;
            return (
              <li key={c.href}>
                <button
                  onMouseEnter={() => setActive(i)}
                  onClick={() => go(c.href)}
                  className={`w-full flex items-center gap-3 px-4 py-2 text-left text-sm ${
                    i === active ? 'bg-indigo-50 text-indigo-700' : 'text-gray-700'
                  }`}
                >
                  {Icon && <Icon className="w-4 h-4 shrink-0 opacity-70" />}
                  <span className="flex-1">{c.name}</span>
                  {c.section && <span className="text-[11px] text-gray-400">{c.section}</span>}
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </div>
  );
}
