import { useEffect, useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft, Download, Printer, Loader2 } from 'lucide-react';
import { getEstimateDocumentHtml, getEstimateDocumentPdf } from '../../api/client';

/// The estimate document is rendered server-side by the same renderer as
/// invoices, so the on-screen preview, the downloaded PDF, and the emailed PDF
/// are identical. We load the server HTML into an iframe; Download/Print fetch
/// the PDF (the server prints that exact HTML to PDF).
export default function EstimatePreview() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [html, setHtml] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState(false);

  useEffect(() => {
    let cancelled = false;
    if (!id) return;
    getEstimateDocumentHtml(id)
      .then((r) => { if (!cancelled) setHtml(typeof r.data === 'string' ? r.data : String(r.data)); })
      .catch((e) => { if (!cancelled) setError(e?.response?.data?.error || 'Failed to load document.'); });
    return () => { cancelled = true; };
  }, [id]);

  const download = async () => {
    if (!id) return;
    setDownloading(true);
    try {
      const r = await getEstimateDocumentPdf(id);
      const cd = (r.headers?.['content-disposition'] as string) || '';
      const match = /filename="?([^";]+)"?/.exec(cd);
      const filename = match ? match[1] : `estimate-${id}.pdf`;
      const url = URL.createObjectURL(new Blob([r.data], { type: 'application/pdf' }));
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 4000);
    } catch (e: any) {
      setError(e?.response?.data?.error || 'Failed to generate PDF.');
    } finally {
      setDownloading(false);
    }
  };

  const print = async () => {
    if (!id) return;
    try {
      const r = await getEstimateDocumentPdf(id);
      const url = URL.createObjectURL(new Blob([r.data], { type: 'application/pdf' }));
      window.open(url, '_blank');
      setTimeout(() => URL.revokeObjectURL(url), 60000);
    } catch (e: any) {
      setError(e?.response?.data?.error || 'Failed to open PDF.');
    }
  };

  return (
    <div className="p-6">
      <div className="flex items-center gap-2 mb-4">
        <button onClick={() => navigate(-1)} className="btn-secondary"><ArrowLeft className="w-4 h-4" /> Back</button>
        <button onClick={print} className="btn-secondary"><Printer className="w-4 h-4" /> Print</button>
        <button onClick={download} className="btn-primary" disabled={downloading}>
          {downloading ? <Loader2 className="w-4 h-4 animate-spin" /> : <Download className="w-4 h-4" />} Download PDF
        </button>
      </div>

      {error && <div className="bg-red-50 text-red-700 text-sm p-3 rounded-lg mb-4">{error}</div>}

      {html === null && !error ? (
        <div className="p-12 text-center text-gray-500">
          <div className="animate-spin w-8 h-8 border-2 border-blue-600 border-t-transparent rounded-full mx-auto" />
          <p className="mt-3 text-sm">Loading document…</p>
        </div>
      ) : html ? (
        <div className="mx-auto max-w-4xl border border-gray-200 rounded-lg overflow-hidden shadow-sm bg-white">
          <iframe
            title="Estimate document"
            srcDoc={html}
            className="w-full"
            style={{ height: '297mm', border: 'none', background: '#fff' }}
          />
        </div>
      ) : null}
    </div>
  );
}
