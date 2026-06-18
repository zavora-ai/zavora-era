import { useNavigate } from 'react-router-dom';
import { Printer, ArrowLeft } from 'lucide-react';

interface DocumentActionsProps {
  showBack?: boolean;
}

export default function DocumentActions({ showBack = true }: DocumentActionsProps) {
  const navigate = useNavigate();

  return (
    <div className="no-print flex items-center gap-2 mb-6">
      {showBack && (
        <button onClick={() => navigate(-1)} className="btn-secondary">
          <ArrowLeft className="w-4 h-4" /> Back
        </button>
      )}
      <button onClick={() => window.print()} className="btn-primary">
        <Printer className="w-4 h-4" /> Print / PDF
      </button>
    </div>
  );
}
