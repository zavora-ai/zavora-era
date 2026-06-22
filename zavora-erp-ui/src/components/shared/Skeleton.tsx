// Tailwind animate-pulse placeholders shown while data loads.

export function SkeletonCard({ className = '' }: { className?: string }) {
  return (
    <div className={`card p-5 animate-pulse ${className}`}>
      <div className="h-3 w-24 bg-gray-200 rounded mb-3" />
      <div className="h-7 w-32 bg-gray-200 rounded" />
    </div>
  );
}

export function SkeletonTable({ rows = 6, cols = 4 }: { rows?: number; cols?: number }) {
  return (
    <div className="card p-5 animate-pulse">
      <div className="h-4 bg-gray-200 rounded mb-4 w-1/3" />
      {Array.from({ length: rows }).map((_, r) => (
        <div key={r} className="flex gap-4 py-2 border-b border-gray-50">
          {Array.from({ length: cols }).map((_, c) => (
            <div key={c} className="h-3 bg-gray-100 rounded flex-1" />
          ))}
        </div>
      ))}
    </div>
  );
}

export function SkeletonLines({ lines = 4 }: { lines?: number }) {
  return (
    <div className="animate-pulse space-y-3">
      {Array.from({ length: lines }).map((_, i) => (
        <div key={i} className="h-9 bg-gray-100 rounded" />
      ))}
    </div>
  );
}
