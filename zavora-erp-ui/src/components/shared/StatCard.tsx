import type { ReactNode } from 'react';
import clsx from 'clsx';

interface Props {
  title: string;
  value: string;
  subtitle?: string;
  icon?: ReactNode;
  trend?: { value: string; positive: boolean };
  className?: string;
  onClick?: () => void;
}

export default function StatCard({ title, value, subtitle, icon, trend, className, onClick }: Props) {
  const clickable = !!onClick;
  return (
    <div
      className={clsx(
        'card p-5 group hover:shadow-md transition-shadow',
        clickable && 'cursor-pointer hover:border-indigo-300',
        className,
      )}
      onClick={onClick}
      role={clickable ? 'button' : undefined}
      tabIndex={clickable ? 0 : undefined}
      onKeyDown={clickable ? (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onClick!(); } } : undefined}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="space-y-1 min-w-0">
          <p className="text-xs font-semibold uppercase tracking-wider text-gray-400 truncate">{title}</p>
          <p className="text-xl font-bold text-gray-900 tracking-tight tabular-nums break-words leading-tight">{value}</p>
          {subtitle && <p className="text-xs text-gray-500">{subtitle}</p>}
          {trend && (
            <div className={clsx('inline-flex items-center gap-1 text-xs font-semibold px-1.5 py-0.5 rounded-md', trend.positive ? 'bg-green-50 text-green-700' : 'bg-red-50 text-red-700')}>
              {trend.positive ? '↗' : '↘'} {trend.value}
            </div>
          )}
        </div>
        {icon && (
          <div className="p-2.5 bg-indigo-50 rounded-xl text-indigo-600 group-hover:bg-indigo-100 transition-colors shrink-0">
            {icon}
          </div>
        )}
      </div>
    </div>
  );
}
