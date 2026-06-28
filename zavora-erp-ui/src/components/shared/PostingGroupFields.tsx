import { useQuery } from '@tanstack/react-query';
import { getPostingGroups } from '../../api/client';

interface Group { id: string; code: string; name: string }

/// Posting-group selectors for a master record. `scope='party'` shows business
/// groups (customers/vendors); `scope='product'` shows product groups. Leaving a
/// selector blank keeps the record's current/default group.
export function PostingGroupFields({
  scope, generalId, vatId, onGeneral, onVat,
}: {
  scope: 'party' | 'product';
  generalId: string;
  vatId: string;
  onGeneral: (v: string) => void;
  onVat: (v: string) => void;
}) {
  const { data } = useQuery<any>({ queryKey: ['posting-groups'], queryFn: () => getPostingGroups().then(r => r.data) });
  const general: Group[] = (scope === 'party' ? data?.general_business : data?.general_product) || [];
  const vat: Group[] = (scope === 'party' ? data?.vat_business : data?.vat_product) || [];

  return (
    <div>
      <label className="label">Posting Groups <span className="text-gray-400 font-normal">(derive GL accounts automatically)</span></label>
      <div className="grid grid-cols-2 gap-4">
        <select className="input" value={generalId} onChange={(e) => onGeneral(e.target.value)}>
          <option value="">{scope === 'party' ? 'Business group (default)' : 'Product group (default)'}</option>
          {general.map(g => <option key={g.id} value={g.id}>{g.code} · {g.name}</option>)}
        </select>
        <select className="input" value={vatId} onChange={(e) => onVat(e.target.value)}>
          <option value="">VAT group (default)</option>
          {vat.map(g => <option key={g.id} value={g.id}>{g.code} · {g.name}</option>)}
        </select>
      </div>
    </div>
  );
}
