import { useRef, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getAttachments, uploadAttachment, getAttachment, deleteAttachment } from '../../api/client';
import { Paperclip, Upload, Trash2, FileText } from 'lucide-react';

type Meta = { id: string; filename: string; mime_type: string; size_bytes: number; uploaded_at: string };

/** Attach / list / view / delete source documents linked to a record. */
export default function Attachments({ linkedType, linkedId, label = 'Attachments' }: { linkedType: string; linkedId: string; label?: string }) {
  const qc = useQueryClient();
  const fileRef = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);

  const key = ['attachments', linkedType, linkedId];
  const { data: items = [], isLoading } = useQuery<Meta[]>({
    queryKey: key,
    queryFn: () => getAttachments(linkedType, linkedId).then((r) => (Array.isArray(r.data) ? r.data : [])),
  });

  const upload = useMutation({
    mutationFn: (file: File) => uploadAttachment(linkedType, linkedId, file),
    onSuccess: () => { setError(null); qc.invalidateQueries({ queryKey: key }); },
    onError: (e: any) => setError(e?.response?.data?.error || 'Upload failed'),
  });
  const remove = useMutation({
    mutationFn: (id: string) => deleteAttachment(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: key }),
  });

  const open = async (id: string) => {
    try {
      const { data } = await getAttachment(id);
      const w = window.open();
      if (w) w.document.write(`<iframe src="${data.data_url}" style="width:100%;height:100%;border:0" title="${data.filename}"></iframe>`);
    } catch { setError('Could not open the file'); }
  };

  const human = (n: number) => (n < 1024 ? `${n} B` : n < 1048576 ? `${(n / 1024).toFixed(0)} KB` : `${(n / 1048576).toFixed(1)} MB`);

  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <label className="label flex items-center gap-1.5"><Paperclip className="w-4 h-4" /> {label}</label>
        <button type="button" onClick={() => fileRef.current?.click()} disabled={upload.isPending} className="btn-secondary text-xs inline-flex items-center gap-1">
          <Upload className="w-3.5 h-3.5" /> {upload.isPending ? 'Uploading…' : 'Add file'}
        </button>
        <input ref={fileRef} type="file" accept="application/pdf,image/*,.pdf,.png,.jpg,.jpeg,.xlsx,.csv" className="hidden"
          onChange={(e) => { const f = e.target.files?.[0]; if (f) upload.mutate(f); e.target.value = ''; }} />
      </div>
      {error && <p className="text-xs text-red-600 mb-2">{error}</p>}
      {isLoading ? (
        <p className="text-xs text-gray-400">Loading…</p>
      ) : items.length === 0 ? (
        <p className="text-xs text-gray-400">No documents attached. Add the source invoice or receipt.</p>
      ) : (
        <ul className="divide-y border rounded-lg">
          {items.map((a) => (
            <li key={a.id} className="flex items-center gap-2 px-3 py-2 text-sm">
              <FileText className="w-4 h-4 text-gray-400 shrink-0" />
              <button type="button" onClick={() => open(a.id)} className="text-blue-600 hover:underline truncate flex-1 text-left" title={a.filename}>{a.filename}</button>
              <span className="text-xs text-gray-400">{human(a.size_bytes)}</span>
              <button type="button" onClick={() => { if (confirm(`Remove ${a.filename}?`)) remove.mutate(a.id); }} className="text-gray-400 hover:text-red-600" title="Remove">
                <Trash2 className="w-3.5 h-3.5" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
