use chrono::Utc;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::parties::VendorRow;
use crate::payments::receipt_capture::*;
use crate::types::AgentOrUserId;

/// Overall OCR confidence at or above which a capture is auto-marked `reviewed`;
/// below it the capture is flagged `needs_review` for mandatory human review.
const CONFIDENCE_REVIEW_THRESHOLD: f32 = 0.7;

/// Minimum normalised name-similarity for an OCR-extracted vendor name to be
/// auto-matched to an existing vendor record.
const VENDOR_MATCH_THRESHOLD: f64 = 0.6;

/// Submit a receipt for OCR processing.
/// In production, this would call Azure AI Content Understanding.
/// Here we store the capture record and return it for manual review.
pub async fn capture_receipt(engine: &ErpEngine, entity_id: Uuid, req: CaptureReceiptRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"INSERT INTO receipt_captures
           (id, entity_id, image_url, status, captured_by, captured_at)
           VALUES ($1, $2, $3, 'pending', $4, $5)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&req.image_url)
    .bind(serde_json::to_value(&req.captured_by).unwrap_or_default())
    .bind(now)
    .execute(engine.pool())
    .await?;

    Ok(id)
}

/// Process OCR results (called after external OCR service returns).
///
/// This function:
/// 1. Extracts vendor_name, date, total, vat_amount, line_items with confidence from OCR result
/// 2. Attempts fuzzy vendor matching against existing vendor records
/// 3. If confidence < 0.7: sets status to "needs_review" (mandatory human review)
/// 4. If confidence >= 0.7: sets status to "reviewed"
/// 5. Stores the OCR result and suggested vendor on the capture record
/// 6. Records an audit event linking OCR result to capture
pub async fn process_ocr_result(
    engine: &ErpEngine,
    entity_id: Uuid,
    capture_id: Uuid,
    result: OcrResult,
) -> ErpResult<()> {
    // Step 1: Attempt vendor matching via fuzzy match on extracted vendor_name
    let suggested_vendor_id = if let Some(ref vendor_name) = result.vendor_name {
        fuzzy_match_vendor(engine, entity_id, vendor_name).await?
    } else {
        None
    };

    // Step 2: Determine status based on confidence score.
    let status = if result.confidence < CONFIDENCE_REVIEW_THRESHOLD {
        "needs_review"
    } else {
        "reviewed"
    };

    // Step 3: Update receipt_capture record with ocr_result, suggested vendor, and status
    sqlx::query(
        r#"UPDATE receipt_captures
           SET ocr_result = $1, suggested_vendor_id = $2, status = $3
           WHERE id = $4 AND entity_id = $5"#,
    )
    .bind(serde_json::to_value(&result).unwrap_or_default())
    .bind(suggested_vendor_id)
    .bind(status)
    .bind(capture_id)
    .bind(entity_id)
    .execute(engine.pool())
    .await?;

    // Step 4: Record audit event linking OCR result to capture
    let audit_event = serde_json::json!({
        "event_type": "ocr_completed",
        "object_type": "receipt_capture",
        "object_id": capture_id,
        "entity_id": entity_id,
        "timestamp": Utc::now(),
        "metadata": {
            "confidence": result.confidence,
            "status": status,
            "vendor_name_extracted": result.vendor_name,
            "suggested_vendor_id": suggested_vendor_id,
            "total_extracted": result.total,
            "vat_extracted": result.vat_amount,
            "date_extracted": result.date,
            "line_items_count": result.line_items.len(),
        }
    });

    let stream_key = format!("erp:audit:{}", entity_id);
    let mut redis_conn = engine.redis_conn().await;
    let _: Result<(), _> = redis::cmd("XADD")
        .arg(&stream_key)
        .arg("*")
        .arg("data")
        .arg(audit_event.to_string())
        .query_async(&mut redis_conn)
        .await;

    Ok(())
}

/// Return the suggested vendor (id + name) stored on a capture after OCR, for
/// display in the review UI. Returns `(None, None)` when no vendor was matched.
pub async fn suggested_vendor_for(
    engine: &ErpEngine,
    entity_id: Uuid,
    capture_id: Uuid,
) -> ErpResult<(Option<Uuid>, Option<String>)> {
    let row = sqlx::query_as::<_, (Option<Uuid>, Option<String>)>(
        r#"SELECT rc.suggested_vendor_id, v.name
           FROM receipt_captures rc
           LEFT JOIN vendors v ON v.id = rc.suggested_vendor_id
           WHERE rc.id = $1 AND rc.entity_id = $2"#,
    )
    .bind(capture_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?;
    Ok(row.unwrap_or((None, None)))
}

/// Attempt to fuzzy match an OCR-extracted vendor name to existing vendor records.
///
/// Uses normalized string similarity to find the best matching vendor.
/// Returns the vendor ID if a match is found with similarity > 0.6.
async fn fuzzy_match_vendor(engine: &ErpEngine, entity_id: Uuid, extracted_name: &str) -> ErpResult<Option<Uuid>> {
    // Fetch all active vendors for this entity
    let vendors = sqlx::query_as::<_, VendorRow>(
        "SELECT * FROM vendors WHERE entity_id = $1 AND is_active = true",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    if vendors.is_empty() {
        return Ok(None);
    }

    let extracted_lower = extracted_name.to_lowercase();
    let mut best_match: Option<(Uuid, f64)> = None;

    for vendor in &vendors {
        let vendor_lower = vendor.name.to_lowercase();
        let similarity = compute_similarity(&extracted_lower, &vendor_lower);

        if let Some((_, best_score)) = best_match {
            if similarity > best_score {
                best_match = Some((vendor.id, similarity));
            }
        } else if similarity > 0.0 {
            best_match = Some((vendor.id, similarity));
        }
    }

    // Only return a match if similarity exceeds the configured threshold.
    match best_match {
        Some((id, score)) if score > VENDOR_MATCH_THRESHOLD => Ok(Some(id)),
        _ => Ok(None),
    }
}

/// Compute normalized string similarity between two strings.
///
/// Uses a combination of:
/// - Containment check (one string contains the other)
/// - Normalized Levenshtein distance
///
/// Returns a value between 0.0 (no match) and 1.0 (exact match).
fn compute_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }

    // Check if one contains the other (common with OCR extractions like "ACME Ltd" vs "ACME")
    if a.contains(b) || b.contains(a) {
        let shorter = a.len().min(b.len()) as f64;
        let longer = a.len().max(b.len()) as f64;
        return shorter / longer;
    }

    // Normalized Levenshtein distance
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }

    let distance = levenshtein_distance(a, b);
    1.0 - (distance as f64 / max_len as f64)
}

/// Compute the Levenshtein edit distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // Use two rows for space efficiency
    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for i in 1..=a_len {
        curr_row[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr_row[j] = (prev_row[j] + 1)
                .min(curr_row[j - 1] + 1)
                .min(prev_row[j - 1] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// Confirm a receipt capture and create a bill from it.
pub async fn confirm_and_create_bill(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: ConfirmReceiptRequest,
    confirmed_by: &AgentOrUserId,
) -> ErpResult<crate::ap::Bill> {
    // Fetch the capture
    let capture_row = sqlx::query_as::<_, CaptureRow>(
        "SELECT id, entity_id, image_url, ocr_result, status FROM receipt_captures WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.capture_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "ReceiptCapture".to_string(),
        id: req.capture_id,
    })?;

    // Parse OCR result
    let ocr: Option<OcrResult> = capture_row
        .ocr_result
        .and_then(|v| serde_json::from_value(v).ok());

    let adj = req.adjustments.as_ref();

    // Resolve the confirmed values: adjustments (what the user reviewed) win
    // over the raw OCR guess.
    let date = adj
        .and_then(|a| a.date)
        .or_else(|| ocr.as_ref().and_then(|o| o.date))
        .unwrap_or_else(|| Utc::now().date_naive());

    let total = adj
        .and_then(|a| a.total)
        .or_else(|| ocr.as_ref().and_then(|o| o.total))
        .unwrap_or_default();

    let vat_amount = adj
        .and_then(|a| a.vat_amount)
        .or_else(|| ocr.as_ref().and_then(|o| o.vat_amount))
        .unwrap_or_default();

    let description = adj
        .and_then(|a| a.description.clone())
        .or_else(|| adj.and_then(|a| a.vendor_name.clone()))
        .or_else(|| ocr.as_ref().and_then(|o| o.vendor_name.clone()))
        .unwrap_or_else(|| "Receipt capture".to_string());

    // Optional override; when absent, lines fall back to the vendor's default
    // expense account (or the tenant posting setup) via resolve_bill_line.
    let account_code = req.account_code.clone();

    // Build the bill lines. Receipt amounts are VAT-INCLUSIVE, but the bill
    // engine ADDS VAT on top of unit_price for Standard16 lines. So we post the
    // NET amount (total - vat) as the line unit_price and let the engine
    // recompute the VAT — this reproduces the receipt's gross without
    // double-counting. When VAT is zero we mark the line OutOfScope so no VAT is
    // added at all.
    let lines = build_bill_lines(adj, total, vat_amount, &description, account_code.as_deref());

    // Create bill
    let bill_req = crate::ap::CreateBillRequest {
        vendor_id: req.vendor_id,
        vendor_invoice_number: None,
        issue_date: Some(date),
        due_date: None,
        currency: None,
        fx_rate: None,
        lines,
        notes: Some("Created from receipt capture".to_string()),
    };

    let bill = crate::services::bills::create_bill(engine, entity_id, bill_req, confirmed_by).await?;

    // Update capture record
    sqlx::query(
        "UPDATE receipt_captures SET status = 'posted', suggested_bill_id = $1, reviewed_at = $2 WHERE id = $3",
    )
    .bind(bill.id)
    .bind(Utc::now())
    .bind(req.capture_id)
    .execute(engine.pool())
    .await?;

    Ok(bill)
}

/// Build the bill lines for a confirmed receipt.
///
/// Receipt totals are **VAT-inclusive** and the bill engine adds VAT *on top* of
/// a line's `unit_price` for rated lines. The invariant: the posted bill's gross
/// must equal the receipt total exactly, at any tax rate.
///   * Receipt VAT ≈ 16% of net → one `Standard16` line at `unit_price = total -
///     vat`; the engine re-adds 16% → gross == `total`.
///   * Any other VAT rate (foreign/8%/etc.) → a net `OutOfScope` line plus a
///     separate `OutOfScope` "Tax / VAT" line; nothing is recomputed.
///   * No VAT → one `OutOfScope` line at `total`.
///
/// When the reviewer supplied explicit `line_items`, each becomes its own
/// `OutOfScope` line at its stated amount (already-net descriptive splits).
fn build_bill_lines(
    adj: Option<&ReceiptAdjustments>,
    total: rust_decimal::Decimal,
    vat_amount: rust_decimal::Decimal,
    description: &str,
    account_code: Option<&str>,
) -> Vec<crate::invoicing::CreateInvoiceLineRequest> {
    use rust_decimal::Decimal;

    let mk = |desc: String, unit_price: Decimal, vat: crate::types::VatTreatment| {
        crate::invoicing::CreateInvoiceLineRequest {
            product_id: None,
            description: Some(desc),
            quantity: Decimal::ONE,
            unit_price: Some(unit_price),
            discount_percent: None,
            account_code: account_code.map(|s| s.to_string()),
            vat_treatment: Some(vat),
            dimensions: None,
        }
    };

    // If the reviewer provided line items, post each as an already-net line.
    if let Some(items) = adj.and_then(|a| a.line_items.as_ref()) {
        let usable: Vec<_> = items
            .iter()
            .filter(|li| !li.description.trim().is_empty())
            .collect();
        if !usable.is_empty() {
            return usable
                .iter()
                .map(|li| {
                    let amount = li
                        .amount
                        .or_else(|| match (li.quantity, li.unit_price) {
                            (Some(q), Some(p)) => Some(q * p),
                            _ => li.unit_price,
                        })
                        .unwrap_or(Decimal::ZERO);
                    mk(li.description.clone(), amount, crate::types::VatTreatment::OutOfScope)
                })
                .collect();
        }
    }

    // Single line(s) derived from the VAT-inclusive receipt total. The cardinal
    // rule: the posted bill's gross MUST equal the receipt total exactly,
    // whatever the tax rate. The bill engine adds VAT *on top* of unit_price for
    // rated lines, so we only use a rated treatment when the receipt's VAT
    // actually matches that rate (within rounding) — e.g. Kenyan 16%. For any
    // other rate (foreign invoices, 8%, etc.) we post the net as an OutOfScope
    // line plus a separate OutOfScope "VAT/Tax" line, so nothing is re-computed
    // and the gross reconciles to the cent.
    if vat_amount > Decimal::ZERO && total > vat_amount {
        let net = total - vat_amount;
        let standard_vat = (net * crate::types::VatTreatment::Standard16.rate())
            .round_dp(2);
        if (standard_vat - vat_amount).abs() <= Decimal::new(1, 2) {
            // Receipt VAT ≈ 16% of net → let the engine re-add 16%.
            vec![mk(description.to_string(), net, crate::types::VatTreatment::Standard16)]
        } else {
            // Arbitrary tax rate → post net + tax as explicit out-of-scope lines.
            vec![
                mk(description.to_string(), net, crate::types::VatTreatment::OutOfScope),
                mk("Tax / VAT".to_string(), vat_amount, crate::types::VatTreatment::OutOfScope),
            ]
        }
    } else {
        vec![mk(description.to_string(), total, crate::types::VatTreatment::OutOfScope)]
    }
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct CaptureRow {
    id: Uuid,
    entity_id: Uuid,
    image_url: String,
    ocr_result: Option<serde_json::Value>,
    status: String,
}

#[cfg(test)]
mod tests {
    use super::build_bill_lines;
    use crate::types::VatTreatment;
    use rust_decimal::Decimal;

    /// Reproduce the bill engine's per-line gross: unit_price + VAT-on-top.
    fn line_gross(unit_price: Decimal, vat: &VatTreatment) -> Decimal {
        let line_total = unit_price; // qty 1, no discount
        let vat_amt = (line_total * vat.rate()).round_dp(2);
        line_total + vat_amt
    }

    fn total_gross(lines: &[crate::invoicing::CreateInvoiceLineRequest]) -> Decimal {
        lines
            .iter()
            .map(|l| line_gross(l.unit_price.unwrap(), l.vat_treatment.as_ref().unwrap()))
            .sum()
    }

    #[test]
    fn kenyan_16pct_receipt_uses_single_rated_line() {
        // 1160 incl. 160 VAT == 16% of 1000 net.
        let lines = build_bill_lines(None, Decimal::new(1160, 0), Decimal::new(160, 0), "Acme", None);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].vat_treatment, Some(VatTreatment::Standard16));
        assert_eq!(total_gross(&lines), Decimal::new(1160, 0));
    }

    #[test]
    fn foreign_tax_rate_reconciles_via_out_of_scope_lines() {
        // US invoice: total 162.37, tax 10.47 (~6.9%, NOT 16%). Must still total
        // exactly 162.37 — the regression that posted 176.20 before the fix.
        let total = Decimal::new(162_37, 2);
        let vat = Decimal::new(10_47, 2);
        let lines = build_bill_lines(None, total, vat, "StripesShop", None);
        assert_eq!(lines.len(), 2, "net + tax as out-of-scope lines");
        assert!(lines.iter().all(|l| l.vat_treatment == Some(VatTreatment::OutOfScope)));
        assert_eq!(total_gross(&lines), total);
    }

    #[test]
    fn no_vat_posts_single_out_of_scope_line() {
        let total = Decimal::new(500, 0);
        let lines = build_bill_lines(None, total, Decimal::ZERO, "Cash sale", None);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].vat_treatment, Some(VatTreatment::OutOfScope));
        assert_eq!(total_gross(&lines), total);
    }
}
