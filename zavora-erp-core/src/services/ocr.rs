use chrono::Utc;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::parties::VendorRow;
use crate::payments::receipt_capture::*;
use crate::types::AgentOrUserId;

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

    // Step 2: Determine status based on confidence score
    // If confidence < 0.7: flag for mandatory human review
    let status = if result.confidence < 0.7 {
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

    // Only return a match if similarity exceeds threshold (0.6)
    match best_match {
        Some((id, score)) if score > 0.6 => Ok(Some(id)),
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
) -> ErpResult<Uuid> {
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

    // Build bill from OCR + adjustments
    let date = req
        .adjustments
        .as_ref()
        .and_then(|a| a.date)
        .or_else(|| ocr.as_ref().and_then(|o| o.date))
        .unwrap_or_else(|| Utc::now().date_naive());

    let total = req
        .adjustments
        .as_ref()
        .and_then(|a| a.total)
        .or_else(|| ocr.as_ref().and_then(|o| o.total))
        .unwrap_or_default();

    let description = req
        .adjustments
        .as_ref()
        .and_then(|a| a.description.clone())
        .or_else(|| ocr.as_ref().and_then(|o| o.vendor_name.clone()))
        .unwrap_or_else(|| "Receipt capture".to_string());

    let account_code = req.account_code.unwrap_or_else(|| "7900".to_string());

    // Create bill
    let bill_req = crate::ap::CreateBillRequest {
        vendor_id: req.vendor_id,
        vendor_invoice_number: None,
        issue_date: Some(date),
        due_date: None,
        currency: None,
        fx_rate: None,
        lines: vec![crate::invoicing::CreateInvoiceLineRequest {
            product_id: None,
            description: Some(description),
            quantity: rust_decimal::Decimal::ONE,
            unit_price: Some(total),
            discount_percent: None,
            account_code: Some(account_code),
            vat_treatment: None,
            dimensions: None,
        }],
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

    Ok(bill.id)
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
