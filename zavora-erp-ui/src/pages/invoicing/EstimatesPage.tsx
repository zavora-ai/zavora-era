import PageHeader from '../../components/shared/PageHeader';
import { Plus, FileText, ArrowRight } from 'lucide-react';

export default function EstimatesPage() {
  return (
    <div>
      <PageHeader title="Estimates & Quotes" subtitle="Create estimates and convert to invoices with one click" actions={<button className="btn-primary"><Plus className="w-4 h-4" /> New Estimate</button>} />
      <div className="card p-12 text-center">
        <FileText className="w-12 h-12 text-gray-300 mx-auto mb-4" />
        <h3 className="text-lg font-medium text-gray-900 mb-2">No estimates yet</h3>
        <p className="text-sm text-gray-500 mb-4">Create estimates for customers. When accepted, convert them to invoices with a single click.</p>
        <div className="flex items-center justify-center gap-2 text-sm text-gray-400">
          <span className="badge-gray">Estimate</span> <ArrowRight className="w-4 h-4" /> <span className="badge-success">Invoice</span>
        </div>
      </div>
    </div>
  );
}
