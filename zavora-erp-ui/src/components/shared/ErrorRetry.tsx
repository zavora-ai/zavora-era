import { AlertTriangle } from 'lucide-react';

// Inline error state with a retry action, used wherever a query can fail.
export default function ErrorRetry({ message, onRetry }: { message?: string; onRetry?: () => void }) {
  return (
    <div className="card p-6 flex flex-col items-center text-center gap-3">
      <AlertTriangle className="w-8 h-8 text-amber-500" />
      <p className="text-sm text-gray-600">{message ?? 'Something went wrong loading this data.'}</p>
      {onRetry && (
        <button onClick={onRetry} className="btn-secondary text-sm">Retry</button>
      )}
    </div>
  );
}
