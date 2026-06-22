// Export helpers — moved verbatim from the original ReportsPage monolith.

const today = new Date().toISOString().split('T')[0];

export function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

// Serialize the rendered statement (tables) to an Excel-readable .xls workbook.
// Excel opens HTML tables natively, so this preserves layout with zero deps.
export function exportDomAsExcel(title: string) {
  const node = document.getElementById('report-document');
  if (!node) return;
  const tables = Array.from(node.querySelectorAll('table'));
  const body = tables.map((t) => t.outerHTML).join('<br/>');
  const html =
    `<html xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel" xmlns="http://www.w3.org/TR/REC-html40">` +
    `<head><meta charset="utf-8"><style>td,th{border:1px solid #ddd;padding:4px 8px;}</style></head>` +
    `<body><h3>${title}</h3>${body}</body></html>`;
  downloadBlob(new Blob([html], { type: 'application/vnd.ms-excel' }), `${title.replace(/\s+/g, '-')}-${today}.xls`);
}
