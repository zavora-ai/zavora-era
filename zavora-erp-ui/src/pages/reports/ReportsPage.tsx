import { Link } from 'react-router-dom';
import { BarChart3 } from 'lucide-react';
import PageHeader from '../../components/shared/PageHeader';
import { REPORT_CATEGORIES, reportTypes, slugFor } from './lib/reportTypes';

// Reports launcher — a grouped grid of cards linking to each report's page.
export default function ReportsPage() {
  return (
    <div>
      <PageHeader title="Reports" subtitle="Financial and compliance reports" />

      <div className="space-y-6">
        {REPORT_CATEGORIES.map((category) => {
          const items = reportTypes.filter((r) => r.category === category);
          if (items.length === 0) return null;
          return (
            <div key={category}>
              <h2 className="text-xs font-semibold tracking-widest text-gray-500 uppercase mb-2">{category}</h2>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
                {items.map((rt) => (
                  <Link
                    key={rt.key}
                    to={`/reports/${slugFor(rt.key)}`}
                    className="card p-3 text-left transition-all hover:border-gray-300"
                  >
                    <div className="flex items-start gap-2">
                      <BarChart3 className="w-4 h-4 mt-0.5 shrink-0 text-gray-400" />
                      <div>
                        <p className="text-sm font-medium text-gray-900">{rt.name}</p>
                        <p className="text-[11px] text-gray-500 mt-0.5">{rt.desc}</p>
                      </div>
                    </div>
                  </Link>
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
