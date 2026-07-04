const AMOS_URL = import.meta.env.VITE_AMOS_URL || 'http://localhost:8090';

/**
 * Amos lives in its own service (:8090); embedding it keeps the ERP shell
 * (sidebar + header) so the experience feels like one system. `embed=1`
 * tells the Amos UI to hide its standalone header.
 */
export default function AmosPage() {
  return (
    <div className="-m-6 h-[calc(100vh-3.5rem)]">
      <iframe
        src={`${AMOS_URL}/?embed=1`}
        title="Amos — AI Accountant"
        className="w-full h-full border-0 block"
        allow="microphone; autoplay; clipboard-write"
      />
    </div>
  );
}
