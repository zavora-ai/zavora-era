import { useState, useEffect, useRef } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getInvoices, getInvoice, createInvoice, updateInvoice, deleteInvoice, postInvoice, sendInvoice, writeOffInvoice, getCustomers, getProducts, getDimensions, getAccounts, getInvoiceTemplates, getFxRates, getSettings } from '../../api/client';
import type { Invoice, Customer, Product } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { workToday } from '../../utils/workDate';
import { dueDateFromTerms, paymentTermsLabel } from '../../utils/paymentTerms';
import { hasRole, ROLES_POST, ROLES_SEND } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import PaginationControls from '../../components/shared/PaginationControls';
import { usePagination } from '../../hooks/usePagination';
import Modal from '../../components/shared/Modal';
import { QuickAddParty, QuickAddProduct, type QuickProduct } from '../../components/shared/QuickAdd';
import { Plus, Send, Pencil, Trash2, ShieldCheck } from 'lucide-react';

const POSTED_LIKE = ['posted', 'sent', 'viewed', 'partially_paid'];
const isPostedLike = (s: string) => POSTED_LIKE.includes(s);

export default function InvoicesPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [editId, setEditId] = useState<string | null>(null);
  // Allow deep-linking to a status tab, e.g. /invoices?status=overdue (used by
  // the dashboard "Needs Attention" overdue card).
  const [searchParams, setSearchParams] = useSearchParams();
  const validFilters = ['all', 'draft', 'posted', 'overdue', 'paid'];
  const initialFilter = validFilters.includes(searchParams.get('status') || '')
    ? (searchParams.get('status') as string)
    : 'all';
  const [filter, setFilter] = useState<string>(initialFilter);
  // Deep-link from a customer's "New Invoice": /invoices?new=1&customer=<id>
  // opens the create form pre-filled for that customer.
  const newCustomerId = searchParams.get('customer') || '';
  const queryClient = useQueryClient();

  useEffect(() => {
    if (searchParams.get('new') === '1') setShowCreate(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const navigate = useNavigate();

  const { page, limit, offset, setPage } = usePagination();
  const { data: resp, isLoading } = useQuery({
    queryKey: ['invoices', offset, limit],
    queryFn: () => getInvoices({ limit, offset }).then(r => r.data),
  });
  const invoices: Invoice[] = resp?.data ?? [];
  const invoicesTotal: number = resp?.total_count ?? 0;

  const { data: customers = [] } = useQuery<any[]>({ queryKey: ['customers'], queryFn: () => getCustomers().then(r => Array.isArray(r.data) ? r.data : []) });
  const customerName = (id?: string) => (Array.isArray(customers) ? customers : []).find((c) => c.id === id)?.name ?? `${id?.slice(0, 8)}…`;
  const [writeOffInv, setWriteOffInv] = useState<any | null>(null);
  const [sendInv, setSendInv] = useState<any | null>(null);

  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['invoices'] });
  const postMutation = useMutation({ mutationFn: (id: string) => postInvoice(id), onSuccess: invalidate });
  const deleteMutation = useMutation({ mutationFn: (id: string) => deleteInvoice(id), onSuccess: invalidate });

  const filtered = filter === 'all' ? invoices
    : filter === 'posted' ? invoices.filter(i => isPostedLike(i.status))
    : invoices.filter(i => i.status === filter);

  const statusCounts: Record<string, number> = {
    all: invoices.length,
    draft: invoices.filter(i => i.status === 'draft').length,
    posted: invoices.filter(i => isPostedLike(i.status)).length,
    overdue: invoices.filter(i => i.status === 'overdue').length,
    paid: invoices.filter(i => i.status === 'paid').length,
  };

  const columns: Column<Invoice>[] = [
    { key: 'status', header: 'Status', render: (r) => (
      <div className="flex items-center gap-1.5">
        <span className={statusColor(r.status)}>{r.status.replace('_', ' ')}</span>
        {(r as any).sent_at && <Send className="w-3 h-3 text-gray-400" aria-label="Sent" />}
        {r.etims_status === 'transmitted' && <ShieldCheck className="w-3 h-3 text-green-600" aria-label="Transmitted to eTIMS" />}
      </div>
    ) },
    { key: 'number', header: 'Invoice #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'customer_id', header: 'Customer', render: (r) => <span className="text-gray-900">{customerName(r.customer_id)}</span> },
    { key: 'issue_date', header: 'Issued', render: (r) => formatDate(r.issue_date) },
    { key: 'due_date', header: 'Due Date', render: (r) => <span className={r.status === 'overdue' ? 'text-red-600 font-medium' : ''}>{formatDate(r.due_date)}</span> },
    { key: 'gross_total', header: 'Total', render: (r) => (
      <div className="text-right">
        <span className="font-medium">{formatCurrency(r.gross_total, r.currency)}</span>
        {r.currency !== 'KES' && <p className="text-xs text-gray-400">≈ {formatCurrency(Number(r.gross_total) * Number(r.fx_rate || 1), 'KES')}</p>}
      </div>
    ), className: 'text-right' },
    { key: 'balance_due', header: 'Amount Due', render: (r) => (
      <div className="text-right">
        <span className="font-bold">{formatCurrency(r.balance_due, r.currency)}</span>
        {r.currency !== 'KES' && <p className="text-xs text-gray-400 font-normal">≈ {formatCurrency(Number(r.balance_due) * Number(r.fx_rate || 1), 'KES')}</p>}
      </div>
    ), className: 'text-right' },
    {
      key: 'actions', header: '',
      render: (r) => (
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          {r.status === 'draft' && (
            <>
              {hasRole(ROLES_POST) && (
                <button onClick={() => postMutation.mutate(r.id)} disabled={postMutation.isPending} className="btn-primary text-xs py-1 px-2" title="Post to the ledger">
                  Post
                </button>
              )}
              <button onClick={() => setEditId(r.id)} className="btn-secondary text-xs py-1 px-2" title="Edit draft">
                <Pencil className="w-3 h-3" />
              </button>
              <button onClick={() => { if (confirm(`Delete draft ${r.number}? This cannot be undone.`)) deleteMutation.mutate(r.id); }} className="btn-secondary text-xs py-1 px-2 text-red-600" title="Delete draft">
                <Trash2 className="w-3 h-3" />
              </button>
            </>
          )}
          {isPostedLike(r.status) && !(r as any).sent_at && hasRole(ROLES_SEND) && (
            <button onClick={() => setSendInv(r)} className="btn-secondary text-xs py-1 px-2" title="Send to customer (email + PDF) or mark as sent">
              <Send className="w-3 h-3" /> Send
            </button>
          )}
          {Number(r.balance_due) > 0 && r.status !== 'draft' && r.status !== 'voided' && hasRole(ROLES_POST) && (
            <button onClick={() => setWriteOffInv(r)} className="btn-secondary text-xs py-1 px-2 text-amber-700" title="Write off as bad debt">
              Write off
            </button>
          )}
        </div>
      )
    },
  ];

  return (
    <div>
      <PageHeader
        title="Invoices"
        subtitle="Draft → Post (to ledger) → Mark sent. Drafts can be edited or deleted."
        actions={
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> Create Invoice
          </button>
        }
      />

      {/* Status filter tabs */}
      <div className="flex gap-1 mb-4 bg-gray-100 p-1 rounded-lg w-fit">
        {(['all', 'draft', 'posted', 'overdue', 'paid'] as const).map((s) => (
          <button
            key={s}
            onClick={() => {
              setFilter(s);
              setSearchParams(s === 'all' ? {} : { status: s }, { replace: true });
            }}
            className={`px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${filter === s ? 'bg-white shadow-sm text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
          >
            {s.charAt(0).toUpperCase() + s.slice(1)} ({statusCounts[s]})
          </button>
        ))}
      </div>

      <DataTable columns={columns} data={filtered} loading={isLoading} onRowClick={(r) => navigate(`/invoices/${r.id}`)} emptyMessage="No invoices yet. Create your first invoice to get paid." />
      <PaginationControls page={page} limit={limit} total={invoicesTotal} onPage={setPage} />

      {showCreate && <CreateInvoiceModal initialCustomerId={newCustomerId} onClose={() => setShowCreate(false)} />}
      {editId && <CreateInvoiceModal editId={editId} onClose={() => setEditId(null)} />}
      {writeOffInv && <WriteOffModal invoice={writeOffInv} onClose={() => setWriteOffInv(null)} onDone={() => { invalidate(); setWriteOffInv(null); }} />}
      {sendInv && <SendInvoiceModal invoice={sendInv} customer={(Array.isArray(customers) ? customers : []).find((c) => c.id === sendInv.customer_id)} onClose={() => setSendInv(null)} onDone={() => { invalidate(); setSendInv(null); }} />}
    </div>
  );
}

// ============================================================
// Full-featured Invoice Creation / Edit — Wave Apps parity
// ============================================================
function SendInvoiceModal({ invoice, customer, onClose, onDone }: { invoice: any; customer?: any; onClose: () => void; onDone: () => void }) {
  const customerEmail = (() => {
    const emails = customer?.email;
    if (Array.isArray(emails) && emails.length > 0) return emails[0]?.email ?? '';
    return '';
  })();
  const [mode, setMode] = useState<'email' | 'mark'>('email');
  const [recipient, setRecipient] = useState(customerEmail);
  const [templateId, setTemplateId] = useState('');
  const [message, setMessage] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);

  const { data: templates = [] } = useQuery<any[]>({
    queryKey: ['invoice-templates'],
    queryFn: () => getInvoiceTemplates().then((r) => (Array.isArray(r.data) ? r.data : [])),
  });

  const mut = useMutation({
    mutationFn: () =>
      sendInvoice(invoice.id, {
        invoice_id: invoice.id,
        channels: ['Email'],
        message: message.trim() || undefined,
        template_id: templateId || undefined,
        recipient_email: mode === 'email' ? (recipient.trim() || undefined) : undefined,
        mark_sent_only: mode === 'mark',
      }),
    onSuccess: (resp: any) => {
      const emailed = resp?.data?.emailed_to;
      if (mode === 'mark' || !emailed) {
        onDone();
      } else {
        setResult(emailed);
      }
    },
    onError: (e: any) => setError(e?.response?.data?.error || 'Failed to send invoice.'),
  });

  return (
    <Modal open={true} onClose={onClose} title={`Send ${invoice.number}`} size="md">
      {result ? (
        <div className="space-y-4">
          <div className="bg-green-50 text-green-700 text-sm p-3 rounded-lg">
            Invoice emailed to <strong>{result}</strong> with a PDF attached, and marked as sent.
          </div>
          <div className="flex justify-end"><button className="btn-primary" onClick={onDone}>Done</button></div>
        </div>
      ) : (
        <div className="space-y-4">
          {error && <div className="bg-red-50 text-red-700 text-sm p-3 rounded-lg">{error}</div>}

          {/* Mode toggle */}
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => setMode('email')}
              className={`flex-1 text-sm py-2 rounded-lg border ${mode === 'email' ? 'border-blue-600 bg-blue-50 text-blue-700 font-medium' : 'border-gray-200 text-gray-600'}`}
            >
              Send by email
            </button>
            <button
              type="button"
              onClick={() => setMode('mark')}
              className={`flex-1 text-sm py-2 rounded-lg border ${mode === 'mark' ? 'border-blue-600 bg-blue-50 text-blue-700 font-medium' : 'border-gray-200 text-gray-600'}`}
            >
              Mark as sent
            </button>
          </div>

          {mode === 'email' ? (
            <>
              <div>
                <label className="label">Recipient email *</label>
                <input className="input" type="email" value={recipient} onChange={(e) => setRecipient(e.target.value)} placeholder="customer@example.com" />
                {!customerEmail && <p className="text-xs text-amber-600 mt-1">This customer has no email on file — enter one to send.</p>}
              </div>
              <div>
                <label className="label">Invoice template</label>
                <select className="input" value={templateId} onChange={(e) => setTemplateId(e.target.value)}>
                  <option value="">Default template</option>
                  {templates.map((t) => (
                    <option key={t.id} value={t.id}>{t.name}{t.is_default ? ' (default)' : ''}</option>
                  ))}
                </select>
                <p className="text-xs text-gray-400 mt-1">Controls the colours and footer on the attached PDF.</p>
              </div>
              <div>
                <label className="label">Message <span className="text-gray-400 font-normal">(optional)</span></label>
                <textarea className="input" rows={3} value={message} onChange={(e) => setMessage(e.target.value)} placeholder="Add a note to the customer…" />
              </div>
              <p className="text-xs text-gray-500">A formatted email with the invoice PDF attached will be sent, and the invoice marked as sent.</p>
            </>
          ) : (
            <p className="text-sm text-gray-600">
              Mark <strong>{invoice.number}</strong> as sent without emailing — for when you've already delivered it
              outside the system (printed, emailed manually, etc.).
            </p>
          )}

          <div className="flex justify-end gap-3 pt-2 border-t">
            <button type="button" className="btn-secondary" onClick={onClose}>Cancel</button>
            <button
              className="btn-primary"
              disabled={mut.isPending || (mode === 'email' && !recipient.trim())}
              onClick={() => { setError(null); mut.mutate(); }}
            >
              {mut.isPending ? 'Sending…' : mode === 'email' ? 'Send invoice' : 'Mark as sent'}
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}

function WriteOffModal({ invoice, onClose, onDone }: { invoice: any; onClose: () => void; onDone: () => void }) {
  const { data: accounts = [] } = useQuery<any[]>({ queryKey: ['accounts'], queryFn: () => getAccounts().then(r => Array.isArray(r.data) ? r.data : []) });
  const expenseAccounts = accounts.filter((a) => a.account_type === 'Expense');
  const [account, setAccount] = useState('');
  const [amount, setAmount] = useState(String(invoice.balance_due));
  const [reason, setReason] = useState('');

  const mut = useMutation({
    mutationFn: () => writeOffInvoice(invoice.id, { expense_account: account, amount: Number(amount), reason: reason || undefined }),
    onSuccess: onDone,
  });

  return (
    <Modal open title={`Write off ${invoice.number}`} onClose={onClose}>
      <div className="space-y-3">
        <p className="text-sm text-gray-600">Writes the outstanding balance off to a bad-debt expense account (DR expense / CR receivables).</p>
        <div>
          <label className="label">Bad-debt expense account</label>
          <select className="input w-full" value={account} onChange={(e) => setAccount(e.target.value)}>
            <option value="">Select account…</option>
            {expenseAccounts.map((a) => <option key={a.code} value={a.code}>{a.code} — {a.name}</option>)}
          </select>
        </div>
        <div>
          <label className="label">Amount</label>
          <input type="number" step="0.01" className="input w-full" value={amount} onChange={(e) => setAmount(e.target.value)} />
          <p className="text-xs text-gray-400 mt-0.5">Outstanding: {formatCurrency(invoice.balance_due)}</p>
        </div>
        <div>
          <label className="label">Reason (optional)</label>
          <input className="input w-full" value={reason} onChange={(e) => setReason(e.target.value)} placeholder="e.g. Customer insolvent" />
        </div>
        {mut.isError && <p className="text-sm text-red-600">{(mut.error as any)?.response?.data?.error ?? 'Failed'}</p>}
        <div className="flex justify-end gap-2 pt-2">
          <button className="btn-secondary" onClick={onClose}>Cancel</button>
          <button className="btn-primary" disabled={!account || !(Number(amount) > 0) || mut.isPending} onClick={() => mut.mutate()}>
            {mut.isPending ? 'Writing off…' : 'Write off'}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function CreateInvoiceModal({ editId, initialCustomerId, onClose }: { editId?: string; initialCustomerId?: string; onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data: customers = [] } = useQuery<Customer[]>({ queryKey: ['customers'], queryFn: () => getCustomers().then(r => Array.isArray(r.data) ? r.data : []) });
  const { data: products = [] } = useQuery<Product[]>({ queryKey: ['products'], queryFn: () => getProducts().then(r => Array.isArray(r.data) ? r.data : []) });
  const { data: dimensionTypes = [] } = useQuery<any[]>({ queryKey: ['dimensions'], queryFn: () => getDimensions().then(r => Array.isArray(r.data) ? r.data : []) });
  // Stored spot rates (the fx_rates table) + base currency, used to auto-fill
  // the invoice exchange rate on the transaction date.
  const { data: fxRates = [] } = useQuery<any[]>({ queryKey: ['fx-rates'], queryFn: () => getFxRates().then(r => Array.isArray(r.data) ? r.data : []) });
  const { data: settings } = useQuery<any>({ queryKey: ['settings'], queryFn: () => getSettings().then(r => r.data) });
  const baseCurrency: string = settings?.base_currency ?? 'KES';

  /** Spot rate (foreign -> base) for `ccy` on/just-before `date` from fx_rates. */
  const lookupSpotRate = (ccy: string, date: string): number | null => {
    if (ccy === baseCurrency) return 1;
    const matches = fxRates
      .filter((r) => r.from_ccy === ccy && r.to_ccy === baseCurrency && r.rate_date <= date)
      .sort((a, b) => (a.rate_date < b.rate_date ? 1 : -1));
    return matches.length ? Number(matches[0].rate) : null;
  };

  const isEdit = !!editId;

  const today = workToday();
  const defaultDue = new Date(new Date(today).getTime() + 30 * 86400000).toISOString().split('T')[0];

  const [form, setForm] = useState({
    customer_id: initialCustomerId || '',
    invoice_date: today,
    due_date: defaultDue,
    po_number: '',
    lines: [emptyLine()],
    notes: '',
    footer: 'Thank you for your business!',
    currency: 'KES',
    fx_rate: '1',
    discount_type: 'none' as 'none' | 'percent' | 'fixed',
    discount_value: 0,
    send_on_save: false,
  });

  // Load existing invoice data when editing
  const { data: existingInvoice } = useQuery({
    queryKey: ['invoice', editId],
    queryFn: () => getInvoice(editId!).then(r => r.data),
    enabled: isEdit,
  });

  useEffect(() => {
    if (existingInvoice && isEdit) {
      const inv = existingInvoice.invoice ?? existingInvoice;
      const lines = (existingInvoice.lines ?? []).map((l: any) => ({
        product_id: l.product_id || '',
        description: l.description || '',
        quantity: l.quantity || 1,
        unit_price: l.unit_price || 0,
        tax_rate: l.vat_treatment === 'Standard16' ? 16 : l.vat_treatment === 'ZeroRated' ? 0 : 0,
        account_code: l.account_code || '5000',
        dimensions: l.dimensions || {},
      }));
      setForm({
        customer_id: inv.customer_id || '',
        invoice_date: inv.issue_date || today,
        due_date: inv.due_date || defaultDue,
        po_number: '',
        lines: lines.length > 0 ? lines : [emptyLine()],
        notes: inv.notes || '',
        footer: 'Thank you for your business!',
        currency: inv.currency || 'KES',
        fx_rate: inv.fx_rate != null ? String(inv.fx_rate) : '1',
        discount_type: 'none',
        discount_value: 0,
        send_on_save: false,
      });
      // Preserve the saved invoice's due date — don't auto-derive over it.
      dueDateTouched.current = true;
    }
  }, [existingInvoice, isEdit]);

  // Auto-fill the exchange rate from the stored spot rates whenever the currency
  // or invoice date changes — unless the user has manually overridden it. For
  // the base currency the rate is always 1. If no spot rate is on file the field
  // is left for the user to enter (and flagged in the UI).
  const fxTouched = useRef(false);
  useEffect(() => {
    if (fxTouched.current) return;
    if (form.currency === baseCurrency) {
      if (form.fx_rate !== '1') setForm((f) => ({ ...f, fx_rate: '1' }));
      return;
    }
    const spot = lookupSpotRate(form.currency, form.invoice_date);
    if (spot != null) setForm((f) => ({ ...f, fx_rate: String(spot) }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [form.currency, form.invoice_date, fxRates, baseCurrency]);

  // Auto-derive the due date from the selected customer's payment terms
  // (invoice date + N days), so the terms visibly take effect. Stops once the
  // user edits the due date by hand, and never overrides a saved invoice.
  const dueDateTouched = useRef(false);
  useEffect(() => {
    if (dueDateTouched.current || isEdit) return;
    const c = customers.find((x) => x.id === form.customer_id);
    if (!c) return;
    const derived = dueDateFromTerms(form.invoice_date, c.payment_terms);
    if (derived && derived !== form.due_date) setForm((f) => ({ ...f, due_date: derived }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [form.customer_id, form.invoice_date, customers, isEdit]);
  const [addingCustomer, setAddingCustomer] = useState(false);
  const [addingItemForLine, setAddingItemForLine] = useState<number | null>(null);

  function emptyLine() {
    // account_code left blank: the server derives it from posting groups (matrix)
    // → product account → default. A value here would override that.
    return { product_id: '', description: '', quantity: 1, unit_price: 0, tax_rate: 16, account_code: '', dimensions: {} as Record<string, string> };
  }

  const applyProductToLine = (i: number, p: QuickProduct) => {
    const lines = [...form.lines];
    lines[i] = {
      ...lines[i],
      product_id: p.id,
      description: lines[i].description || p.name,
      unit_price: p.unit_price,
      // leave account_code blank so the posting-group matrix can derive it.
      tax_rate: p.vat_treatment === 'Standard16' ? 16 : p.vat_treatment === 'ZeroRated' ? 0 : 0,
    };
    setForm({ ...form, lines });
  };

  const mutation = useMutation({
    mutationFn: (data: any) => isEdit ? updateInvoice(editId!, data) : createInvoice(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['invoices'] });
      if (isEdit) queryClient.invalidateQueries({ queryKey: ['invoice', editId] });
      onClose();
    },
  });

  const addLine = () => setForm({ ...form, lines: [...form.lines, emptyLine()] });

  const updateLine = (i: number, field: string, value: any) => {
    const lines = [...form.lines];
    (lines[i] as any)[field] = value;

    // Auto-fill from product selection
    if (field === 'product_id' && value) {
      const product = products.find(p => p.id === value);
      if (product) {
        lines[i].description = product.description || product.name;
        lines[i].unit_price = product.unit_price || 0;
        lines[i].account_code = product.sales_account;
        lines[i].tax_rate = product.vat_treatment === 'Standard16' ? 16 : product.vat_treatment === 'ZeroRated' ? 0 : 0;
      }
    }
    setForm({ ...form, lines });
  };

  const removeLine = (i: number) => {
    if (form.lines.length === 1) return;
    setForm({ ...form, lines: form.lines.filter((_, idx) => idx !== i) });
  };

  // Calculations
  const subtotal = form.lines.reduce((sum, l) => sum + l.quantity * l.unit_price, 0);
  const discount = form.discount_type === 'percent' ? subtotal * form.discount_value / 100
    : form.discount_type === 'fixed' ? form.discount_value : 0;
  const afterDiscount = subtotal - discount;
  const taxByRate: Record<number, number> = {};
  form.lines.forEach(l => {
    const lineTotal = l.quantity * l.unit_price;
    const tax = lineTotal * l.tax_rate / 100;
    taxByRate[l.tax_rate] = (taxByRate[l.tax_rate] || 0) + tax;
  });
  const totalTax = Object.values(taxByRate).reduce((a, b) => a + b, 0);
  const grandTotal = afterDiscount + totalTax;

  // Customer selection handling
  const selectedCustomer = customers.find(c => c.id === form.customer_id);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      customer_id: form.customer_id,
      currency: form.currency,
      fx_rate: Number(form.fx_rate) || 1,
      issue_date: form.invoice_date,
      due_date: form.due_date,
      lines: form.lines.map(l => ({
        product_id: l.product_id || undefined,
        description: l.description,
        quantity: l.quantity,
        unit_price: l.unit_price,
        account_code: l.account_code || undefined,
        vat_treatment: l.tax_rate === 16 ? 'Standard16' : l.tax_rate === 0 ? 'ZeroRated' : 'Exempt',
        dimensions: l.dimensions && Object.keys(l.dimensions).length ? l.dimensions : undefined,
      })),
      notes: form.notes || undefined,
      send_immediately: form.send_on_save,
    });
  };

  return (
    <Modal open={true} onClose={onClose} title={isEdit ? "Edit Invoice" : "Create Invoice"} size="xl">
      <form onSubmit={handleSubmit} className="space-y-6">
        {/* Header — Customer + Dates */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Left — Customer */}
          <div className="space-y-4">
            <div>
              <div className="flex items-center justify-between">
                <label className="label">Bill To *</label>
                <button type="button" onClick={() => setAddingCustomer((v) => !v)} className="text-xs font-medium text-indigo-600 hover:text-indigo-800">
                  {addingCustomer ? 'Cancel' : '+ New customer'}
                </button>
              </div>
              <select
                className="input"
                value={form.customer_id}
                onChange={(e) => {
                  const cust = customers.find((c) => c.id === e.target.value);
                  // Default the invoice currency to the customer's currency (the
                  // fx auto-fill effect then picks the spot rate for the date).
                  setForm((f) => ({ ...f, customer_id: e.target.value, currency: cust?.currency || f.currency }));
                  fxTouched.current = false;
                }}
                required
              >
                <option value="">Choose a customer...</option>
                {customers.map(c => (
                  <option key={c.id} value={c.id}>{c.name}</option>
                ))}
              </select>
              {addingCustomer && (
                <QuickAddParty
                  kind="customer"
                  onCreated={(c) => { setForm((f) => ({ ...f, customer_id: c.id })); setAddingCustomer(false); }}
                  onCancel={() => setAddingCustomer(false)}
                />
              )}
            </div>
            {selectedCustomer && (
              <div className="bg-gray-50 rounded-lg p-3 text-sm text-gray-600">
                <p className="font-medium text-gray-900">{selectedCustomer.name}</p>
                {selectedCustomer.email?.[0] && <p>{selectedCustomer.email[0].email}</p>}
                {selectedCustomer.kra_pin && <p>PIN: {selectedCustomer.kra_pin}</p>}
                <p>Terms: <span className="font-medium text-gray-800">{paymentTermsLabel(selectedCustomer.payment_terms)}</span></p>
              </div>
            )}
          </div>

          {/* Right — Invoice details */}
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="label">Invoice Date</label>
                <input type="date" className="input" value={form.invoice_date} onChange={(e) => setForm({ ...form, invoice_date: e.target.value })} />
              </div>
              <div>
                <label className="label">Payment Due</label>
                <input type="date" className="input" value={form.due_date} onChange={(e) => { dueDateTouched.current = true; setForm({ ...form, due_date: e.target.value }); }} />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="label">P.O. / S.O. Number</label>
                <input className="input" value={form.po_number} onChange={(e) => setForm({ ...form, po_number: e.target.value })} placeholder="Optional" />
              </div>
              <div>
                <label className="label">Currency</label>
                <select className="input" value={form.currency} onChange={(e) => { fxTouched.current = false; setForm({ ...form, currency: e.target.value }); }}>
                  <option value="KES">KES - Kenya Shilling</option>
                  <option value="USD">USD - US Dollar</option>
                  <option value="EUR">EUR - Euro</option>
                  <option value="GBP">GBP - British Pound</option>
                </select>
              </div>
            </div>
            {form.currency !== baseCurrency && (
              <div className="grid grid-cols-2 gap-3 items-start">
                <div>
                  <label className="label">Exchange Rate (1 {form.currency} = ? {baseCurrency})</label>
                  <input
                    type="number" step="0.0001" min="0"
                    className="input font-mono"
                    value={form.fx_rate}
                    onChange={(e) => { fxTouched.current = true; setForm({ ...form, fx_rate: e.target.value }); }}
                    placeholder="e.g. 129.2155"
                  />
                  <p className="text-xs text-gray-400 mt-1">
                    {lookupSpotRate(form.currency, form.invoice_date) != null
                      ? <>Spot rate for {form.invoice_date} (editable — enter your bank/contract rate if different).</>
                      : <span className="text-amber-600">No spot rate on file for this date — enter the rate manually or add it under FX Rates.</span>}
                  </p>
                </div>
                <div className="pt-6 text-sm text-gray-600">
                  ≈ <span className="font-medium">{formatCurrency(grandTotal * (Number(form.fx_rate) || 0), baseCurrency)}</span>
                  <span className="text-gray-400"> in {baseCurrency} at {form.fx_rate || '—'}</span>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Line Items */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="label mb-0">Items</label>
          </div>
          <div className="border rounded-lg overflow-hidden">
            {/* Table header */}
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-3">Product / Service</div>
              <div className="col-span-3">Description</div>
              <div className="col-span-1">Qty</div>
              <div className="col-span-2">Price</div>
              <div className="col-span-1">Tax</div>
              <div className="col-span-1 text-right">Amount</div>
              <div className="col-span-1"></div>
            </div>
            {/* Line rows */}
            {form.lines.map((line, i) => (
              <div key={i} className="grid grid-cols-12 gap-2 px-3 py-2 border-b last:border-b-0 items-center">
                <div className="col-span-3">
                  <select className="input text-sm py-1.5" value={line.product_id} onChange={(e) => updateLine(i, 'product_id', e.target.value)}>
                    <option value="">Select item...</option>
                    {products.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                  <button type="button" onClick={() => setAddingItemForLine(addingItemForLine === i ? null : i)} className="mt-1 text-xs font-medium text-indigo-600 hover:text-indigo-800">
                    {addingItemForLine === i ? 'Cancel' : '+ New item'}
                  </button>
                </div>
                <div className="col-span-3">
                  <input className="input text-sm py-1.5" placeholder="Description" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} />
                  {dimensionTypes.map((dt: any) => (
                    <select
                      key={dt.code}
                      className="input text-xs py-1 mt-1"
                      value={(line.dimensions ?? {})[dt.code] ?? ''}
                      onChange={(e) => updateLine(i, 'dimensions', { ...(line.dimensions ?? {}), [dt.code]: e.target.value })}
                    >
                      <option value="">{dt.name}…</option>
                      {(dt.values ?? []).map((v: any) => <option key={v.code} value={v.code}>{v.name}</option>)}
                    </select>
                  ))}
                </div>
                <div className="col-span-1">
                  <input className="input text-sm py-1.5 text-center" type="number" min="1" step="0.01" value={line.quantity} onChange={(e) => updateLine(i, 'quantity', +e.target.value)} />
                </div>
                <div className="col-span-2">
                  <input className="input text-sm py-1.5" type="number" min="0" step="0.01" value={line.unit_price} onChange={(e) => updateLine(i, 'unit_price', +e.target.value)} />
                </div>
                <div className="col-span-1">
                  <select className="input text-sm py-1.5" value={line.tax_rate} onChange={(e) => updateLine(i, 'tax_rate', +e.target.value)}>
                    <option value={16}>16%</option>
                    <option value={8}>8%</option>
                    <option value={0}>0%</option>
                  </select>
                </div>
                <div className="col-span-1 text-right text-sm font-medium">
                  {formatCurrency(line.quantity * line.unit_price, form.currency)}
                </div>
                <div className="col-span-1 text-center">
                  <button type="button" onClick={() => removeLine(i)} className="text-gray-400 hover:text-red-500 text-lg" disabled={form.lines.length === 1}>×</button>
                </div>
              </div>
            ))}
          </div>
          {addingItemForLine !== null && (
            <QuickAddProduct
              onCreated={(p) => { applyProductToLine(addingItemForLine, p); setAddingItemForLine(null); }}
              onCancel={() => setAddingItemForLine(null)}
            />
          )}
          <button type="button" onClick={addLine} className="mt-2 text-sm font-medium text-blue-600 hover:text-blue-800">
            + Add a Line
          </button>
        </div>

        {/* Notes + Totals row */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Left — Notes */}
          <div className="space-y-3">
            <div>
              <label className="label">Notes / Memo <span className="text-gray-400 font-normal">(visible to customer)</span></label>
              <textarea className="input" rows={3} value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} placeholder="Add payment instructions, project details, or a personal note..." />
            </div>
            <div>
              <label className="label">Footer <span className="text-gray-400 font-normal">(appears on all invoices)</span></label>
              <input className="input" value={form.footer} onChange={(e) => setForm({ ...form, footer: e.target.value })} />
            </div>
          </div>

          {/* Right — Totals */}
          <div className="bg-gray-50 rounded-lg p-4 space-y-2">
            <div className="flex justify-between text-sm">
              <span className="text-gray-600">Subtotal</span>
              <span className="font-medium">{formatCurrency(subtotal, form.currency)}</span>
            </div>

            {/* Discount */}
            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2">
                <span className="text-gray-600">Discount</span>
                <select className="input text-xs py-0.5 px-2 w-auto" value={form.discount_type} onChange={(e) => setForm({ ...form, discount_type: e.target.value as any })}>
                  <option value="none">None</option>
                  <option value="percent">%</option>
                  <option value="fixed">Fixed</option>
                </select>
                {form.discount_type !== 'none' && (
                  <input type="number" className="input text-xs py-0.5 px-2 w-16" value={form.discount_value} onChange={(e) => setForm({ ...form, discount_value: +e.target.value })} />
                )}
              </div>
              <span>{discount > 0 ? `-${formatCurrency(discount, form.currency)}` : '—'}</span>
            </div>

            {/* Tax lines */}
            {Object.entries(taxByRate).map(([rate, amount]) => (
              <div key={rate} className="flex justify-between text-sm">
                <span className="text-gray-600">VAT ({rate}%)</span>
                <span>{formatCurrency(amount, form.currency)}</span>
              </div>
            ))}

            <div className="border-t pt-2 mt-2 flex justify-between text-base font-bold">
              <span>Total ({form.currency})</span>
              <span>{formatCurrency(grandTotal, form.currency)}</span>
            </div>

            <div className="border-t pt-3 mt-3 flex justify-between text-sm text-gray-600">
              <span>Balance Due</span>
              <span className="font-bold text-lg text-gray-900">{formatCurrency(grandTotal, form.currency)}</span>
            </div>
            {form.currency !== baseCurrency && (
              <div className="flex justify-between text-xs text-gray-500">
                <span>Equivalent in {baseCurrency}</span>
                <span>{formatCurrency(grandTotal * (Number(form.fx_rate) || 0), baseCurrency)} @ {form.fx_rate || '—'}</span>
              </div>
            )}
          </div>
        </div>

        {/* Footer actions */}
        <div className="flex items-center justify-between pt-4 border-t">
          <label className="flex items-center gap-2 text-sm text-gray-600 cursor-pointer">
            <input type="checkbox" checked={form.send_on_save} onChange={(e) => setForm({ ...form, send_on_save: e.target.checked })} className="rounded" />
            Send invoice to customer immediately
          </label>
          <div className="flex gap-3">
            <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
            <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.customer_id}>
              {mutation.isPending ? 'Saving...' : isEdit ? 'Update Invoice' : form.send_on_save ? 'Save & Send' : 'Save as Draft'}
            </button>
          </div>
        </div>
      </form>
    </Modal>
  );
}
