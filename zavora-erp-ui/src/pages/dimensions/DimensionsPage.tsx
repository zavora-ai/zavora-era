import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getDimensions, createDimensionType, createDimensionValue } from '../../api/client';
import PageHeader from '../../components/shared/PageHeader';
import { Plus } from 'lucide-react';

export default function DimensionsPage() {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ['dimensions'], queryFn: getDimensions });
  const types: any[] = data?.data ?? [];

  const [typeCode, setTypeCode] = useState('');
  const [typeName, setTypeName] = useState('');
  const [valDraft, setValDraft] = useState<Record<string, { code: string; name: string }>>({});

  const invalidate = () => qc.invalidateQueries({ queryKey: ['dimensions'] });
  const addType = useMutation({ mutationFn: () => createDimensionType({ code: typeCode, name: typeName }), onSuccess: () => { setTypeCode(''); setTypeName(''); invalidate(); } });
  const addValue = useMutation({ mutationFn: (v: { type_code: string; code: string; name: string }) => createDimensionValue(v), onSuccess: invalidate });

  return (
    <div>
      <PageHeader title="Dimensions" subtitle="Analytical segments (cost centre, project, location…) for tagging transactions and dimensional reporting" />

      <div className="card p-4 mb-5 flex items-end gap-3">
        <div><label className="label">Type code</label><input className="input w-32" value={typeCode} onChange={(e) => setTypeCode(e.target.value)} placeholder="CC" /></div>
        <div><label className="label">Type name</label><input className="input w-56" value={typeName} onChange={(e) => setTypeName(e.target.value)} placeholder="Cost Centre" /></div>
        <button className="btn-primary" disabled={!typeCode || !typeName || addType.isPending} onClick={() => addType.mutate()}>
          <Plus className="w-4 h-4" /> Add dimension type
        </button>
      </div>

      <div className="space-y-4">
        {types.map((t) => {
          const draft = valDraft[t.code] ?? { code: '', name: '' };
          const setDraft = (d: { code: string; name: string }) => setValDraft((m) => ({ ...m, [t.code]: d }));
          return (
            <div key={t.code} className="card p-5">
              <h3 className="font-semibold text-gray-900 mb-2">{t.name} <span className="font-mono text-xs text-gray-400">{t.code}</span></h3>
              <table className="w-full text-sm mb-3">
                <thead><tr className="text-xs text-gray-500 uppercase border-b"><th className="text-left py-1.5">Value</th><th className="text-left">Name</th></tr></thead>
                <tbody>
                  {t.values.map((v: any) => (
                    <tr key={v.code} className="border-b border-gray-50"><td className="py-1.5 font-mono text-xs">{v.code}</td><td>{v.name}</td></tr>
                  ))}
                  {t.values.length === 0 && <tr><td colSpan={2} className="py-2 text-gray-400">No values yet</td></tr>}
                </tbody>
              </table>
              <div className="flex items-end gap-2">
                <div><label className="label">Value code</label><input className="input w-32" value={draft.code} onChange={(e) => setDraft({ ...draft, code: e.target.value })} placeholder="CC100" /></div>
                <div><label className="label">Value name</label><input className="input w-56" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} placeholder="Sales Dept" /></div>
                <button className="btn-secondary" disabled={!draft.code || !draft.name || addValue.isPending}
                  onClick={() => addValue.mutate({ type_code: t.code, code: draft.code, name: draft.name }, { onSuccess: () => setDraft({ code: '', name: '' }) })}>
                  <Plus className="w-4 h-4" /> Add value
                </button>
              </div>
            </div>
          );
        })}
        {types.length === 0 && <div className="card px-6 py-12 text-center text-sm text-gray-500">No dimensions yet. Add a type above to get started.</div>}
      </div>
    </div>
  );
}
