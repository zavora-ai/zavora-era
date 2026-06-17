import type { ReactNode } from 'react';

export interface Column<T> {
  key: string;
  header: string;
  render?: (row: T) => ReactNode;
  className?: string;
}

interface Props<T> {
  columns: Column<T>[];
  data: T[];
  onRowClick?: (row: T) => void;
  emptyMessage?: string;
  loading?: boolean;
}

export default function DataTable<T extends Record<string, any>>({
  columns, data, onRowClick, emptyMessage = 'No data found', loading
}: Props<T>) {
  if (loading) {
    return (
      <div className="card">
        <div className="p-12 text-center">
          <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
          <p className="mt-3 text-sm text-gray-500">Loading...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="card overflow-hidden">
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="border-b border-gray-200 bg-gray-50">
              {columns.map((col) => (
                <th key={col.key} className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200">
            {data.length === 0 ? (
              <tr>
                <td colSpan={columns.length} className="px-6 py-12 text-center text-sm text-gray-500">
                  {emptyMessage}
                </td>
              </tr>
            ) : (
              data.map((row, i) => (
                <tr
                  key={row.id || i}
                  onClick={() => onRowClick?.(row)}
                  className={onRowClick ? 'hover:bg-gray-50 cursor-pointer' : ''}
                >
                  {columns.map((col) => (
                    <td key={col.key} className={`px-6 py-4 text-sm text-gray-900 ${col.className || ''}`}>
                      {col.render ? col.render(row) : row[col.key]}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
