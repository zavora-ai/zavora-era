import { Link } from 'react-router-dom';
import { Fragment } from 'react';
import { num } from './primitives';

function sourceDocLink(l: any): string | null {
  // Link by the source document's own id (NOT the journal-entry id).
  const id = l.source_id;
  if (!id) return null;
  const src = (l.source || '').replace(/"/g, '');
  if (src === 'Invoice') return `/documents/invoice/${id}`;
  if (src === 'CreditNote') return `/documents/credit-note/${id}`; // customer CN (invoices table)
  if (src === 'Bill') return `/documents/bill/${id}`;
  return null; // SupplierCreditNote/Payment/Payroll/etc. have no preview route
}

export default function GlDetailView({ c }: { c: any }) {
  const allAccounts = c.account_code === 'ALL';
  let lastAcct: string | null = null;
  return (
    <table className="w-full text-sm">
      <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2">Date</th><th className="text-left">JE #</th><th className="text-left">Source</th><th className="text-left">Reference</th><th className="text-right">Debit</th><th className="text-right">Credit</th><th className="text-right">Balance</th></tr></thead>
      <tbody>
        {!allAccounts && (
          <tr className="border-b border-gray-50 text-gray-500"><td className="py-1.5" colSpan={6}>Opening balance — {c.account_code} {c.account_name}</td><td className="text-right font-medium">{num(c.opening_balance)}</td></tr>
        )}
        {c.lines.map((l: any, i: number) => {
          const docLink = sourceDocLink(l);
          // In all-accounts mode, emit a subheader row when the account changes.
          const acctHeader = allAccounts && l.account_code !== lastAcct ? (lastAcct = l.account_code, true) : false;
          return (
            <Fragment key={i}>
              {acctHeader && (
                <tr className="bg-gray-50 border-b">
                  <td className="py-1.5 font-semibold text-gray-700" colSpan={7}>{l.account_code} — {l.account_name}</td>
                </tr>
              )}
              <tr className="border-b border-gray-50">
                <td className="py-1.5">{l.date}</td>
                <td className="font-mono text-xs">
                  <Link to={`/journal-entries/${l.entry_id}`} className="text-blue-600 hover:underline" title="View journal entry">{l.journal_number}</Link>
                </td>
                <td className="text-xs text-gray-400">{(l.source || '').replace(/"/g, '')}</td>
                <td className="text-gray-500">
                  {docLink ? <Link to={docLink} className="text-blue-600 hover:underline">{l.reference}</Link> : l.reference}
                </td>
                <td className="text-right">{l.debit ? num(l.debit) : '—'}</td>
                <td className="text-right">{l.credit ? num(l.credit) : '—'}</td>
                <td className="text-right">{num(l.balance)}</td>
              </tr>
            </Fragment>
          );
        })}
        <tr className="font-bold border-t-2"><td className="py-2" colSpan={6}>{allAccounts ? 'Net movement (all accounts)' : 'Closing balance'}</td><td className="text-right">{num(c.closing_balance)}</td></tr>
      </tbody>
    </table>
  );
}
