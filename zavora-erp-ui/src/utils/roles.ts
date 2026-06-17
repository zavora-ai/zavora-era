import { getIdentity } from '../api/client';

// === Role-aware action gating ===
// The backend already enforces these permissions on every endpoint. This is a
// UX layer so users don't see actions they can't perform. Never rely on this
// for security — it only hides buttons.

/** Read the current user's role from the in-memory identity, or null. */
export function getCurrentRole(): string | null {
  const identity = getIdentity() as { role?: string } | null;
  return identity?.role ?? null;
}

/** True when the current user's role is one of the allowed roles. */
export function hasRole(allowed: string[]): boolean {
  const role = getCurrentRole();
  return role != null && allowed.includes(role);
}

// Permission groups mirroring the backend role checks.
export const ROLES_CREATE = ['Owner', 'Admin', 'Accountant', 'Editor'];
export const ROLES_SEND = ['Owner', 'Admin', 'Accountant', 'Editor'];
export const ROLES_APPROVE = ['Owner', 'Admin', 'Approver'];
export const ROLES_POST = ['Owner', 'Admin', 'Accountant']; // post journal/invoice/bill, reverse
export const ROLES_CLOSE_PERIOD = ['Owner', 'Admin'];
export const ROLES_MANAGE = ['Owner', 'Admin'];
