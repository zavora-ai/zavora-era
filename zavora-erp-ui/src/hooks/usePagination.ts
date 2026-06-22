import { useSearchParams } from 'react-router-dom';

// Page/limit/offset synced to the URL (?page=) so list pages are bookmarkable
// and survive back/forward navigation (Requirement 3.6).
export function usePagination(limit = 50) {
  const [sp, setSp] = useSearchParams();
  const page = Math.max(1, parseInt(sp.get('page') ?? '1', 10) || 1);
  const offset = (page - 1) * limit;
  const setPage = (p: number) => {
    const next = new URLSearchParams(sp);
    if (p <= 1) next.delete('page');
    else next.set('page', String(p));
    setSp(next);
  };
  return { page, limit, offset, setPage };
}
