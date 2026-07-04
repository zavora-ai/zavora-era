// Payment terms helpers (mirror of the backend `PaymentTerms` enum in
// zavora-erp-core/src/types.rs). Vendors and customers carry a payment-terms
// string like "Net30"; documents derive their due date as issue_date + N days.
// Keeping this in sync with the backend lets the UI *preview* the due date the
// server would compute, so the user sees the terms take effect immediately.

const NET_DAYS: Record<string, number> = {
  DueOnReceipt: 0,
  Net7: 7,
  Net14: 14,
  Net30: 30,
  Net45: 45,
  Net60: 60,
  Net90: 90,
};

/**
 * Number of days a payment-terms value represents, or null if unknown.
 * Handles the named variants, a "Custom { days }" JSON object, and a plain
 * "Net<N>" fallback.
 */
export function paymentTermsDays(terms: string | null | undefined): number | null {
  if (!terms) return null;
  // Stored/serialized values may arrive JSON-quoted (e.g. "Net30"); strip them.
  const t = terms.replace(/^"+|"+$/g, '').trim();
  if (t in NET_DAYS) return NET_DAYS[t];
  // Custom terms may be stored as a JSON object { "Custom": { "days": N } }.
  try {
    const parsed = JSON.parse(terms);
    if (parsed?.Custom?.days != null) return Number(parsed.Custom.days);
  } catch {
    /* not JSON */
  }
  const m = /^Net\s*(\d+)$/i.exec(t);
  return m ? Number(m[1]) : null;
}

/** Human label for a payment-terms value, e.g. "Net 30" or "Due on receipt". */
export function paymentTermsLabel(terms: string | null | undefined): string {
  if (!terms) return '—';
  const t = terms.replace(/^"+|"+$/g, '').trim();
  if (t === 'DueOnReceipt') return 'Due on receipt';
  const days = paymentTermsDays(terms);
  return days != null ? `Net ${days}` : t;
}

/**
 * Due date = issue date + terms days, as YYYY-MM-DD. Returns null when the
 * terms are unknown or the issue date is invalid.
 */
export function dueDateFromTerms(
  issueDate: string,
  terms: string | null | undefined,
): string | null {
  const days = paymentTermsDays(terms);
  if (days == null || !/^\d{4}-\d{2}-\d{2}$/.test(issueDate)) return null;
  const d = new Date(issueDate + 'T00:00:00');
  if (isNaN(d.getTime())) return null;
  d.setDate(d.getDate() + days);
  return d.toISOString().split('T')[0];
}
