import { type ReactNode, useEffect } from 'react';
import { X } from 'lucide-react';

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  children: ReactNode;
  size?: 'sm' | 'md' | 'lg' | 'xl';
}

export default function Modal({ open, onClose, title, subtitle, children, size = 'md' }: Props) {
  useEffect(() => {
    if (open) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }
    return () => { document.body.style.overflow = ''; };
  }, [open]);

  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    if (open) document.addEventListener('keydown', handleEsc);
    return () => document.removeEventListener('keydown', handleEsc);
  }, [open, onClose]);

  if (!open) return null;

  const widths = { sm: 'max-w-md', md: 'max-w-lg', lg: 'max-w-2xl', xl: 'max-w-4xl' };

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[5vh]">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-gray-900/40 modal-backdrop animate-fadeIn"
        onClick={onClose}
      />
      {/* Modal panel */}
      <div className={`relative bg-white rounded-2xl shadow-2xl w-full ${widths[size]} max-h-[88vh] overflow-hidden mx-4 animate-slideUp`}>
        {/* Header */}
        <div className="flex items-start justify-between px-6 py-5 border-b border-gray-100 bg-gradient-to-b from-gray-50/80 to-white">
          <div>
            <h2 className="text-lg font-semibold text-gray-900 tracking-tight">{title}</h2>
            {subtitle && <p className="text-sm text-gray-500 mt-0.5">{subtitle}</p>}
          </div>
          <button
            onClick={onClose}
            className="p-1.5 -m-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>
        {/* Body */}
        <div className="px-6 py-5 overflow-y-auto max-h-[calc(88vh-5rem)]">
          {children}
        </div>
      </div>
    </div>
  );
}
