interface Props {
  page: number;
  limit: number;
  total: number;
  onPage: (p: number) => void;
}

// Next/previous pager with a record-range indicator, shown below list tables.
export default function PaginationControls({ page, limit, total, onPage }: Props) {
  const totalPages = Math.max(1, Math.ceil(total / limit));
  if (total <= limit && page === 1) return null; // nothing to page through
  const from = total === 0 ? 0 : (page - 1) * limit + 1;
  const to = Math.min(page * limit, total);
  return (
    <div className="flex items-center justify-between mt-3 text-sm text-gray-600">
      <span>{from}–{to} of {total}</span>
      <div className="flex items-center gap-2">
        <button className="btn-secondary text-xs py-1" disabled={page <= 1} onClick={() => onPage(page - 1)}>Previous</button>
        <span className="text-xs">Page {page} of {totalPages}</span>
        <button className="btn-secondary text-xs py-1" disabled={page >= totalPages} onClick={() => onPage(page + 1)}>Next</button>
      </div>
    </div>
  );
}
