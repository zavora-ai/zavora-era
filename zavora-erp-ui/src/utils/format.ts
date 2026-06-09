import { format, parseISO, isValid } from 'date-fns';

export function formatCurrency(amount: number, currency = 'KES'): string {
  return new Intl.NumberFormat('en-KE', {
    style: 'currency',
    currency,
    minimumFractionDigits: 2,
  }).format(amount);
}

export function formatNumber(n: number): string {
  return new Intl.NumberFormat('en-KE').format(n);
}

export function formatDate(date: string | Date): string {
  if (!date) return '';
  const d = typeof date === 'string' ? parseISO(date) : date;
  return isValid(d) ? format(d, 'dd MMM yyyy') : '';
}

export function formatDateShort(date: string | Date): string {
  if (!date) return '';
  const d = typeof date === 'string' ? parseISO(date) : date;
  return isValid(d) ? format(d, 'dd/MM/yyyy') : '';
}

export function statusColor(status: string): string {
  const map: Record<string, string> = {
    draft: 'badge-gray',
    sent: 'badge-info',
    viewed: 'badge-info',
    partially_paid: 'badge-warning',
    paid: 'badge-success',
    overdue: 'badge-danger',
    voided: 'badge-gray',
    pending_approval: 'badge-warning',
    approved: 'badge-success',
    posted: 'badge-success',
    disputed: 'badge-danger',
    cancelled: 'badge-gray',
    open: 'badge-success',
    soft_closed: 'badge-warning',
    hard_closed: 'badge-danger',
    future: 'badge-gray',
  };
  return map[status] || 'badge-gray';
}
