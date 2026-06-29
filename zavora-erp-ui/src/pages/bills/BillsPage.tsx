import { useState, useEffect, useRef } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getBills, getBill, createBill, updateBill, deleteBill, approveBill, postBill, getVendors, getProducts, getDimensions, createSupplierCreditNote, getFxRates, getSettings } from '../../api/client';
import type { Bill, Vendor, Product } from '../../types';
import { formatCurrency, formatDate, statusColor } from '../../utils/format';
import { hasRole, ROLES_APPROVE, ROLES_CREATE, ROLES_POST } from '../../utils/roles';
import PageHeader from '../../components/shared/PageHeader';
import DataTable, { type Column } from '../../components/shared/DataTable';
import PaginationControls from '../../components/shared/PaginationControls';
import { usePagination } from '../../hooks/usePagination';
import Modal from '../../components/shared/Modal';
import { QuickAddParty, QuickAddProduct, type QuickProduct } from '../../components/shared/QuickAdd';
import Attachments from '../../components/shared/Attachments';
import { Plus, CheckCircle, Pencil, Trash2, Eye, ReceiptText, Paperclip } from 'lucide-react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';

export default function BillsPage() {
  const [showCreate, setShowCreate] = useState(false);
  const [editId, setEditId] = useState<string | null>(null);
  const [scnBill, setScnBill] = useState<Bill | null>(null);
  const [attachBill, setAttachBill] = useState<Bill | null>(null);
  const [filter, setFilter] = useState<string>('all');
  // Deep-link from a vendor's "New Bill": /bills?new=1&vendor=<id> opens the
  // create form pre-filled for that vendor.
  const [searchParams] = useSearchParams();
  const newVendorId = searchParams.get('vendor') || '';
  useEffect(() => {
    if (searchParams.get('new') === '1') setShowCreate(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  const { page, limit, offset, setPage } = usePagination();
  const { data: resp, isLoading } = useQuery({
    queryKey: ['bills', offset, limit],
    queryFn: () => getBills({ limit, offset }).then(r => r.data),
  });
  const bills: Bill[] = resp?.data ?? [];
  const billsTotal: number = resp?.total_count ?? 0;
  const { data: vendors = [] } = useQuery<Vendor[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then(r => Array.isArray(r.data) ? r.data : []) });
  const vendorName = (id?: string) => vendors.find(v => v.id === id)?.name ?? `${id?.slice(0, 8)}…`;

  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['bills'] });
  const approveMut = useMutation({ mutationFn: (id: string) => approveBill(id), onSuccess: invalidate });
  const postMut = useMutation({ mutationFn: (id: string) => postBill(id), onSuccess: invalidate });
  const deleteMut = useMutation({ mutationFn: (id: string) => deleteBill(id), onSuccess: invalidate });

  const filtered = filter === 'all' ? bills
    : bills.filter(b => b.status === filter);

  const statusCounts: Record<string, number> = {
    all: bills.length,
    draft: bills.filter(b => b.status === 'draft').length,
    approved: bills.filter(b => b.status === 'approved').length,
    posted: bills.filter(b => b.status === 'posted').length,
    paid: bills.filter(b => b.status === 'paid').length,
  };

  const columns: Column<Bill>[] = [
    { key: 'status', header: 'Status', render: (r) => <span className={statusColor(r.status)}>{r.status.replace('_', ' ')}</span> },
    { key: 'number', header: 'Bill #', render: (r) => <span className="font-medium text-blue-600">{r.number}</span> },
    { key: 'vendor_id', header: 'Vendor', render: (r) => <span className="text-gray-900">{vendorName(r.vendor_id)}</span> },
    { key: 'issue_date', header: 'Date', render: (r) => formatDate(r.issue_date) },
    { key: 'due_date', header: 'Due', render: (r) => formatDate(r.due_date) },
    {
      key: 'gross_total',
      header: 'Amount',
      className: 'text-right',
      render: (r) => (
        <div>
          <span className="font-medium">{formatCurrency(r.gross_total, r.currency)}</span>
          {r.currency !== 'KES' && (
            <p className="text-xs text-gray-400">≈ {formatCurrency(Number(r.gross_total) * Number(r.fx_rate || 1), 'KES')}</p>
          )}
        </div>
      ),
    },
    { key: 'wht_amount', header: 'WHT', render: (r) => r.wht_amount > 0 ? formatCurrency(r.wht_amount, r.currency) : '—', className: 'text-right' },
    {
      key: 'actions', header: '',
      render: (r) => (
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          <Link
            to={`/documents/bill/${r.id}`}
            className="btn-secondary text-xs py-1 px-2"
            title="Preview document"
          >
            <Eye className="w-3 h-3" />
          </Link>
          {/* Attachments available at every status — the source document
              (e.g. an OCR-captured invoice) should be viewable before posting. */}
          {hasRole(ROLES_CREATE) && (
            <button onClick={() => setAttachBill(r)} className="btn-secondary text-xs py-1 px-2" title="Attachments">
              <Paperclip className="w-3 h-3" />
            </button>
          )}
          {r.status === 'draft' && (
            <>
              {hasRole(ROLES_APPROVE) && (
                <button onClick={() => approveMut.mutate(r.id)} disabled={approveMut.isPending} className="btn-success text-xs py-1 px-2" title="Approve bill">
                  <CheckCircle className="w-3 h-3" /> Approve
                </button>
              )}
              <button onClick={() => setEditId(r.id)} className="btn-secondary text-xs py-1 px-2" title="Edit draft">
                <Pencil className="w-3 h-3" />
              </button>
              <button onClick={() => { if (confirm(`Delete draft ${r.number}? This cannot be undone.`)) deleteMut.mutate(r.id); }} className="btn-secondary text-xs py-1 px-2 text-red-600" title="Delete draft">
                <Trash2 className="w-3 h-3" />
              </button>
            </>
          )}
          {r.status === 'approved' && hasRole(ROLES_POST) && (
            <button onClick={() => postMut.mutate(r.id)} disabled={postMut.isPending} className="btn-primary text-xs py-1 px-2" title="Post to the ledger">
              Post
            </button>
          )}
          {(r.status === 'posted' || r.status === 'paid') && hasRole(ROLES_CREATE) && (
            <>
              <button onClick={() => setScnBill(r)} className="btn-secondary text-xs py-1 px-2 text-red-600" title="Issue supplier credit note">
                <ReceiptText className="w-3 h-3" /> Credit Note
              </button>
            </>
          )}
        </div>
      )
    },
  ];

  return (
    <div>
      <PageHeader
        title="Bills"
        subtitle="Draft → Approve → Post (to ledger). Drafts can be edited or deleted."
        actions={
          <button onClick={() => setShowCreate(true)} className="btn-primary">
            <Plus className="w-4 h-4" /> New Bill
          </button>
        }
      />

      {/* Status filter tabs */}
      <div className="flex gap-1 mb-4 bg-gray-100 p-1 rounded-lg w-fit">
        {(['all', 'draft', 'approved', 'posted', 'paid'] as const).map((s) => (
          <button
            key={s}
            onClick={() => setFilter(s)}
            className={`px-3 py-1.5 rounded-md text-sm font-medium transition-colors ${filter === s ? 'bg-white shadow-sm text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
          >
            {s.charAt(0).toUpperCase() + s.slice(1)} ({statusCounts[s]})
          </button>
        ))}
      </div>

      <DataTable columns={columns} data={filtered} loading={isLoading} onRowClick={(r) => navigate(`/documents/bill/${r.id}`)} emptyMessage="No bills yet. Create your first bill to track payables." />
      <PaginationControls page={page} limit={limit} total={billsTotal} onPage={setPage} />

      {showCreate && <CreateBillModal initialVendorId={newVendorId} onClose={() => setShowCreate(false)} />}
      {editId && <CreateBillModal editId={editId} onClose={() => setEditId(null)} />}
      {scnBill && <SupplierCreditNoteModal bill={scnBill} onClose={() => setScnBill(null)} />}
      {attachBill && (
        <Modal open={true} onClose={() => setAttachBill(null)} title={`Attachments — ${attachBill.number}`}>
          <Attachments linkedType="bill" linkedId={attachBill.id} label="Source document (supplier invoice / receipt)" />
          <div className="flex justify-end pt-4 mt-2 border-t">
            <button type="button" onClick={() => setAttachBill(null)} className="btn-secondary">Close</button>
          </div>
        </Modal>
      )}
    </div>
  );
}


// ============================================================
// Full-featured Bill Creation / Edit modal
// ============================================================
function CreateBillModal({ editId, initialVendorId, onClose }: { editId?: string; initialVendorId?: string; onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data: vendors = [] } = useQuery<Vendor[]>({ queryKey: ['vendors'], queryFn: () => getVendors().then(r => Array.isArray(r.data) ? r.data : []) });
  const { data: products = [] } = useQuery<Product[]>({ queryKey: ['products'], queryFn: () => getProducts().then(r => Array.isArray(r.data) ? r.data : []) });
  const { data: dimensionTypes = [] } = useQuery<any[]>({ queryKey: ['dimensions'], queryFn: () => getDimensions().then(r => Array.isArray(r.data) ? r.data : []) });

  const isEdit = !!editId;

  const today = new Date().toISOString().split('T')[0];

  function emptyLine() {
    // Blank account: product lines derive it from the posting-group matrix; a
    // product-less expense line falls back to the vendor/default expense account.
    return { product_id: '', description: '', quantity: 1, unit_price: 0, tax_rate: 16, account_code: '', dimensions: {} as Record<string, string> };
  }

  const [form, setForm] = useState({
    vendor_id: initialVendorId || '',
    issue_date: today,
    due_date: '',
    vendor_invoice_number: '',
    notes: '',
    currency: 'KES',
    fx_rate: '1',
    lines: [emptyLine()],
  });

  // Stored spot rates for auto-filling the exchange rate on a foreign-currency bill.
  const { data: fxRates = [] } = useQuery<any[]>({ queryKey: ['fx-rates'], queryFn: () => getFxRates().then(r => Array.isArray(r.data) ? r.data : []) });
  const { data: settings } = useQuery<any>({ queryKey: ['settings'], queryFn: () => getSettings().then(r => r.data) });
  const baseCurrency: string = settings?.base_currency ?? 'KES';
  const lookupSpotRate = (ccy: string, date: string): number | null => {
    if (ccy === baseCurrency) return 1;
    const matches = fxRates
      .filter((r) => r.from_ccy === ccy && r.to_ccy === baseCurrency && r.rate_date <= date)
      .sort((a, b) => (a.rate_date < b.rate_date ? 1 : -1));
    return matches.length ? Number(matches[0].rate) : null;
  };
  // Auto-fill the rate from stored spot rates on currency/date change (unless the
  // user overrode it). Base currency is always 1.
  const fxTouched = useRef(false);
  useEffect(() => {
    if (fxTouched.current) return;
    if (form.currency === baseCurrency) { if (form.fx_rate !== '1') setForm((f) => ({ ...f, fx_rate: '1' })); return; }
    const spot = lookupSpotRate(form.currency, form.issue_date);
    if (spot != null) setForm((f) => ({ ...f, fx_rate: String(spot) }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [form.currency, form.issue_date, fxRates, baseCurrency]);

  const [addingVendor, setAddingVendor] = useState(false);
  const [addingItemForLine, setAddingItemForLine] = useState<number | null>(null);

  // Load existing bill data when editing
  const { data: existingBill } = useQuery({
    queryKey: ['bill', editId],
    queryFn: () => getBill(editId!).then(r => r.data),
    enabled: isEdit,
  });

  useEffect(() => {
    if (existingBill && isEdit) {
      const bill = existingBill.bill ?? existingBill;
      const lines = (existingBill.lines ?? []).map((l: any) => ({
        product_id: l.product_id || '',
        description: l.description || '',
        quantity: l.quantity || 1,
        unit_price: l.unit_price || 0,
        tax_rate: l.vat_treatment === 'Standard16' ? 16 : l.vat_treatment === 'ZeroRated' ? 0 : 0,
        account_code: l.account_code || '7900',
        dimensions: l.dimensions || {},
      }));
      setForm({
        vendor_id: bill.vendor_id || '',
        issue_date: bill.issue_date || today,
        due_date: bill.due_date || '',
        vendor_invoice_number: bill.vendor_invoice_number || '',
        notes: bill.notes || '',
        currency: bill.currency || 'KES',
        fx_rate: bill.fx_rate != null ? String(bill.fx_rate) : '1',
        lines: lines.length > 0 ? lines : [emptyLine()],
      });
    }
  }, [existingBill, isEdit]);

  const mutation = useMutation({
    mutationFn: (data: any) => isEdit ? updateBill(editId!, data) : createBill(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bills'] });
      if (isEdit) queryClient.invalidateQueries({ queryKey: ['bill', editId] });
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
        // leave account_code blank so the posting-group matrix can derive it.
        lines[i].account_code = '';
        lines[i].tax_rate = product.vat_treatment === 'Standard16' ? 16 : product.vat_treatment === 'ZeroRated' ? 0 : 0;
      }
    }
    setForm({ ...form, lines });
  };

  const removeLine = (i: number) => {
    if (form.lines.length === 1) return;
    setForm({ ...form, lines: form.lines.filter((_, idx) => idx !== i) });
  };

  const applyProductToLine = (i: number, p: QuickProduct) => {
    const lines = [...form.lines];
    lines[i] = {
      ...lines[i],
      product_id: p.id,
      description: lines[i].description || p.name,
      unit_price: p.unit_price,
      account_code: '',
      tax_rate: p.vat_treatment === 'Standard16' ? 16 : p.vat_treatment === 'ZeroRated' ? 0 : 0,
    };
    setForm({ ...form, lines });
  };

  // Calculations
  const subtotal = form.lines.reduce((sum, l) => sum + l.quantity * l.unit_price, 0);
  const taxByRate: Record<number, number> = {};
  form.lines.forEach(l => {
    const lineTotal = l.quantity * l.unit_price;
    const tax = lineTotal * l.tax_rate / 100;
    taxByRate[l.tax_rate] = (taxByRate[l.tax_rate] || 0) + tax;
  });
  const totalTax = Object.values(taxByRate).reduce((a, b) => a + b, 0);
  const grandTotal = subtotal + totalTax;

  const selectedVendor = vendors.find(v => v.id === form.vendor_id);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const defaultAccount = selectedVendor?.default_expense_account || '7900';
    mutation.mutate({
      vendor_id: form.vendor_id,
      issue_date: form.issue_date,
      due_date: form.due_date || undefined,
      vendor_invoice_number: form.vendor_invoice_number || undefined,
      currency: form.currency,
      fx_rate: Number(form.fx_rate) || 1,
      notes: form.notes || undefined,
      lines: form.lines.map(l => ({
        product_id: l.product_id || undefined,
        description: l.description,
        quantity: l.quantity,
        unit_price: l.unit_price,
        // product lines: blank → matrix derives the purchase account; manual
        // (product-less) lines fall back to the vendor/default expense account.
        account_code: l.account_code || (l.product_id ? undefined : defaultAccount),
        vat_treatment: l.tax_rate === 16 ? 'Standard16' : l.tax_rate === 0 ? 'ZeroRated' : 'Exempt',
        dimensions: l.dimensions && Object.keys(l.dimensions).length ? l.dimensions : undefined,
      })),
    });
  };

  return (
    <Modal open={true} onClose={onClose} title={isEdit ? "Edit Bill" : "New Bill"} size="xl">
      <form onSubmit={handleSubmit} className="space-y-6">
        {/* Header — Vendor + Dates */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Left — Vendor */}
          <div className="space-y-4">
            <div>
              <div className="flex items-center justify-between">
                <label className="label">Vendor *</label>
                <button type="button" onClick={() => setAddingVendor((v) => !v)} className="text-xs font-medium text-indigo-600 hover:text-indigo-800">
                  {addingVendor ? 'Cancel' : '+ New vendor'}
                </button>
              </div>
              <select
                className="input"
                value={form.vendor_id}
                onChange={(e) => setForm({ ...form, vendor_id: e.target.value })}
                required
              >
                <option value="">Choose a vendor...</option>
                {vendors.map(v => (
                  <option key={v.id} value={v.id}>{v.name}</option>
                ))}
              </select>
              {addingVendor && (
                <QuickAddParty
                  kind="vendor"
                  onCreated={(v) => { setForm((f) => ({ ...f, vendor_id: v.id })); setAddingVendor(false); }}
                  onCancel={() => setAddingVendor(false)}
                />
              )}
            </div>
            {selectedVendor && (
              <div className="bg-gray-50 rounded-lg p-3 text-sm text-gray-600">
                <p className="font-medium text-gray-900">{selectedVendor.name}</p>
                {selectedVendor.kra_pin && <p>PIN: {selectedVendor.kra_pin}</p>}
              </div>
            )}
          </div>

          {/* Right — Bill details */}
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="label">Issue Date</label>
                <input type="date" className="input" value={form.issue_date} onChange={(e) => setForm({ ...form, issue_date: e.target.value })} />
              </div>
              <div>
                <label className="label">Due Date</label>
                <input type="date" className="input" value={form.due_date} onChange={(e) => setForm({ ...form, due_date: e.target.value })} />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="label">Vendor Invoice #</label>
                <input className="input" value={form.vendor_invoice_number} onChange={(e) => setForm({ ...form, vendor_invoice_number: e.target.value })} placeholder="Optional" />
              </div>
              <div>
                <label className="label">Currency</label>
                <select className="input" value={form.currency} onChange={(e) => { fxTouched.current = false; setForm({ ...form, currency: e.target.value, fx_rate: e.target.value === baseCurrency ? '1' : form.fx_rate }); }}>
                  <option value="KES">KES - Kenya Shilling</option>
                  <option value="USD">USD - US Dollar</option>
                  <option value="EUR">EUR - Euro</option>
                  <option value="GBP">GBP - British Pound</option>
                </select>
              </div>
              {form.currency !== baseCurrency && (
                <div>
                  <label className="label">FX Rate → {baseCurrency}</label>
                  <input type="number" step="0.0001" className="input" value={form.fx_rate}
                    onChange={(e) => { fxTouched.current = true; setForm({ ...form, fx_rate: e.target.value }); }} />
                  <p className="text-xs text-gray-400 mt-1">1 {form.currency} = {form.fx_rate} {baseCurrency} · {form.lines.reduce((s, l) => s + l.quantity * l.unit_price, 0).toFixed(2)} {form.currency} ≈ {(form.lines.reduce((s, l) => s + l.quantity * l.unit_price, 0) * (Number(form.fx_rate) || 1)).toFixed(2)} {baseCurrency}</p>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Line Items */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="label mb-0">Line Items</label>
          </div>
          <div className="border rounded-lg overflow-hidden">
            {/* Table header */}
            <div className="grid grid-cols-12 gap-2 px-3 py-2 bg-gray-50 border-b text-xs font-medium text-gray-500 uppercase">
              <div className="col-span-3">Product / Service</div>
              <div className="col-span-3">Description</div>
              <div className="col-span-1">Qty</div>
              <div className="col-span-2">Unit Price</div>
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
                  <input className="input text-sm py-1.5" placeholder="Description" value={line.description} onChange={(e) => updateLine(i, 'description', e.target.value)} required />
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
          {/* Left — Notes + source document */}
          <div className="space-y-4">
            <div>
              <label className="label">Notes <span className="text-gray-400 font-normal">(internal)</span></label>
              <textarea className="input" rows={3} value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} placeholder="Internal notes..." />
            </div>
            {isEdit && editId ? (
              <Attachments linkedType="bill" linkedId={editId} label="Source document" />
            ) : (
              <p className="text-xs text-gray-400">Save the bill first, then re-open it to attach the source invoice.</p>
            )}
          </div>

          {/* Right — Totals */}
          <div className="bg-gray-50 rounded-lg p-4 space-y-2">
            <div className="flex justify-between text-sm">
              <span className="text-gray-600">Subtotal</span>
              <span className="font-medium">{formatCurrency(subtotal, form.currency)}</span>
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
            {form.currency !== 'KES' && (
              <div className="flex justify-between text-xs text-gray-500">
                <span>Equivalent in KES</span>
                <span>{formatCurrency(grandTotal * (Number(form.fx_rate) || 0), 'KES')} @ {form.fx_rate || '—'}</span>
              </div>
            )}
          </div>
        </div>

        {/* Footer actions */}
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !form.vendor_id}>
            {mutation.isPending ? 'Saving...' : isEdit ? 'Update Bill' : 'Create Bill'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function SupplierCreditNoteModal({ bill, onClose }: { bill: Bill; onClose: () => void }) {
  const queryClient = useQueryClient();
  const [reason, setReason] = useState('');

  const mutation = useMutation({
    mutationFn: (data: any) => createSupplierCreditNote(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['bills'] });
      queryClient.invalidateQueries({ queryKey: ['supplier-credit-notes'] });
      onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!reason.trim()) return;
    // Full reversal: create a supplier credit note referencing this bill with the
    // same gross_total. The backend resolves lines if empty (full reversal), or the
    // user can navigate to /supplier-credit-notes for partial line entry.
    mutation.mutate({
      vendor_id: bill.vendor_id,
      applies_to_bill: bill.id,
      reason: reason.trim(),
      lines: [], // empty = full reversal of the bill
    });
  };

  return (
    <Modal open={true} onClose={onClose} title={`Credit Note against ${bill.number}`} subtitle="Reverses this bill in the ledger (DR AP / CR Expense / CR VAT)" size="sm">
      <form onSubmit={handleSubmit} className="space-y-5">
        <div className="flex items-start gap-2 p-3 rounded-lg bg-amber-50 text-amber-700 text-sm">
          <ReceiptText className="w-4 h-4 shrink-0 mt-0.5" />
          <span>This creates a supplier credit note that fully reverses bill {bill.number} ({formatCurrency(bill.gross_total)}). For a partial credit, use the Supplier Credits page.</span>
        </div>
        <div>
          <label className="label">Reason *</label>
          <textarea
            className="input"
            rows={3}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            placeholder="e.g. Goods returned, pricing error, duplicate bill"
            required
          />
        </div>
        <div className="flex items-center justify-end pt-4 border-t gap-3">
          <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
          <button type="submit" className="btn-primary" disabled={mutation.isPending || !reason.trim()}>
            {mutation.isPending ? 'Creating...' : 'Issue Credit Note'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
