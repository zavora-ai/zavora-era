import { useState } from 'react';
import type { KeyboardEvent } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createCustomer, createVendor, createProduct } from '../../api/client';

/** Stop Enter inside a quick-add field from submitting the surrounding form. */
function swallowEnter(run: () => void) {
  return (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      run();
    }
  };
}

function Panel({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mt-2 rounded-lg border border-indigo-100 bg-indigo-50/40 p-3 space-y-2">
      <p className="text-xs font-semibold text-indigo-700">{title}</p>
      {children}
    </div>
  );
}

// ── Customer / Vendor ────────────────────────────────────────────────────────

export function QuickAddParty({
  kind,
  onCreated,
  onCancel,
}: {
  kind: 'customer' | 'vendor';
  onCreated: (party: { id: string; name: string }) => void;
  onCancel: () => void;
}) {
  const qc = useQueryClient();
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const create = kind === 'customer' ? createCustomer : createVendor;
  const listKey = kind === 'customer' ? 'customers' : 'vendors';

  const mutation = useMutation({
    mutationFn: () =>
      create({
        name: name.trim(),
        email: email.trim() ? [{ email: email.trim(), label: 'Main', is_primary: true }] : [],
        phone: [],
      } as any),
    onSuccess: (res) => {
      const record = {
        id: res.data.id,
        name: name.trim(),
        email: email.trim() ? [{ email: email.trim() }] : [],
        phone: [],
      };
      // Insert optimistically so the new option is immediately selectable, then
      // reconcile with the server.
      qc.setQueryData<any[]>([listKey], (old = []) => [...old, record]);
      qc.invalidateQueries({ queryKey: [listKey] });
      onCreated(record);
    },
  });

  const submit = () => {
    if (name.trim()) mutation.mutate();
  };

  return (
    <Panel title={`New ${kind}`}>
      <input
        className="input text-sm py-1.5"
        autoFocus
        placeholder={`${kind === 'customer' ? 'Customer' : 'Vendor'} name *`}
        value={name}
        onChange={(e) => setName(e.target.value)}
        onKeyDown={swallowEnter(submit)}
      />
      <input
        className="input text-sm py-1.5"
        type="email"
        placeholder="Email (optional)"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        onKeyDown={swallowEnter(submit)}
      />
      {mutation.isError && <p className="text-xs text-red-600">Could not create {kind}. Try again.</p>}
      <div className="flex gap-2">
        <button
          type="button"
          className="btn-primary text-xs py-1 px-3"
          disabled={!name.trim() || mutation.isPending}
          onClick={submit}
        >
          {mutation.isPending ? 'Adding…' : `Add ${kind}`}
        </button>
        <button type="button" className="btn-secondary text-xs py-1 px-3" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </Panel>
  );
}

// ── Product / Service ────────────────────────────────────────────────────────

export interface QuickProduct {
  id: string;
  name: string;
  unit_price: number;
  sales_account: string;
  purchase_account: string;
  vat_treatment: string;
}

export function QuickAddProduct({
  onCreated,
  onCancel,
}: {
  onCreated: (product: QuickProduct) => void;
  onCancel: () => void;
}) {
  const qc = useQueryClient();
  const [form, setForm] = useState({
    name: '',
    product_type: 'Service' as 'Service' | 'Goods' | 'Expense',
    unit_price: '',
    uom: 'Each',
    sales_account: '5100',
    purchase_account: '6000',
    vat_treatment: 'Standard16',
  });

  const mutation = useMutation({
    mutationFn: () =>
      createProduct({
        name: form.name.trim(),
        product_type: form.product_type,
        unit_price: form.unit_price ? parseFloat(form.unit_price) : undefined,
        uom: form.uom,
        sales_account: form.sales_account,
        purchase_account: form.purchase_account,
        vat_treatment: form.vat_treatment,
      }),
    onSuccess: (res) => {
      const price = form.unit_price ? parseFloat(form.unit_price) : 0;
      const record = {
        id: res.data.id,
        name: form.name.trim(),
        unit_price: price,
        sales_account: form.sales_account,
        purchase_account: form.purchase_account,
        vat_treatment: form.vat_treatment,
        product_type: form.product_type,
        uom: form.uom,
      };
      qc.setQueryData<any[]>(['products'], (old = []) => [...old, record]);
      qc.invalidateQueries({ queryKey: ['products'] });
      onCreated(record);
    },
  });

  const submit = () => {
    if (form.name.trim()) mutation.mutate();
  };

  return (
    <Panel title="New item">
      <div className="grid grid-cols-2 gap-2">
        <input
          className="input text-sm py-1.5 col-span-2"
          autoFocus
          placeholder="Item / service name *"
          value={form.name}
          onChange={(e) => setForm({ ...form, name: e.target.value })}
          onKeyDown={swallowEnter(submit)}
        />
        <select
          className="input text-sm py-1.5"
          value={form.product_type}
          onChange={(e) => {
            const t = e.target.value as 'Service' | 'Goods' | 'Expense';
            setForm({
              ...form,
              product_type: t,
              sales_account: t === 'Goods' ? '5000' : '5100',
              purchase_account: t === 'Expense' ? '7900' : '6000',
            });
          }}
        >
          <option value="Service">Service</option>
          <option value="Goods">Goods</option>
          <option value="Expense">Expense</option>
        </select>
        <input
          className="input text-sm py-1.5"
          type="number"
          min="0"
          step="0.01"
          placeholder="Unit price"
          value={form.unit_price}
          onChange={(e) => setForm({ ...form, unit_price: e.target.value })}
          onKeyDown={swallowEnter(submit)}
        />
        <select
          className="input text-sm py-1.5"
          value={form.vat_treatment}
          onChange={(e) => setForm({ ...form, vat_treatment: e.target.value })}
        >
          <option value="Standard16">VAT 16%</option>
          <option value="ZeroRated">Zero rated</option>
          <option value="Exempt">Exempt</option>
        </select>
        <input
          className="input text-sm py-1.5 font-mono"
          placeholder="Income acct"
          value={form.sales_account}
          onChange={(e) => setForm({ ...form, sales_account: e.target.value })}
          onKeyDown={swallowEnter(submit)}
        />
      </div>
      {mutation.isError && <p className="text-xs text-red-600">Could not create item. Try again.</p>}
      <div className="flex gap-2">
        <button
          type="button"
          className="btn-primary text-xs py-1 px-3"
          disabled={!form.name.trim() || mutation.isPending}
          onClick={submit}
        >
          {mutation.isPending ? 'Adding…' : 'Add item'}
        </button>
        <button type="button" className="btn-secondary text-xs py-1 px-3" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </Panel>
  );
}
