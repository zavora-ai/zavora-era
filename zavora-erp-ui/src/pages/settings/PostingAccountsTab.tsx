import { useEffect, useMemo, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getAccounts, getSettings, updateSettings } from '../../api/client';
import type { Account, ErpConfig, PostingSetup } from '../../types';
import { Save } from 'lucide-react';

type FieldDef = { key: keyof PostingSetup; label: string };
type Group = { title: string; hint?: string; fields: FieldDef[] };

const GROUPS: Group[] = [
  {
    title: 'Receivables & Payables',
    fields: [
      { key: 'accounts_receivable', label: 'Accounts Receivable' },
      { key: 'accounts_payable', label: 'Accounts Payable' },
      {
        key: 'unapplied_payments',
        label: 'Unapplied Payments',
      },
    ],
  },
  {
    title: 'Tax',
    fields: [
      { key: 'vat_output', label: 'VAT Output (Payable)' },
      { key: 'vat_input', label: 'VAT Input (Claimable)' },
      { key: 'wht_payable', label: 'WHT Payable' },
    ],
  },
  {
    title: 'Foreign Exchange',
    fields: [
      { key: 'realised_fx_gain', label: 'Realised FX Gain' },
      { key: 'realised_fx_loss', label: 'Realised FX Loss' },
      { key: 'unrealised_fx_gain', label: 'Unrealised FX Gain' },
      { key: 'unrealised_fx_loss', label: 'Unrealised FX Loss' },
    ],
  },
  {
    title: 'Equity & Cash',
    fields: [
      { key: 'retained_earnings', label: 'Retained Earnings' },
      { key: 'default_bank', label: 'Default Bank / Cash' },
    ],
  },
  {
    title: 'Default Income & Expense',
    hint: 'Used when a line has no product or master-record account.',
    fields: [
      { key: 'default_sales', label: 'Default Sales' },
      { key: 'default_purchase', label: 'Default Purchase / COGS' },
      { key: 'default_expense', label: 'Default Expense' },
    ],
  },
  {
    title: 'Payroll',
    fields: [
      { key: 'salaries_expense', label: 'Salaries & Wages Expense' },
      { key: 'nssf_employer_expense', label: 'Employer NSSF Expense' },
      { key: 'housing_levy_employer_expense', label: 'Employer Housing Levy Expense' },
      { key: 'paye_payable', label: 'PAYE Payable' },
      { key: 'nssf_payable', label: 'NSSF Payable' },
      { key: 'sha_payable', label: 'SHA Payable' },
      { key: 'helb_payable', label: 'HELB Payable' },
      { key: 'housing_levy_payable', label: 'Housing Levy Payable' },
      { key: 'net_pay_payable', label: 'Net Pay Payable' },
    ],
  },
];

export default function PostingAccountsTab() {
  const queryClient = useQueryClient();
  const { data: config } = useQuery<ErpConfig>({
    queryKey: ['settings'],
    queryFn: () => getSettings().then((r) => r.data),
  });
  const { data: accounts } = useQuery<Account[]>({
    queryKey: ['accounts'],
    queryFn: () => getAccounts().then((r) => r.data),
  });

  const [form, setForm] = useState<PostingSetup | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  useEffect(() => {
    if (config?.posting && !form) setForm(config.posting);
  }, [config, form]);

  const mutation = useMutation({
    mutationFn: (posting: PostingSetup) => updateSettings({ posting }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['settings'] });
      setSavedAt(Date.now());
    },
  });

  // Postable accounts only (exclude control accounts, which cannot be posted to).
  const options = useMemo(
    () =>
      (accounts ?? [])
        .filter((a) => a.is_active && !a.is_control)
        .sort((a, b) => a.code.localeCompare(b.code)),
    [accounts],
  );

  const codeIsKnown = (code: string) => options.some((o) => o.code === code);

  if (!form) {
    return <p className="text-sm text-gray-500">Loading posting setup…</p>;
  }

  const set = (key: keyof PostingSetup, value: string) =>
    setForm((f) => (f ? { ...f, [key]: value } : f));

  return (
    <div className="space-y-6">
      <p className="text-sm text-gray-500">
        Map each accounting event to a GL account. These accounts are used automatically
        when posting invoices, bills, payments, payroll, FX, and year-end close.
      </p>

      {GROUPS.map((group) => (
        <div key={group.title}>
          <h4 className="text-sm font-semibold text-gray-900 mb-1">{group.title}</h4>
          {group.hint && <p className="text-xs text-gray-500 mb-3">{group.hint}</p>}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-3">
            {group.fields.map((field) => {
              const value = form[field.key];
              const unknown = value && !codeIsKnown(value);
              return (
                <div key={field.key}>
                  <label className="label">{field.label}</label>
                  <select
                    className="input"
                    value={value}
                    onChange={(e) => set(field.key, e.target.value)}
                  >
                    {/* Preserve an unknown/missing code so it isn't silently lost. */}
                    {unknown && (
                      <option value={value}>{value} — (not in chart of accounts)</option>
                    )}
                    {options.map((a) => (
                      <option key={a.code} value={a.code}>
                        {a.code} — {a.name}
                      </option>
                    ))}
                  </select>
                  {unknown && (
                    <p className="text-xs text-amber-600 mt-1">
                      Account {value} is not in the chart of accounts — pick a valid account.
                    </p>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      ))}

      <div className="pt-4 border-t flex items-center justify-end gap-3">
        {savedAt && !mutation.isPending && (
          <span className="text-sm text-green-600">Saved</span>
        )}
        {mutation.isError && (
          <span className="text-sm text-red-600">Save failed</span>
        )}
        <button
          className="btn-primary"
          disabled={mutation.isPending}
          onClick={() => form && mutation.mutate(form)}
        >
          <Save className="w-4 h-4" /> {mutation.isPending ? 'Saving…' : 'Save Posting Setup'}
        </button>
      </div>
    </div>
  );
}
