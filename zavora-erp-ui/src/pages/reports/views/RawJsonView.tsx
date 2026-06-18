// Fallback renderer for report content keys without a dedicated view
// (e.g. CashFlow, ArAgeing, ApAgeing) — preserves the original behaviour.
export default function RawJsonView({ c }: { c: any }) {
  return <pre className="text-xs bg-gray-50 p-4 rounded-lg overflow-auto max-h-96">{JSON.stringify(c, null, 2)}</pre>;
}
