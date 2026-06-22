import { useEffect, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getWhtRates, updateWhtRate } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { Save } from 'lucide-react';

// Rates are stored as fractions (0.05 = 5%); shown and edited as percentages.
export default function WhtRatesPage() {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ['wht-rates'], queryFn: getWhtRates });
  const rows: any[] = data?.data ?? [];

  const [edits, setEdits] = useState<Record<string, { resident: string; nonResident: string }>>({});
  useEffect(() => {
    const m: Record<string, { resident: string; nonResident: string }> = {};
    rows.forEach((r) => { m[r.category] = { resident: String(Number(r.resident_rate) * 100), nonResident: String(Number(r.non_resident_rate) * 100) }; });
    setEdits(m);
  }, [data]);

  const save = useMutation({
    mutationFn: (cat: string) => updateWhtRate({
      category: cat,
      resident_rate: Number(edits[cat].resident) / 100,
      non_resident_rate: Number(edits[cat].nonResident) / 100,
    }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['wht-rates'] }),
  });

  return (
    <div>
      <PageHeader title="Withholding Tax Rates" subtitle="The single source of truth for WHT rates used when posting bills. Edit to change a rate — no redeploy needed." />

      <div className="card p-5 max-w-2xl">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-xs text-gray-500 uppercase border-b">
              <th className="text-left py-2">Category</th>
              <th className="text-right">Resident %</th>
              <th className="text-right">Non-resident %</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const e = edits[r.category] ?? { resident: '', nonResident: '' };
              return (
                <tr key={r.category} className="border-b border-gray-50">
                  <td className="py-2 font-medium">{r.category}</td>
                  <td className="text-right">
                    <input type="number" step="0.01" className="input w-24 text-right" value={e.resident}
                      onChange={(ev) => setEdits((m) => ({ ...m, [r.category]: { ...e, resident: ev.target.value } }))} />
                  </td>
                  <td className="text-right">
                    <input type="number" step="0.01" className="input w-24 text-right" value={e.nonResident}
                      onChange={(ev) => setEdits((m) => ({ ...m, [r.category]: { ...e, nonResident: ev.target.value } }))} />
                  </td>
                  <td className="text-right pl-2">
                    <button className="btn-secondary text-xs py-1" disabled={save.isPending} onClick={() => save.mutate(r.category)}>
                      <Save className="w-3.5 h-3.5" /> Save
                    </button>
                  </td>
                </tr>
              );
            })}
            {rows.length === 0 && <tr><td colSpan={4} className="py-4 text-center text-gray-400">No WHT rates configured.</td></tr>}
          </tbody>
        </table>
      </div>
    </div>
  );
}
