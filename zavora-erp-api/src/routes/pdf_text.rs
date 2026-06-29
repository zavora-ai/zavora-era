//! Local PDF text-layer extraction — pure Rust, no native dependency.
//!
//! The majority of Kenyan bank statements and supplier invoices are *digitally
//! generated* PDFs with a real text layer (e.g. the Equity Bank app exports a
//! proper text layer, NOT a scan). We pull that text in-process with the
//! pure-Rust `pdf-extract` crate so the PDF import works offline and reserves the
//! xberg OCR sidecar for genuine scans (an empty text layer).
//!
//! Using a pure-Rust extractor (rather than binding `libpdfium` via dlopen)
//! removes the runtime native-library requirement entirely: previously, when
//! `libpdfium.dylib` was absent the code silently fell back to OCR, which
//! *scrambles* a clean text layer into column-major order and corrupts the
//! parsed rows. With the text layer read directly, digital statements extract
//! row-by-row exactly as laid out.

/// Extract the concatenated text layer of a PDF. Returns `None` when the bytes
/// aren't a readable PDF or the text layer is empty/whitespace (a scanned PDF —
/// the caller should then OCR it). Never panics.
pub fn extract_pdf_text(bytes: &[u8]) -> Option<String> {
    // `pdf-extract` can panic on some malformed PDFs; isolate it so a bad file
    // degrades to the OCR fallback rather than taking down the request.
    let result = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(bytes));
    let text = match result {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "pdf-extract could not read the PDF; falling back to the OCR sidecar");
            return None;
        }
        Err(_) => {
            tracing::warn!("pdf-extract panicked on the PDF; falling back to the OCR sidecar");
            return None;
        }
    };

    // A scanned PDF yields no real text layer — treat as "needs OCR".
    let has_content = text.chars().any(|c| c.is_alphanumeric());
    if has_content {
        Some(text)
    } else {
        None
    }
}
