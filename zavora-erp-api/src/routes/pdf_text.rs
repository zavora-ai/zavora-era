//! Local PDF text-layer extraction via Pdfium (the engine Chrome/PyMuPDF use).
//!
//! The majority of Kenyan bank statements and supplier invoices are *digitally
//! generated* PDFs with a real text layer — no OCR needed. This module pulls that
//! text in-process so the PDF import works offline, reserving the xberg OCR
//! sidecar for genuine scans (an empty text layer).
//!
//! Pdfium is bound at runtime through `dlopen`, so the crate builds with no native
//! dependency; the `libpdfium` dynamic library only needs to be present when the
//! server runs. Resolution order: `PDFIUM_LIB_PATH` (a file or a directory), then
//! the system library. If Pdfium can't be bound, extraction returns `None` and the
//! caller falls back to the OCR sidecar — the feature degrades, never crashes.

use pdfium_render::prelude::*;

/// Extract the concatenated text layer of a PDF. Returns `None` when Pdfium is
/// unavailable, the bytes aren't a readable PDF, or the text layer is empty (a
/// scanned PDF — the caller should then OCR it).
pub fn extract_pdf_text(bytes: &[u8]) -> Option<String> {
    let bindings = bind_pdfium()?;
    let pdfium = Pdfium::new(bindings);
    let doc = pdfium.load_pdf_from_byte_slice(bytes, None).ok()?;

    let mut out = String::new();
    for page in doc.pages().iter() {
        if let Ok(text) = page.text() {
            out.push_str(&text.all());
            out.push('\n');
        }
    }
    let has_content = out.chars().any(|c| c.is_alphanumeric());
    if has_content {
        Some(out)
    } else {
        None
    }
}

/// Bind the Pdfium library, trying an explicit path first, then the system lib.
fn bind_pdfium() -> Option<Box<dyn PdfiumLibraryBindings>> {
    if let Ok(path) = std::env::var("PDFIUM_LIB_PATH") {
        let path = path.trim();
        if !path.is_empty() {
            // Treat the value as a direct path to the dynamic library file…
            if let Ok(b) = Pdfium::bind_to_library(path) {
                return Some(b);
            }
            // …or as a directory containing the platform-named library.
            if let Ok(b) = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(path)) {
                return Some(b);
            }
            tracing::warn!(path, "PDFIUM_LIB_PATH set but Pdfium could not be bound; trying system library");
        }
    }
    match Pdfium::bind_to_system_library() {
        Ok(b) => Some(b),
        Err(e) => {
            tracing::warn!(error = %e, "Pdfium not available locally; PDF text extraction will fall back to the OCR sidecar");
            None
        }
    }
}
