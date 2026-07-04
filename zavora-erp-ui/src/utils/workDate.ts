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

function tzKey(): string {
  const id = (getIdentity() as { user_id?: string } | null)?.user_id ?? 'anon';
  return `zavora.timezone.${id}`;
}

/** Default timezone for a Kenyan business (East Africa Time, UTC+3). */
export const DEFAULT_TIMEZONE = 'Africa/Nairobi';

/** The full list of IANA timezones the browser knows, for the picker. */
export function timezoneList(): string[] {
  try {
    // Supported in modern browsers; fall back to a small curated list.
    const supported = (Intl as unknown as { supportedValuesOf?: (k: string) => string[] })
      .supportedValuesOf?.('timeZone');
    if (supported && supported.length) return supported;
  } catch {
    /* ignore */
  }
  return [
    'Africa/Nairobi', 'Africa/Lagos', 'Africa/Cairo', 'Africa/Johannesburg',
    'Europe/London', 'Europe/Paris', 'America/New_York', 'America/Los_Angeles',
    'Asia/Dubai', 'Asia/Kolkata', 'Asia/Shanghai', 'UTC',
  ];
}

/** The user's chosen timezone, defaulting to East Africa Time. */
export function getTimezone(): string {
  try {
    return localStorage.getItem(tzKey()) || DEFAULT_TIMEZONE;
  } catch {
    return DEFAULT_TIMEZONE;
  }
}

/** Set (or reset, with null/empty → default) the user's timezone. */
export function setTimezone(tz: string | null) {
  try {
    if (tz) localStorage.setItem(tzKey(), tz);
    else localStorage.removeItem(tzKey());
  } catch {
    /* ignore storage errors */
  }
  window.dispatchEvent(new CustomEvent('zavora:workdate-changed'));
}

/**
 * Real calendar "today" as YYYY-MM-DD, computed in the user's timezone (not
 * UTC). This matters near midnight: in Nairobi (UTC+3) a 01:00 entry is still
 * "yesterday" in UTC, which would post to the wrong day without this.
 */
export function realToday(): string {
  const tz = getTimezone();
  try {
    // en-CA formats as YYYY-MM-DD.
    return new Intl.DateTimeFormat('en-CA', {
      timeZone: tz,
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
    }).format(new Date());
  } catch {
    return new Date().toISOString().split('T')[0];
  }
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
