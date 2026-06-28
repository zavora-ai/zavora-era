//! Pluggable OCR provider abstraction for receipt capture.
//!
//! Receipt OCR is intentionally decoupled from any single engine. The default
//! [`ManualReviewProvider`] performs no extraction at all — it returns an empty,
//! zero-confidence result so the whole capture → review → confirm → bill flow
//! works with no external infrastructure (the reviewer simply types the fields).
//!
//! Real extraction is provided out-of-process by an OCR **sidecar** (e.g.
//! `xberg serve`, configured with a local OCR backend so receipt images never
//! leave the deployment). The API crate implements that HTTP-backed provider and
//! selects it via environment configuration; if it is unset or unreachable the
//! system degrades gracefully to manual review — mirroring how the M-Pesa STK
//! Push path returns a clear, actionable state when its gateway is not
//! provisioned, rather than failing opaquely or fabricating data.
//!
//! This module also hosts [`ocr_from_xberg_structured`], the **pure** mapping
//! from xberg's `Structured` output (JSON with `elements[] { text, bbox,
//! confidence }`) into our [`OcrResult`]. Keeping the mapping pure (no I/O) lets
//! it be unit-tested without a running sidecar and shared by any transport.

use async_trait::async_trait;

use crate::error::ErpResult;
use crate::payments::receipt_capture::{OcrLineItem, OcrResult};

/// The raw bytes of an uploaded receipt plus its MIME type, handed to a provider
/// for extraction.
#[derive(Debug, Clone)]
pub struct OcrInput {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub filename: String,
}

/// A pluggable receipt-OCR backend.
///
/// Implementations must be cheap to hold in shared application state and safe to
/// call concurrently. They should never panic on malformed input — a provider
/// that cannot extract anything returns an empty, low-confidence [`OcrResult`]
/// so the capture falls through to mandatory human review.
#[async_trait]
pub trait OcrProvider: Send + Sync {
    /// Human-readable provider name (for logs / diagnostics).
    fn name(&self) -> &'static str;

    /// Extract structured fields from a receipt image/PDF.
    async fn extract(&self, input: &OcrInput) -> ErpResult<OcrResult>;
}

/// Default provider: performs no extraction. Returns an empty, zero-confidence
/// result so the reviewer enters every field by hand. Requires no external
/// service and is always available.
#[derive(Debug, Default, Clone)]
pub struct ManualReviewProvider;

#[async_trait]
impl OcrProvider for ManualReviewProvider {
    fn name(&self) -> &'static str {
        "manual_review"
    }

    async fn extract(&self, _input: &OcrInput) -> ErpResult<OcrResult> {
        Ok(empty_result())
    }
}

/// An empty, zero-confidence result — the safe baseline that forces human
/// review. Used by the manual provider and as a fallback when a sidecar is
/// unreachable.
pub fn empty_result() -> OcrResult {
    OcrResult {
        vendor_name: None,
        vendor_pin: None,
        date: None,
        total: None,
        vat_amount: None,
        line_items: Vec::new(),
        confidence: 0.0,
        raw_text: None,
        vendor_name_confidence: None,
        date_confidence: None,
        total_confidence: None,
        vat_amount_confidence: None,
    }
}

// ---------------------------------------------------------------------------
// xberg "Structured" output mapping (pure; unit-tested without a sidecar).
// ---------------------------------------------------------------------------

/// Map xberg's `Structured` JSON output into our [`OcrResult`].
///
/// xberg's Structured format is `{ "elements": [ { "text", "bbox",
/// "confidence" }, ... ], "content"?: "..." }`. xberg is a generic
/// document-intelligence engine, not a receipt parser, so this function applies
/// lightweight, receipt-oriented heuristics over the recognised text lines:
///
/// * **vendor name** — the first non-empty text line (receipts lead with the
///   merchant name).
/// * **total** — the amount on the line whose label mentions "total" (the last
///   such line wins, so "grand total" beats an interim "subtotal"); falls back
///   to the largest detected money amount.
/// * **VAT** — the amount on a line mentioning "vat"/"tax".
/// * **date** — the first token parseable as `YYYY-MM-DD`, `DD/MM/YYYY`, or
///   `MM/DD/YYYY`.
///
/// Per-field confidence is taken from the contributing element's confidence;
/// overall `confidence` is the mean of element confidences. The function is
/// deliberately conservative: anything it cannot find is left `None` so the
/// review UI flags it. It performs no I/O and never panics.
pub fn ocr_from_xberg_structured(value: &serde_json::Value) -> OcrResult {
    let elements = value
        .get("elements")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    // Collect (text, confidence) for each non-empty recognised line.
    let lines: Vec<(String, f32)> = elements
        .iter()
        .filter_map(|el| {
            let text = el.get("text").and_then(|t| t.as_str())?.trim().to_string();
            if text.is_empty() {
                return None;
            }
            let conf = el
                .get("confidence")
                .and_then(|c| c.as_f64())
                .map(|c| c as f32)
                .unwrap_or(0.0);
            Some((text, conf))
        })
        .collect();

    let raw_text = value.get("content").and_then(|c| c.as_str()).map(|s| s.to_string());

    if lines.is_empty() {
        let mut r = empty_result();
        r.raw_text = raw_text;
        return r;
    }
    ocr_from_text_lines(lines, raw_text)
}

/// Map xberg's **REST `/extract`** response into an [`OcrResult`].
///
/// The HTTP server returns recognised text in `results[].content` (plain text,
/// one item per uploaded file) rather than the per-element `Structured` shape.
/// We split that content into lines and apply the same receipt heuristics as
/// [`ocr_from_xberg_structured`]. The server does not attach per-line OCR
/// confidence over HTTP, so every line takes the document-level confidence
/// derived from `results[].detected_languages` presence (a recognised-language
/// result is treated as reasonably confident, 0.85; otherwise 0.5). The exact
/// value only drives the review UI's "needs review" highlighting — the user
/// confirms every posted value regardless.
pub fn ocr_from_xberg_rest(result: &serde_json::Value) -> OcrResult {
    let content = result.get("content").and_then(|c| c.as_str()).unwrap_or("");
    let has_lang = result
        .get("detected_languages")
        .and_then(|l| l.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let line_conf: f32 = if content.trim().is_empty() {
        0.0
    } else if has_lang {
        0.85
    } else {
        0.5
    };

    let lines: Vec<(String, f32)> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| (l.to_string(), line_conf))
        .collect();

    if lines.is_empty() {
        let mut r = empty_result();
        r.raw_text = Some(content.to_string());
        return r;
    }
    ocr_from_text_lines(lines, Some(content.to_string()))
}

/// Shared receipt heuristics over recognised `(text, confidence)` lines. Pure;
/// used by both the `Structured` (`elements[]`) and REST (`content`) mappers.
fn ocr_from_text_lines(lines: Vec<(String, f32)>, raw_text: Option<String>) -> OcrResult {
    use rust_decimal::Decimal;

    if lines.is_empty() {
        let mut r = empty_result();
        r.raw_text = raw_text;
        return r;
    }

    let overall = lines.iter().map(|(_, c)| c).sum::<f32>() / lines.len() as f32;

    // Vendor: the first line that looks like a company name — has letters, is not
    // a money/date line, and is not a generic document heading or a label
    // ("Invoice", "Bill to", "Page 1 of 2", …). Real merchant names lead the
    // document but sit just under such headings.
    let (vendor_name, vendor_conf) = lines
        .iter()
        .find(|(t, _)| is_vendor_candidate(t))
        .map(|(t, c)| (Some(clean_vendor(t)), Some(*c)))
        .unwrap_or((None, None));

    // Scan lines for total / vat / date.
    let mut total: Option<(Decimal, f32)> = None;
    let mut vat: Option<(Decimal, f32)> = None;
    let mut max_amount: Option<(Decimal, f32)> = None;
    let mut date: Option<(chrono::NaiveDate, f32)> = None;

    for (text, conf) in &lines {
        let lower = text.to_lowercase();

        if date.is_none() {
            if let Some(d) = parse_any_date(text) {
                date = Some((d, *conf));
            }
        }

        if let Some(amount) = parse_money(text) {
            if max_amount.map(|(m, _)| amount > m).unwrap_or(true) {
                max_amount = Some((amount, *conf));
            }
            // VAT amount: a "vat"/"tax" line carrying a real amount — but never a
            // registration line ("VAT number", "PIN", "Tax ID") or a bare rate
            // ("VAT (16%)"), whose digits are not money.
            let is_reg_line = lower.contains("number") || lower.contains("reg")
                || lower.contains("pin") || lower.contains(" id") || lower.contains("no.");
            if (lower.contains("vat") || lower.contains("tax")) && !is_reg_line && vat.is_none() {
                vat = Some((amount, *conf));
            }
            // "grand total"/"total" wins over an interim "subtotal".
            if lower.contains("total") && !lower.contains("subtotal") {
                total = Some((amount, *conf));
            }
        }
    }

    let total = total.or(max_amount);

    OcrResult {
        vendor_name,
        vendor_pin: None,
        date: date.map(|(d, _)| d),
        total: total.map(|(t, _)| t),
        vat_amount: vat.map(|(v, _)| v),
        line_items: Vec::<OcrLineItem>::new(),
        confidence: overall,
        raw_text,
        vendor_name_confidence: vendor_conf,
        date_confidence: date.map(|(_, c)| c),
        total_confidence: total.map(|(_, c)| c),
        vat_amount_confidence: vat.map(|(_, c)| c),
    }
}

/// True when a line could be a merchant/vendor name: it has letters, is not a
/// money or pure-number/id line, and is not a generic document heading or label.
fn is_vendor_candidate(t: &str) -> bool {
    let trimmed = t.trim();
    if trimmed.len() < 3 || parse_money(trimmed).is_some() {
        return false;
    }
    // Must contain at least two letters (skip "1", "$", dotted separators).
    if trimmed.chars().filter(|c| c.is_alphabetic()).count() < 2 {
        return false;
    }
    // Reject date lines ("Nov 30, 2025 …") and punctuation-heavy separator lines
    // (the dotted rules some invoices print between fields).
    if parse_any_date(trimmed).is_some() {
        return false;
    }
    let punct = trimmed.chars().filter(|c| matches!(c, '.' | '·' | '-' | '_' | '*')).count();
    if punct * 3 > trimmed.len() {
        return false;
    }
    let lower = trimmed.to_lowercase();
    // Generic headings / field labels that precede or surround the real name.
    const HEADINGS: [&str; 14] = [
        "invoice", "receipt", "statement", "bill to", "bill from", "page ",
        "date", "order", "customer", "description", "details", "subtotal",
        "amount", "tax ",
    ];
    if HEADINGS.iter().any(|h| lower.starts_with(h) || lower == h.trim()) {
        return false;
    }
    true
}

/// Strip common OCR noise from a candidate vendor-name line (leading symbols
/// like the logo glyph "%", surrounding whitespace).
fn clean_vendor(s: &str) -> String {
    s.trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim()
        .to_string()
}


/// Parse a monetary amount from a text line. Returns the last value that looks
/// like a **currency amount** — i.e. it has a decimal part (e.g. `8.00`,
/// `1,525.99`). Requiring the decimal is what keeps invoice numbers
/// (`5427409035`), VAT/PIN registration ids (`EU372063981`), quantities (`1`),
/// and percentages (`16`) from being misread as money on real invoices, which
/// is the single most common OCR extraction error.
fn parse_money(text: &str) -> Option<rust_decimal::Decimal> {
    use std::str::FromStr;

    let mut best: Option<rust_decimal::Decimal> = None;
    let mut current = String::new();
    let flush = |cur: &mut String, best: &mut Option<rust_decimal::Decimal>| {
        if cur.is_empty() {
            return;
        }
        let token = std::mem::take(cur);
        let cleaned = token.replace(',', "");
        // Must be a decimal amount: one dot with 1–2 trailing digits, and a
        // sane integer length (≤ 9 digits) so a long id with a stray dot can't
        // slip through.
        let mut parts = cleaned.split('.');
        let int_part = parts.next().unwrap_or("");
        let frac_part = parts.next();
        let extra = parts.next();
        let is_amount = extra.is_none()
            && matches!(frac_part, Some(f) if (1..=2).contains(&f.len()) && f.chars().all(|c| c.is_ascii_digit()))
            && !int_part.is_empty()
            && int_part.len() <= 9
            && int_part.chars().all(|c| c.is_ascii_digit());
        if is_amount {
            if let Ok(d) = rust_decimal::Decimal::from_str(&cleaned) {
                *best = Some(d);
            }
        }
    };

    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == ',' || ch == '.' {
            current.push(ch);
        } else {
            flush(&mut current, &mut best);
        }
    }
    flush(&mut current, &mut best);
    best
}

/// Parse the first date-like value in `text`. Accepts numeric formats
/// (`YYYY-MM-DD`, `DD/MM/YYYY`, `MM/DD/YYYY`) as single tokens, and the common
/// invoice month-name form `Mon DD, YYYY` (e.g. "Dec 11, 2020") scanned across
/// tokens.
fn parse_any_date(text: &str) -> Option<chrono::NaiveDate> {
    // Numeric single-token formats.
    for token in text.split(|c: char| c.is_whitespace() || c == ',') {
        let t = token.trim();
        if t.len() < 8 {
            continue;
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d") {
            return Some(d);
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%d/%m/%Y") {
            return Some(d);
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%m/%d/%Y") {
            return Some(d);
        }
    }

    // Month-name form: find a "Mon DD YYYY" / "Month DD, YYYY" run anywhere in
    // the line (commas stripped). e.g. "ORDER DATE Dec 11, 2020, 10:56:14".
    let toks: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.trim().is_empty())
        .collect();
    for w in toks.windows(3) {
        let candidate = format!("{} {} {}", w[0].trim(), w[1].trim(), w[2].trim());
        for fmt in ["%b %d %Y", "%B %d %Y"] {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(&candidate, fmt) {
                return Some(d);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use serde_json::json;

    fn el(text: &str, conf: f64) -> serde_json::Value {
        json!({ "text": text, "confidence": conf })
    }

    #[tokio::test]
    async fn manual_provider_returns_empty_low_confidence() {
        let p = ManualReviewProvider;
        let out = p
            .extract(&OcrInput { bytes: vec![], mime_type: "image/png".into(), filename: "r.png".into() })
            .await
            .unwrap();
        assert_eq!(out.confidence, 0.0);
        assert!(out.vendor_name.is_none());
        assert!(out.line_items.is_empty());
    }

    #[test]
    fn maps_vendor_total_vat_and_date() {
        let v = json!({
            "elements": [
                el("ACME SUPPLIES LTD", 0.98),
                el("Date: 2026-03-14", 0.91),
                el("Subtotal 862.07", 0.80),
                el("VAT 16% 137.93", 0.88),
                el("TOTAL 1,000.00", 0.95),
            ]
        });
        let r = ocr_from_xberg_structured(&v);
        assert_eq!(r.vendor_name.as_deref(), Some("ACME SUPPLIES LTD"));
        assert_eq!(r.total, Some(Decimal::new(1000_00, 2)));
        assert_eq!(r.vat_amount, Some(Decimal::new(137_93, 2)));
        assert_eq!(r.date, chrono::NaiveDate::from_ymd_opt(2026, 3, 14));
        // Per-field confidence flows through from the contributing elements.
        assert_eq!(r.total_confidence, Some(0.95));
        assert_eq!(r.vendor_name_confidence, Some(0.98));
        assert!(r.confidence > 0.0);
    }

    #[test]
    fn total_label_beats_subtotal_and_largest_fallback() {
        // No explicit "total" → fall back to the largest detected amount.
        let v = json!({
            "elements": [ el("Shop", 0.9), el("Item A 200.00", 0.9), el("Item B 50.00", 0.9) ]
        });
        let r = ocr_from_xberg_structured(&v);
        assert_eq!(r.total, Some(Decimal::new(200_00, 2)));
    }

    #[test]
    fn empty_elements_force_review() {
        let v = json!({ "elements": [], "content": "unreadable" });
        let r = ocr_from_xberg_structured(&v);
        assert_eq!(r.confidence, 0.0);
        assert!(r.total.is_none());
        assert_eq!(r.raw_text.as_deref(), Some("unreadable"));
    }

    #[test]
    fn parse_money_requires_a_decimal_amount() {
        assert_eq!(parse_money("TOTAL 1,234.56"), Some(Decimal::new(1234_56, 2)));
        assert_eq!(parse_money("Ksh 99.00"), Some(Decimal::new(99_00, 2)));
        assert_eq!(parse_money("no digits here"), None);
        // The bugs this guards against on real invoices: an invoice number, a
        // VAT/PIN registration id, a bare percentage, and a quantity must NOT be
        // read as money (they have no decimal part).
        assert_eq!(parse_money("Invoice number: 5427409035"), None);
        assert_eq!(parse_money("VAT number: EU372063981"), None);
        assert_eq!(parse_money("VAT (16%)"), None);
        assert_eq!(parse_money("Qty 1"), None);
    }

    #[test]
    fn rest_mapper_extracts_from_real_invoice_ocr() {
        // Verbatim Tesseract OCR output of the StripesShop sample invoice as
        // returned by `xberg serve` /extract (results[0].content).
        let content = "% StripesShop\n\nSales Invoice\n\nINVOICE NUMBER #9000000001\n\
            ORDER DATE Dec 11, 2020, 10:56:14\nAM\nCUSTOMER NAME\n\
            Endurance Watch SKU: 24-MGO1 1 $49.00\n\
            SUBTOTAL $141.00\n\
            DISCOUNT (EYHPAOMMT9O9FXDH, FREE SHIPPING ON ANY PURCHASE OVER $50) -$14.10\n\
            TAX $10.47\nSHIPPING & HANDLING $25.00\nGRAND TOTAL $162.37\n";
        let resp = json!({ "content": content, "detected_languages": ["eng"] });
        let r = ocr_from_xberg_rest(&resp);

        // Grand total wins over subtotal; tax captured; vendor is the first
        // real text line (logo glyph stripped).
        assert_eq!(r.total, Some(Decimal::new(162_37, 2)), "grand total");
        assert_eq!(r.vat_amount, Some(Decimal::new(10_47, 2)), "tax line");
        assert_eq!(r.vendor_name.as_deref(), Some("StripesShop"));
        // "Dec 11, 2020" month-name date is parsed.
        assert_eq!(r.date, chrono::NaiveDate::from_ymd_opt(2020, 12, 11));
        // Recognised-language result → reasonably confident lines.
        assert!(r.confidence > 0.7);
        assert!(r.raw_text.is_some());
    }

    #[test]
    fn rest_mapper_empty_content_forces_review() {
        let r = ocr_from_xberg_rest(&json!({ "content": "", "detected_languages": [] }));
        assert_eq!(r.confidence, 0.0);
        assert!(r.total.is_none());
    }
}
