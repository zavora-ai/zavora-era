import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { createCustomer, createVendor, createProduct } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { Upload, CheckCircle2, XCircle } from 'lucide-react';

// Minimal RFC-4180-ish CSV parser (handles quoted fields, commas, newlines).
function parseCsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [], field = '', q = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (q) {
      if (c === '"') { if (text[i + 1] === '"') { field += '"'; i++; } else q = false; }
      else field += c;
    } else if (c === '"') q = true;
    else if (c === ',') { row.push(field); field = ''; }
    else if (c === '\n') { row.push(field); rows.push(row); row = []; field = ''; }
    else if (c !== '\r') field += c;
  }
  if (field.length || row.length) { row.push(field); rows.push(row); }
  return rows.filter((r) => r.some((c) => c.trim() !== ''));
}

const bool = (v?: string) => ['true', 'yes', '1', 'y'].includes((v ?? '').trim().toLowerCase());

type TypeKey = 'customers' | 'vendors' | 'products';
const TYPES: Record<TypeKey, { label: string; headers: string; build: (r: any) => any; create: (d: any) => Promise<any> }> = {
  customers: {
    label: 'Customers',
    headers: 'name, email, phone, kra_pin, payment_terms, credit_limit, notes',
    build: (r) => ({
      name: r.name,
      kra_pin: r.kra_pin || undefined,
      email: r.email ? [{ email: r.email, label: 'Main', is_primary: true }] : [],
      phone: r.phone ? [{ number: r.phone, label: 'Main', is_primary: true, whatsapp_enabled: false }] : [],
      payment_terms: r.payment_terms || undefined,
      credit_limit: r.credit_limit ? Number(r.credit_limit) : undefined,
      notes: r.notes || undefined,
    }),
    create: createCustomer,
  },
  vendors: {
    label: 'Vendors',
    headers: 'name, email, phone, kra_pin, wht_category, resident, notes',
    build: (r) => ({
      name: r.name,
      kra_pin: r.kra_pin || undefined,
      email: r.email ? [{ email: r.email, label: 'Main', is_primary: true }] : [],
      phone: r.phone ? [{ number: r.phone, label: 'Main', is_primary: true, whatsapp_enabled: false }] : [],
      wht_category: r.wht_category || undefined,
      resident: r.resident ? bool(r.resident) : true,
      notes: r.notes || undefined,
    }),
    create: createVendor,
  },
  products: {
    label: 'Products',
    headers: 'name, description, product_type, unit_price, sales_account, purchase_account, vat_treatment',
    build: (r) => ({
      name: r.name,
      description: r.description || undefined,
      product_type: r.product_type || 'Service',
      unit_price: r.unit_price ? Number(r.unit_price) : undefined,
      sales_account: r.sales_account || undefined,
      purchase_account: r.purchase_account || undefined,
      vat_treatment: r.vat_treatment || undefined,
    }),
    create: createProduct,
  },
};

export default function ImportPage() {
  const qc = useQueryClient();
  const [type, setType] = useState<TypeKey>('customers');
  const [text, setText] = useState('');
  const [running, setRunning] = useState(false);
  const [results, setResults] = useState<{ row: number; name: string; ok: boolean; error?: string }[]>([]);

  const cfg = TYPES[type];

  const run = async () => {
    const grid = parseCsv(text);
    if (grid.length < 2) { setResults([{ row: 0, name: '', ok: false, error: 'Need a header row and at least one data row.' }]); return; }
    const headers = grid[0].map((h) => h.trim().toLowerCase());
    setRunning(true);
    setResults([]);
    const out: typeof results = [];
    for (let i = 1; i < grid.length; i++) {
      const obj: any = {};
      headers.forEach((h, idx) => { obj[h] = (grid[i][idx] ?? '').trim(); });
      const name = obj.name || `(row ${i})`;
      if (!obj.name) { out.push({ row: i, name, ok: false, error: 'Missing name' }); continue; }
      try {
        await cfg.create(cfg.build(obj));
        out.push({ row: i, name, ok: true });
      } catch (e: any) {
        out.push({ row: i, name, ok: false, error: e?.response?.data?.error ?? 'Failed' });
      }
      setResults([...out]);
    }
    setRunning(false);
    qc.invalidateQueries({ queryKey: [type] });
  };

  const okCount = results.filter((r) => r.ok).length;
  const errCount = results.filter((r) => !r.ok).length;

  return (
    <div>
      <PageHeader title="Import Data" subtitle="Bulk-create master records from CSV. The first row must be a header." />

      <div className="card p-4 mb-4 flex flex-wrap items-end gap-4">
        <div>
          <label className="label">Record type</label>
          <select className="input" value={type} onChange={(e) => { setType(e.target.value as TypeKey); setResults([]); }}>
            {Object.entries(TYPES).map(([k, v]) => <option key={k} value={k}>{v.label}</option>)}
          </select>
        </div>
        <div className="flex-1 text-xs text-gray-500">
          Expected columns: <span className="font-mono">{cfg.headers}</span> · only <span className="font-mono">name</span> is required.
        </div>
        <label className="btn-secondary cursor-pointer">
          Choose file
          <input type="file" accept=".csv,text/csv" className="hidden" onChange={async (e) => { const f = e.target.files?.[0]; if (f) setText(await f.text()); }} />
        </label>
        <button className="btn-primary" disabled={running || !text.trim()} onClick={run}>
          <Upload className="w-4 h-4" /> {running ? 'Importing…' : 'Import'}
        </button>
      </div>

      <textarea
        className="input w-full font-mono text-xs h-40 mb-4"
        placeholder={`${cfg.headers}\n...`}
        value={text}
        onChange={(e) => setText(e.target.value)}
      />

      {results.length > 0 && (
        <div className="card p-5">
          <p className="text-sm mb-3"><span className="text-green-700 font-medium">{okCount} imported</span>{errCount > 0 && <span className="text-red-700 font-medium"> · {errCount} failed</span>}</p>
          <table className="w-full text-sm">
            <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-2 w-16">Row</th><th className="text-left">Name</th><th className="text-left">Result</th></tr></thead>
            <tbody>
              {results.map((r) => (
                <tr key={r.row} className="border-b border-gray-50">
                  <td className="py-1.5 text-gray-400">{r.row}</td>
                  <td>{r.name}</td>
                  <td>{r.ok
                    ? <span className="inline-flex items-center gap-1 text-green-700"><CheckCircle2 className="w-3.5 h-3.5" /> OK</span>
                    : <span className="inline-flex items-center gap-1 text-red-600"><XCircle className="w-3.5 h-3.5" /> {r.error}</span>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
