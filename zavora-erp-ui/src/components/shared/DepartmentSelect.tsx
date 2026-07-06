import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { listDepartments, createDepartment } from '../../api/client';

/**
 * Reusable department lookup. Renders a dropdown of departments (from the
 * masters) with a "+ Add new department…" option that reveals an inline
 * create form — so a department can be added from any screen that uses it
 * without leaving the flow. Calls `onChange(department_id, department_name)`.
 */
export default function DepartmentSelect({
  value,
  onChange,
  className,
  byName = false,
}: {
  value?: string;
  onChange: (id: string, name: string) => void;
  className?: string;
  /** When true, options and value use the department NAME instead of its id
   *  (for screens that store the department as free text, e.g. requisitions). */
  byName?: boolean;
}) {
  const qc = useQueryClient();
  const { data: departments = [] } = useQuery<any[]>({
    queryKey: ['departments'],
    queryFn: () => listDepartments().then(r => r.data),
  });
  const [adding, setAdding] = useState(false);
  const [code, setCode] = useState('');
  const [name, setName] = useState('');
  const [err, setErr] = useState('');

  const create = useMutation({
    mutationFn: () => createDepartment({ code, name }),
    onSuccess: async () => {
      await qc.invalidateQueries({ queryKey: ['departments'] });
      const res = await listDepartments();
      const d = (res.data as any[]).find(x => x.code === code);
      if (d) onChange(byName ? d.name : d.id, d.name);
      setAdding(false); setCode(''); setName(''); setErr('');
    },
    onError: (e: any) => setErr(e?.response?.data?.error ?? 'Failed to add department'),
  });

  if (adding) {
    return (
      <div className={className}>
        <div className="flex items-center gap-2">
          <input className="input font-mono w-24" placeholder="Code" value={code} onChange={e => setCode(e.target.value.toUpperCase())} />
          <input className="input flex-1" placeholder="Department name" value={name} onChange={e => setName(e.target.value)} />
          <button type="button" className="btn-primary" disabled={!code || !name || create.isPending} onClick={() => { setErr(''); create.mutate(); }}>{create.isPending ? '…' : 'Add'}</button>
          <button type="button" className="btn-secondary" onClick={() => { setAdding(false); setErr(''); }}>Cancel</button>
        </div>
        {err && <p className="text-xs text-red-600 mt-1">{err}</p>}
      </div>
    );
  }

  return (
    <select
      className={`input ${className ?? ''}`}
      value={value ?? ''}
      onChange={e => {
        if (e.target.value === '__new__') { setAdding(true); return; }
        const d = (departments as any[]).find(x => (byName ? x.name : x.id) === e.target.value);
        onChange(e.target.value, d?.name ?? e.target.value);
      }}
    >
      <option value="">— None —</option>
      {(departments as any[]).map(d => <option key={d.id} value={byName ? d.name : d.id}>{d.name}</option>)}
      <option value="__new__">+ Add new department…</option>
    </select>
  );
}
