// Per-user "work-as-of date" preference.
//
// Documents (invoices, bills, payments, journals, …) default their date field
// to this value, letting a user post into the period they are working in (e.g.
// finalising last year's books) without re-typing the date on every form. It is
// a CONVENIENCE DEFAULT only — the date field stays visible and editable on each
// form, and the backend still validates the date against open fiscal periods.
//
// Scope: per user, stored in localStorage keyed by the signed-in user id, so it
// is private to that user on that browser and does not affect anyone else.
// Clearing it (or signing in as another user) falls back to the real today.

import { getIdentity } from '../api/client';

function userKey(): string {
  const id = (getIdentity() as { user_id?: string } | null)?.user_id ?? 'anon';
  return `zavora.workDate.${id}`;
}

/** Real calendar today, as YYYY-MM-DD (local time). */
export function realToday(): string {
  return new Date().toISOString().split('T')[0];
}

/** The user's work-as-of date, or `null` when not set. */
export function getWorkDate(): string | null {
  try {
    const v = localStorage.getItem(userKey());
    return v && /^\d{4}-\d{2}-\d{2}$/.test(v) ? v : null;
  } catch {
    return null;
  }
}

/** Set (or clear, with null/empty) the user's work-as-of date. */
export function setWorkDate(date: string | null) {
  try {
    if (date && /^\d{4}-\d{2}-\d{2}$/.test(date)) localStorage.setItem(userKey(), date);
    else localStorage.removeItem(userKey());
  } catch {
    /* ignore storage errors */
  }
  // Notify listeners (the UserMenu badge, open forms) within this tab.
  window.dispatchEvent(new CustomEvent('zavora:workdate-changed'));
}

/**
 * The date new documents should default to: the user's work-as-of date when
 * set, otherwise the real today. Use this in place of
 * `new Date().toISOString().split('T')[0]` for transaction-posting forms.
 */
export function workToday(): string {
  return getWorkDate() ?? realToday();
}
