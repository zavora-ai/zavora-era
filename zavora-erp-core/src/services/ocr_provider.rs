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
        currency: None,
        line_items: Vec::new(),
        confidence: 0.0,
        raw_text: None,
        vendor_name_confidence: None,
        date_confidence: None,
        total_confidence: None,
        vat_amount_confidence: None,
        currency_confidence: None,
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
///
/// The heuristics are built to tolerate the wide variety of real supplier
/// invoice layouts (Amazon Advertising US/EU, AWS, SaaS receipts, Kenyan VAT
/// invoices), where the same field appears under many different labels and
/// formats:
///
/// * **total** — label-priority scan. Tax-inclusive / "amount due" / "grand
///   total" labels outrank a bare "total", which outranks interim lines
///   ("subtotal", "campaign charges total", "total amount billed",
///   "adjustments total", "total vat"). When a strong total label carries no
///   amount on its own line (e.g. AWS lists "TOTAL AMOUNT" then the value
///   pages later), the next money-bearing line supplies the value. Falls back
///   to the largest detected amount.
/// * **currency** — explicit "Invoice Currency: EUR" wins, then the currency
///   token on the total line, then the first currency token anywhere.
/// * **vat** — a "vat"/"tax" line carrying a real amount (never a registration
///   line or a bare rate, and never the grand "total vat" interim line).
/// * **date** — prefers an "Invoice Date:"-labelled value over the first date
///   seen (invoices often print an earlier carry-forward/period date first),
///   accepting numeric, dashed, and month-name formats.
/// * **vendor** — the first company-like line, skipping `FROM`/`TO` markers and
///   generic headings/labels; a line immediately following a `From` marker is
///   preferred.
fn ocr_from_text_lines(lines: Vec<(String, f32)>, raw_text: Option<String>) -> OcrResult {
    use rust_decimal::Decimal;

    if lines.is_empty() {
        let mut r = empty_result();
        r.raw_text = raw_text;
        return r;
    }

    let overall = lines.iter().map(|(_, c)| c).sum::<f32>() / lines.len() as f32;

    let (vendor_name, vendor_conf) = detect_vendor(&lines);
    let (currency, currency_conf) = match detect_currency(&lines) {
        Some((c, conf)) => (Some(c), Some(conf)),
        None => (None, None),
    };

    // total / vat / date scan.
    let mut best_total: Option<(u8, Decimal, f32)> = None; // (priority, amount, conf)
    let mut pending_total: Option<(u8, f32)> = None; // strong label awaiting its value
    let mut vat: Option<(u8, Decimal, f32)> = None; // (priority, amount, conf)
    let mut max_amount: Option<(Decimal, f32)> = None;
    let mut date: Option<(chrono::NaiveDate, f32)> = None;
    let mut labelled_date: Option<(chrono::NaiveDate, f32)> = None;

    let consider = |best: &mut Option<(u8, Decimal, f32)>, prio: u8, amt: Decimal, conf: f32| {
        match best {
            None => *best = Some((prio, amt, conf)),
            Some((bp, ba, _)) => {
                if prio > *bp || (prio == *bp && amt > *ba) {
                    *best = Some((prio, amt, conf));
                }
            }
        }
    };

    for (text, conf) in &lines {
        let lower = text.to_lowercase();

        // --- date: keep the first one seen, but let a labelled invoice date win.
        if let Some(d) = parse_any_date(text) {
            if date.is_none() {
                date = Some((d, *conf));
            }
            if labelled_date.is_none()
                && (lower.contains("invoice date") || lower.contains("date of issue")
                    || lower.contains("issue date") || lower.contains("bill date"))
            {
                labelled_date = Some((d, *conf));
            }
        }

        let amount = parse_money(text);

        // --- total: label-priority with pending-value support.
        if let Some(prio) = total_priority(&lower) {
            if let Some(a) = amount {
                consider(&mut best_total, prio, a, *conf);
                pending_total = None;
            } else if prio >= 2 {
                // Strong total label with no value on its line; capture the next
                // money-bearing line (AWS-style "TOTAL AMOUNT" → "USD 16.24").
                pending_total = Some((prio, *conf));
            }
        } else if let Some(a) = amount {
            if let Some((prio, pconf)) = pending_total.take() {
                consider(&mut best_total, prio, a, pconf.min(*conf));
            }
        }

        // --- vat amount (independent of total).
        if let Some(a) = amount {
            if max_amount.map(|(m, _)| a > m).unwrap_or(true) {
                max_amount = Some((a, *conf));
            }
            if let Some(prio) = vat_priority(&lower) {
                match &vat {
                    None => vat = Some((prio, a, *conf)),
                    Some((bp, _, _)) if prio > *bp => vat = Some((prio, a, *conf)),
                    _ => {}
                }
            }
        }
    }

    let total = best_total
        .map(|(_, a, c)| (a, c))
        .or(max_amount);
    let date = labelled_date.or(date);
    let vat = vat.map(|(_, a, c)| (a, c));

    OcrResult {
        vendor_name,
        vendor_pin: None,
        date: date.map(|(d, _)| d),
        total: total.map(|(t, _)| t),
        vat_amount: vat.map(|(v, _)| v),
        currency,
        line_items: Vec::<OcrLineItem>::new(),
        confidence: overall,
        raw_text,
        vendor_name_confidence: vendor_conf,
        date_confidence: date.map(|(_, c)| c),
        total_confidence: total.map(|(_, c)| c),
        vat_amount_confidence: vat.map(|(_, c)| c),
        currency_confidence: currency_conf,
    }
}

/// Priority of a "total"-like label line, or `None` when the line is not a
/// grand-total label. Higher wins. Interim/aggregate lines that are *not* the
/// payable grand total are excluded outright.
fn total_priority(lower: &str) -> Option<u8> {
    // Interim or component lines that must never be taken as the grand total.
    const EXCLUDE: [&str; 19] = [
        "subtotal", "sub total", "campaign charges total", "total campaign",
        "total amount billed", "adjustments total", "total adjustments",
        "total vat", "total tax",
        // French interim/component lines.
        "total frais", "total du portefeuille", "total ajustements",
        "total des ajustements", "montant total facturé", "montant total facture",
        "total autres frais",
        // Spanish interim/component lines.
        "cantidad total facturada", "total de ajustes", "total de ajuste",
    ];
    if EXCLUDE.iter().any(|e| lower.contains(e)) {
        return None;
    }
    // Strongest: an explicitly payable / tax-inclusive grand total.
    if lower.contains("tax inclusive")
        || lower.contains("tax included")
        || lower.contains("incluant les taxes")
        || lower.contains("impuestos incluidos")
        || lower.contains("amount due")
        || lower.contains("balance due")
        || lower.contains("grand total")
        || lower.contains("total payable")
        || lower.contains("amount payable")
    {
        return Some(3);
    }
    // Strong: a "total amount" / "total due" / French "montant total" / Spanish
    // "importe total".
    if lower.contains("total amount") || lower.contains("total due")
        || lower.contains("montant total") || lower.contains("importe total")
    {
        return Some(2);
    }
    // Weak: a bare "total".
    if lower.contains("total") {
        return Some(1);
    }
    None
}

/// Priority of a "VAT/tax amount"-like line, or `None` when the line does not
/// carry a VAT amount. Higher wins. This must reject the many tax-adjacent
/// lines that are *not* the VAT charged: registration ids, bare rates, the
/// "(excluding tax)"/"net charges excl. tax" subtotal, the document heading
/// "TAX INVOICE", and the "Average CPC Amount (ex. Tax)" column header.
fn vat_priority(lower: &str) -> Option<u8> {
    // Hard exclusions: never a VAT *amount*.
    let is_reg = lower.contains("number") || lower.contains("reg")
        || lower.contains("pin") || lower.contains(" id") || lower.contains("no.")
        || lower.contains("registration") || lower.contains("numéro") || lower.contains("numero");
    let is_excl = lower.contains("excl") || lower.contains("ex. tax")
        || lower.contains("ex tax") || lower.contains("net charges")
        || lower.contains("subtotal");
    let is_heading = lower.contains("tax invoice") || lower.contains("total vat")
        || lower.contains("total tax");
    let is_rate_in_kes = lower.contains("in kes"); // "VAT in KES (1 USD = …)" reference line
    // A tax-inclusive *total* line mentions "tax"/"taxes" but is the grand
    // total, not the VAT amount ("Total Amount (tax included) …", French
    // "Montant total (incluant les taxes) …").
    let is_inclusive_total = lower.contains("included") || lower.contains("inclusive")
        || lower.contains("incluant") || lower.contains("total");
    // A bare tax/VAT *rate* line carries a percentage, not an amount
    // ("Taux de TVA (FR) %0.00", "VAT Rate - VAT 16%").
    let is_rate = lower.contains("rate") || lower.contains("taux");
    if is_reg || is_excl || is_heading || is_rate_in_kes || is_inclusive_total || is_rate {
        return None;
    }
    let mentions_vat = lower.contains("vat") || lower.contains("tax") || lower.contains("tva");
    if !mentions_vat {
        return None;
    }
    // Strong: an explicit "tax amount" / "vat amount" / "montant tva" line.
    if lower.contains("tax amount") || lower.contains("vat amount")
        || lower.contains("amount - vat") || lower.contains("montant tva")
        || lower.contains("montant de tva")
    {
        return Some(2);
    }
    // Weak: any other vat/tax line that still carries a money amount (e.g.
    // "VAT 16% 137.93").
    Some(1)
}

/// Detect the document currency. Prefers an explicit "Invoice Currency:" line,
/// then a currency token on a total/amount-due line, then the first currency
/// token anywhere in the document.
fn detect_currency(lines: &[(String, f32)]) -> Option<(String, f32)> {
    for (t, c) in lines {
        if t.to_lowercase().contains("currency") {
            if let Some(code) = currency_code_in(t) {
                return Some((code, *c));
            }
        }
    }
    for (t, c) in lines {
        let l = t.to_lowercase();
        if l.contains("total") || l.contains("amount due") || l.contains("balance due") {
            if let Some(code) = currency_code_in(t) {
                return Some((code, *c));
            }
        }
    }
    for (t, c) in lines {
        if let Some(code) = currency_code_in(t) {
            return Some((code, *c));
        }
    }
    None
}

/// Find an ISO currency code (USD/EUR/GBP/KES/…) as a whole word, or a currency
/// symbol ($/€/£), in a line. Returns the ISO code.
fn currency_code_in(text: &str) -> Option<String> {
    const CODES: [&str; 16] = [
        "USD", "EUR", "GBP", "KES", "AUD", "CAD", "CHF", "JPY", "INR", "AED",
        "MXN", "BRL", "USH", "TZS", "ZAR", "NGN",
    ];
    let upper = text.to_uppercase();
    // Common local abbreviations → ISO code.
    for (abbr, code) in [("KSH", "KES"), ("KSHS", "KES"), ("USHS", "USH"), ("R ", "ZAR")] {
        if let Some(pos) = upper.find(abbr) {
            let before = upper[..pos].chars().next_back();
            let after = upper[pos + abbr.len()..].chars().next();
            let bound_ok = before.map(|c| !c.is_alphabetic()).unwrap_or(true)
                && after.map(|c| !c.is_alphabetic()).unwrap_or(true);
            if bound_ok {
                return Some(code.to_string());
            }
        }
    }
    for code in CODES {
        let cu = code.to_uppercase();
        // Whole-word match: bounded by non-alphabetic chars.
        if let Some(pos) = upper.find(&cu) {
            let before = upper[..pos].chars().next_back();
            let after = upper[pos + cu.len()..].chars().next();
            let bound_ok = before.map(|c| !c.is_alphabetic()).unwrap_or(true)
                && after.map(|c| !c.is_alphabetic()).unwrap_or(true);
            if bound_ok {
                return Some(code.to_string());
            }
        }
    }
    if text.contains('€') {
        return Some("EUR".to_string());
    }
    if text.contains('£') {
        return Some("GBP".to_string());
    }
    if text.contains('$') {
        return Some("USD".to_string());
    }
    None
}

/// Pick the vendor/merchant name. Skips `FROM`/`TO` markers and generic
/// headings; prefers the line immediately after a `From` marker.
fn detect_vendor(lines: &[(String, f32)]) -> (Option<String>, Option<f32>) {
    // Prefer the first vendor candidate right after a "From"/"FROM TO" marker.
    for (i, (t, _)) in lines.iter().enumerate() {
        let l = t.trim().to_lowercase();
        if l == "from" || l == "from to" || l == "from:" || l.starts_with("from ") && l.len() < 8 {
            if let Some((cand, c)) = lines
                .iter()
                .skip(i + 1)
                .find(|(t, _)| is_vendor_candidate(t))
            {
                return (Some(clean_vendor(cand)), Some(*c));
            }
        }
    }
    // Otherwise the first company-like line.
    lines
        .iter()
        .find(|(t, _)| is_vendor_candidate(t))
        .map(|(t, c)| (Some(clean_vendor(t)), Some(*c)))
        .unwrap_or((None, None))
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
    const HEADINGS: [&str; 26] = [
        "invoice", "receipt", "statement", "bill to", "bill from", "page ",
        "date", "order", "customer", "description", "details", "subtotal",
        "amount", "tax ", "from to", "from", "to", "client", "payment",
        "campaign", "vat number", "tax number", "account number", "address",
        "po box", "attn",
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
///
/// Handles both decimal conventions: the dot form (`1,525.99`) and the European
/// comma form (`1.525,99` / `1,47`), normalising the latter to a dot so amounts
/// on Eurozone supplier invoices parse correctly.
fn parse_money(text: &str) -> Option<rust_decimal::Decimal> {
    use std::str::FromStr;

    let mut best: Option<rust_decimal::Decimal> = None;
    let mut current = String::new();
    let flush = |cur: &mut String, best: &mut Option<rust_decimal::Decimal>| {
        if cur.is_empty() {
            return;
        }
        let token = std::mem::take(cur);
        let cleaned = normalise_decimal(&token);
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

/// Normalise a numeric token to a dot-decimal string, removing thousands
/// separators. Decides the decimal convention from the *last* separator: if the
/// final `,` or `.` is followed by exactly 1–2 digits it is the decimal point,
/// and the other separator (if any) is the thousands grouping.
///
/// Examples: `1,525.99 → 1525.99`, `1.525,99 → 1525.99`, `1,47 → 1.47`,
/// `2.42 → 2.42`, `1,234 → 1234` (no decimal — comma is grouping).
fn normalise_decimal(token: &str) -> String {
    let last_dot = token.rfind('.');
    let last_comma = token.rfind(',');
    let decimal_pos = match (last_dot, last_comma) {
        (Some(d), Some(c)) => Some(d.max(c)),
        (Some(d), None) => Some(d),
        (None, Some(c)) => Some(c),
        (None, None) => None,
    };
    if let Some(pos) = decimal_pos {
        let frac = &token[pos + 1..];
        // Treat as a decimal separator only when 1–2 trailing digits follow;
        // otherwise it is a thousands separator (e.g. "1,234" / "1.234.567").
        if (1..=2).contains(&frac.len()) && frac.chars().all(|c| c.is_ascii_digit()) {
            let int_part: String = token[..pos].chars().filter(|c| c.is_ascii_digit()).collect();
            return format!("{int_part}.{frac}");
        }
    }
    // No decimal part: strip all separators.
    token.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Parse the first date-like value in `text`. Accepts a wide range of invoice
/// formats: numeric (`YYYY-MM-DD`, `DD/MM/YYYY`, `MM/DD/YYYY`, `DD-MM-YYYY`,
/// `DD.MM.YYYY`), the month-name forms `Mon DD, YYYY` / `Month DD, YYYY`
/// (scanned across tokens), and the dashed month-name form `DD-Mon-YYYY`
/// (e.g. `10-Feb-2025`, `02-Jan-2026`) as a single token.
fn parse_any_date(text: &str) -> Option<chrono::NaiveDate> {
    // Single-token formats (numeric and dashed month-name).
    for token in text.split(|c: char| c.is_whitespace() || c == ',') {
        let t = token.trim().trim_matches(|c: char| matches!(c, '(' | ')' | ':' | ';'));
        if t.len() < 8 {
            continue;
        }
        for fmt in [
            "%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y", "%d-%m-%Y", "%d.%m.%Y",
            "%d-%b-%Y", "%d-%B-%Y", "%Y/%m/%d",
        ] {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(t, fmt) {
                return Some(d);
            }
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

    // --- Broad variety: real supplier-invoice layouts (verbatim extractor text). ---

    fn rest(content: &str) -> OcrResult {
        ocr_from_xberg_rest(&json!({ "content": content, "detected_languages": ["eng"] }))
    }

    #[test]
    fn amazon_ads_eu_tax_inclusive_total_and_currency() {
        // Amazon Online Germany GmbH (EUR). Tax-inclusive grand total must win
        // over the interim "Total Amount Billed 1.96" and "Adjustments total".
        let content = "FROM TO\nAmazon Online Germany GmbH\nVAT Number: DE288084764\n\
            Total Amount EUR 2.42\nInvoice Number: CDKS7J9BX-3\nInvoice Date: 02-04-2025\n\
            Invoice Period: 02-03-2025 to 02-04-2025\nInvoice Currency: EUR\n\
            Campaign Charges total: 1.96 EUR\nTotal Amount Billed  1.96 EUR\n\
            Total Adjustments  0.46 EUR\nVAT Amount (KE)  0.00 EUR\n\
            Total Amount (Tax inclusive)  2.42 EUR\n";
        let r = rest(content);
        assert_eq!(r.total, Some(Decimal::new(2_42, 2)), "tax-inclusive grand total");
        assert_eq!(r.currency.as_deref(), Some("EUR"));
        assert_eq!(r.date, chrono::NaiveDate::from_ymd_opt(2025, 4, 2), "DD-MM-YYYY invoice date");
        assert_eq!(r.vendor_name.as_deref(), Some("Amazon Online Germany GmbH"));
    }

    #[test]
    fn amazon_ads_us_billed_amount_due_dashed_month_date() {
        // Amazon Advertising LLC (USD). Grand total is labelled "Billed Amount
        // Due 61.36 USD" with NO "total" word; must beat "Campaign Charges
        // total: 52.90". Date is "10-Feb-2025".
        let content = "From\nAmazon Advertising LLC\nPO Box 24651\nTax Number P052047506W\n\
            TAX INVOICE\nBilled Amount Due 61.36 USD\nInvoice Number: TRRS5G8T2-5\n\
            Invoice Date: 10-Feb-2025\nInvoice Currency: USD\nPayment Method: Credit Card\n\
            Campaign Charges total: 52.90 USD\nTotal Campaign charges 52.90 USD\n";
        let r = rest(content);
        assert_eq!(r.total, Some(Decimal::new(61_36, 2)), "amount due beats campaign total");
        assert_eq!(r.currency.as_deref(), Some("USD"));
        assert_eq!(r.date, chrono::NaiveDate::from_ymd_opt(2025, 2, 10), "DD-Mon-YYYY date");
        assert_eq!(r.vendor_name.as_deref(), Some("Amazon Advertising LLC"), "name after From marker");
    }

    #[test]
    fn aws_total_amount_value_on_following_line() {
        // AWS: "TOTAL AMOUNT" label, with the value "USD 16.24" appearing on a
        // later line. The pending-total mechanism must capture it. The earlier
        // "USD 247.96" charge line must not win.
        let content = "Invoice\nAccount number:\n971994957690\n\
            This Invoice is for the billing period January 1 - January 31, 20\n\
            Invoice Summary\nUSD 16.24AWS Service Charges\n\
            Invoice Number:\nInvoice Date:\nTOTAL AMOUNT\nTOTAL VAT\n\
            EUINKE25-12623\nFebruary 1, 2025\nUSD 16.24\nKES 289.52\n\
            25\nUSD 247.96\n-USD 233.96\nUSD 14.00\n";
        let r = rest(content);
        assert_eq!(r.total, Some(Decimal::new(16_24, 2)), "TOTAL AMOUNT → next money line");
        assert_eq!(r.currency.as_deref(), Some("USD"));
    }

    #[test]
    fn dashed_and_dotted_numeric_dates() {
        assert_eq!(parse_any_date("Invoice Date: 02-04-2025"), chrono::NaiveDate::from_ymd_opt(2025, 4, 2));
        assert_eq!(parse_any_date("Datum 31.12.2025"), chrono::NaiveDate::from_ymd_opt(2025, 12, 31));
        assert_eq!(parse_any_date("10-Feb-2025"), chrono::NaiveDate::from_ymd_opt(2025, 2, 10));
        assert_eq!(parse_any_date("02-Jan-2026"), chrono::NaiveDate::from_ymd_opt(2026, 1, 2));
        assert_eq!(parse_any_date("2025/04/02"), chrono::NaiveDate::from_ymd_opt(2025, 4, 2));
    }

    #[test]
    fn labelled_invoice_date_beats_earlier_carry_forward_date() {
        // A carry-forward adjustment date (02-03-2025) prints before the real
        // invoice date (02-04-2025); the labelled invoice date must win.
        let content = "Amazon Online Germany GmbH\n\
            02-03-2025 Carrying forward amount from invoice CDKS7J9BX-2\n\
            Total Amount (Tax inclusive)  2.42 EUR\nInvoice Date: 02-04-2025\n";
        let r = rest(content);
        assert_eq!(r.date, chrono::NaiveDate::from_ymd_opt(2025, 4, 2));
    }

    #[test]
    fn currency_symbol_and_code_detection() {
        assert_eq!(currency_code_in("GRAND TOTAL $162.37").as_deref(), Some("USD"));
        assert_eq!(currency_code_in("Total Amount EUR 2.42").as_deref(), Some("EUR"));
        assert_eq!(currency_code_in("Total 2.42 €").as_deref(), Some("EUR"));
        assert_eq!(currency_code_in("Ksh 99.00").as_deref(), Some("KES"));
        // A bare word that merely contains a code substring must not match.
        assert_eq!(currency_code_in("EUROPE office"), None);
        assert_eq!(currency_code_in("no currency here"), None);
    }

    #[test]
    fn total_priority_excludes_interim_lines() {
        assert_eq!(total_priority("subtotal $141.00"), None);
        assert_eq!(total_priority("campaign charges total: 52.90 usd"), None);
        assert_eq!(total_priority("total amount billed 1.96 eur"), None);
        assert_eq!(total_priority("total vat 0.00"), None);
        assert!(total_priority("total amount (tax inclusive) 2.42 eur").unwrap() >= 3);
        assert!(total_priority("billed amount due 61.36 usd").unwrap() >= 3);
        assert!(total_priority("total amount eur 2.42").unwrap() >= 2);
        assert_eq!(total_priority("total 100.00").unwrap(), 1);
    }

    #[test]
    fn vat_zero_rated_still_extracts_total() {
        // Non-VAT (0%) foreign ad invoice: VAT lines are 0.00 and must not be
        // mistaken for the grand total.
        let content = "Amazon Advertising LLC\nBilled Amount Due 24.50 USD\n\
            Invoice Currency: USD\nVAT Rate (DE) %0.00\nVAT Amount (KE)  0.00 USD\n";
        let r = rest(content);
        assert_eq!(r.total, Some(Decimal::new(24_50, 2)));
        assert_eq!(r.vat_amount, Some(Decimal::ZERO));
    }

    #[test]
    fn vat_amount_picks_charged_vat_not_subtotal_or_heading() {
        // TRRS5G8T2-5 real lines: VAT is charged (8.46), total 61.36. The
        // detector must pick "Tax Amount - VAT 8.46 USD", NOT "Subtotal
        // (excluding tax) 52.90", "TAX INVOICE", or the "(ex. Tax)" header.
        let content = "Amazon Advertising LLC\nTAX INVOICE\n\
            Average CPC Amount (ex. Tax)\nSubtotal (excluding tax) 52.90 USD\n\
            Tax Rate - VAT 16%\nTax Amount - VAT 8.46 USD\n\
            Total Amount (tax included) 61.36 USD\nInvoice Currency: USD\n";
        let r = rest(content);
        assert_eq!(r.total, Some(Decimal::new(61_36, 2)), "tax-included total");
        assert_eq!(r.vat_amount, Some(Decimal::new(8_46, 2)), "charged VAT, not subtotal");
    }

    #[test]
    fn aws_vat_picks_charged_amount_not_net_charges() {
        // AWS real lines: "Net Charges (excl. Tax) USD 14.00" must NOT be VAT;
        // "VAT - 16% USD 2.24" is the charged VAT.
        let content = "AWS Service Charges\nTOTAL AMOUNT\nUSD 16.24\n\
            Net Charges (After Credits/Discounts, excl. Tax)  USD 14.00\n\
            VAT - 16%  USD 2.24\nVAT in KES (1 USD = 129.24975 KES )  KES 289.52\n";
        let r = rest(content);
        assert_eq!(r.total, Some(Decimal::new(16_24, 2)));
        assert_eq!(r.vat_amount, Some(Decimal::new(2_24, 2)), "charged VAT, not net charges");
    }
    #[test]
    fn european_decimal_comma_amounts() {
        // Eurozone invoices print "1,47 EUR" (comma decimal) and "1.525,99".
        assert_eq!(normalise_decimal("1,47"), "1.47");
        assert_eq!(normalise_decimal("1.525,99"), "1525.99");
        assert_eq!(normalise_decimal("1,525.99"), "1525.99");
        assert_eq!(normalise_decimal("2.42"), "2.42");
        assert_eq!(normalise_decimal("1,234"), "1234"); // comma = grouping
        assert_eq!(parse_money("Billed Amount Due 1,47 EUR"), Some(Decimal::new(1_47, 2)));
        assert_eq!(parse_money("Total 1.525,99 EUR"), Some(Decimal::new(1525_99, 2)));
    }

    #[test]
    fn amazon_ads_aud_currency_and_total() {
        // ZTGMBTHM2-3: Australian ad invoice (AUD). Total tax-included 2.67,
        // currency AUD must be detected (not "?").
        let content = "Amazon Advertising Australia\n\
            Total Amount (tax included) 2.67 AUD\nInvoice Currency: AUD\n\
            Campaign Charges total: 2.30 AUD\nSubtotal (excluding tax) 2.30 AUD\n\
            Tax Amount - VAT 0.37 AUD\n";
        let r = rest(content);
        assert_eq!(r.total, Some(Decimal::new(2_67, 2)));
        assert_eq!(r.currency.as_deref(), Some("AUD"));
        assert_eq!(r.vat_amount, Some(Decimal::new(0_37, 2)));
    }

    #[test]
    fn french_invoice_montant_total() {
        // EQ5NLNK1Z-10: French ad invoice. "Montant total (incluant les taxes)
        // 1.05 EUR" is the grand total; the "Montant Total Facturé 0.36",
        // "Total des Ajustements 0.69", and "Total du portefeuille 0.36" interim
        // lines must not win.
        let content = "Amazon\nMontant Total 1.05 EUR\n\
            Total frais de campagnes: 0.36 EUR\nTotal du portefeuille 0.36 EUR\n\
            Total ajustements: 0.69 EUR\nMontant Total Facturé  0.36 EUR\n\
            Total des Ajustements  0.69 EUR\nMontant total (incluant les taxes)  1.05 EUR\n";
        let r = rest(content);
        assert_eq!(r.total, Some(Decimal::new(1_05, 2)), "French tax-inclusive total");
    }


    #[test]
    fn still_handles_simple_kenyan_vat_invoice() {
        // Regression: a plain local invoice with a clean grand total + VAT.
        let content = "ACME SUPPLIES LTD\nInvoice Date: 2026-03-14\n\
            Subtotal 862.07\nVAT 16% 137.93\nGRAND TOTAL 1,000.00\n";
        let r = rest(content);
        assert_eq!(r.total, Some(Decimal::new(1000_00, 2)));
        assert_eq!(r.vat_amount, Some(Decimal::new(137_93, 2)));
        assert_eq!(r.date, chrono::NaiveDate::from_ymd_opt(2026, 3, 14));
        assert_eq!(r.vendor_name.as_deref(), Some("ACME SUPPLIES LTD"));
    }
}
