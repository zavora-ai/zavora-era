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

// Export the rendered statement to PDF — dependency-free. We open a print window
// containing ONLY the report document (cloning the page's stylesheets so the
// layout is preserved) and trigger the browser's print-to-PDF. Unlike a plain
// window.print(), this excludes the app chrome (sidebar/header) so the output is
// a clean, single-purpose PDF the user saves via the print dialog's "Save as PDF".
export function exportDomAsPdf(title: string) {
  const node = document.getElementById('report-document');
  if (!node) return;

  // Copy the current document's styles (Tailwind/print CSS) into the new window.
  const styleTags = Array.from(document.querySelectorAll('style, link[rel="stylesheet"]'))
    .map((el) => el.outerHTML)
    .join('\n');

  const win = window.open('', '_blank', 'width=900,height=1200');
  if (!win) {
    // Pop-up blocked — fall back to printing the current page.
    window.print();
    return;
  }
  win.document.open();
  win.document.write(
    `<!doctype html><html><head><meta charset="utf-8"><title>${title}</title>${styleTags}` +
    `<style>@page{size:A4;margin:12mm;} body{background:#fff;margin:0;padding:0;} ` +
    `.no-print{display:none !important;} #report-document{border:none !important;box-shadow:none !important;max-width:100% !important;margin:0 !important;}</style>` +
    `</head><body>${node.outerHTML}</body></html>`
  );
  win.document.close();
  // Give the cloned stylesheets a moment to apply, then print.
  win.focus();
  setTimeout(() => {
    win.print();
    // Leave the window open so the user can choose "Save as PDF"; most browsers
    // close it after the dialog. Closing immediately can cancel the print on some.
  }, 350);
}
