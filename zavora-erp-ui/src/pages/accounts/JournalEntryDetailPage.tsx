import { Link, useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { ArrowLeft, FileText } from 'lucide-react';
import { getJournalEntry } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { formatCurrency, formatDate } from '../../utils/format';

// Link a posted entry back to its source document, by source type + id.
function sourceDocLink(entry: any): string | null {
  const id = entry?.source_id;
  if (!id) return null;
  const src = (entry.source || '').replace(/"/g, '');
  if (src === 'Invoice') return `/documents/invoice/${id}`;
  if (src === 'CreditNote') return `/documents/credit-note/${id}`;
  if (src === 'Bill') return `/documents/bill/${id}`;
  return null;
}

export default function JournalEntryDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { data, isLoading, isError } = useQuery({
    queryKey: ['journal-entry', id],
    queryFn: () => getJournalEntry(id!).then((r) => r.data),
    enabled: !!id,
  });

  const entry = data?.entry;
  const lines: any[] = data?.lines ?? [];
  const source = (entry?.source || '').replace(/"/g, '');
  const docLink = entry ? sourceDocLink(entry) : null;

  const totalDebit = lines.reduce((s, l) => s + Number(l.debit ?? 0), 0);
  const totalCredit = lines.reduce((s, l) => s + Number(l.credit ?? 0), 0);
  const balanced = Math.abs(totalDebit - totalCredit) < 0.01;

  return (
    <div>
      <PageHeader
        title={entry ? `Journal Entry ${entry.number}` : 'Journal Entry'}
        subtitle="Posted, immutable — correct via a reversing entry"
        actions={
          <Link to="/journal-entries" className="btn-secondary">
            <ArrowLeft className="w-4 h-4" /> All entries
          </Link>
        }
      />

      {isLoading && (
        <div className="card p-12 text-center">
          <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
          <p className="mt-3 text-sm text-gray-500">Loading entry…</p>
        </div>
      )}
      {isError && <div className="card p-6 text-center text-sm text-red-600">Could not load this journal entry.</div>}

      {entry && (
        <>
          <div className="card p-5 mb-4 grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div><p className="label">Date</p><p className="font-medium">{formatDate(entry.date)}</p></div>
            <div><p className="label">Status</p><p className="font-medium capitalize">{entry.status}</p></div>
            <div><p className="label">Source</p><p className="font-medium">{source || '—'}</p></div>
            <div>
              <p className="label">Reference</p>
              <p className="font-medium">
                {docLink ? <Link to={docLink} className="text-blue-600 hover:underline inline-flex items-center gap-1"><FileText className="w-3.5 h-3.5" />{entry.reference || '—'}</Link> : (entry.reference || '—')}
              </p>
            </div>
            {entry.description && <div className="col-span-2 md:col-span-4"><p className="label">Description</p><p>{entry.description}</p></div>}
          </div>

          <div className="card p-5">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-xs text-gray-500 uppercase border-b">
                  <th className="text-left py-2">Account</th>
                  <th className="text-left">Narration</th>
                  <th className="text-right">Debit</th>
                  <th className="text-right">Credit</th>
                </tr>
              </thead>
              <tbody>
                {lines.map((l) => (
                  <tr key={l.id} className="border-b border-gray-50">
                    <td className="py-1.5 font-mono text-xs text-gray-600">{l.account_code}</td>
                    <td className="text-gray-500">{l.description || '—'}</td>
                    <td className="text-right tabular-nums">{Number(l.debit) ? formatCurrency(Number(l.debit)) : '—'}</td>
                    <td className="text-right tabular-nums">{Number(l.credit) ? formatCurrency(Number(l.credit)) : '—'}</td>
                  </tr>
                ))}
              </tbody>
              <tfoot>
                <tr className="font-bold border-t-2">
                  <td className="py-2" colSpan={2}>
                    Total {balanced
                      ? <span className="ml-2 text-xs font-medium text-green-700 bg-green-50 px-2 py-0.5 rounded">Balanced</span>
                      : <span className="ml-2 text-xs font-medium text-red-700 bg-red-50 px-2 py-0.5 rounded">Out of balance</span>}
                  </td>
                  <td className="text-right tabular-nums">{formatCurrency(totalDebit)}</td>
                  <td className="text-right tabular-nums">{formatCurrency(totalCredit)}</td>
                </tr>
              </tfoot>
            </table>
          </div>
        </>
      )}
    </div>
  );
}
