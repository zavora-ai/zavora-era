import type { ReactNode } from 'react';
import type { BrandingConfig } from '../../types';

interface DocumentLayoutProps {
  branding?: BrandingConfig;
  title: string;
  subtitle?: string;
  documentNumber: string;
  children: ReactNode;
}

export default function DocumentLayout({ branding, title, subtitle, documentNumber, children }: DocumentLayoutProps) {
  const b = branding ?? ({} as Partial<BrandingConfig>);
  const generatedAt = new Date().toLocaleString('en-KE', { dateStyle: 'medium', timeStyle: 'short' });

  return (
    <div className="print-area mx-auto max-w-4xl bg-white border border-gray-200 rounded-lg shadow-sm">
      {/* Letterhead */}
      <div className="px-10 pt-10 pb-6 border-b border-gray-200">
        <div className="flex items-start justify-between gap-6">
          <div className="flex items-center gap-3">
            {b.logo_url && <img src={b.logo_url} alt="" className="h-12 w-auto object-contain" />}
            <div>
              <h1 className="text-xl font-bold text-gray-900 leading-tight">{b.company_name || 'Your Company'}</h1>
              <p className="text-xs text-gray-500">
                {[b.kra_pin && `KRA PIN: ${b.kra_pin}`, b.vat_number && `VAT: ${b.vat_number}`].filter(Boolean).join('  ·  ')}
              </p>
            </div>
          </div>
          <div className="text-right shrink-0">
            <p className="text-sm font-medium text-gray-500">{documentNumber}</p>
          </div>
        </div>
        <div className="text-center mt-6">
          <h2 className="text-lg font-semibold text-gray-900">{title}</h2>
          {subtitle && <p className="text-sm text-gray-500 mt-0.5">{subtitle}</p>}
        </div>
      </div>

      {/* Body */}
      <div className="px-10 py-8">
        {children}
      </div>

      {/* Footer */}
      <div className="px-10 py-4 border-t border-gray-200 text-[11px] text-gray-400 flex justify-between">
        <span>{b.company_name || ''}</span>
        <span>Generated {generatedAt}</span>
      </div>
    </div>
  );
}
