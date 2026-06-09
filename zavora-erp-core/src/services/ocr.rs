use chrono::Utc;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::payments::receipt_capture::*;
use crate::types::AgentOrUserId;

/// Submit a receipt for OCR processing.
/// In production, this would call Azure AI Content Understanding.
/// Here we store the capture record and return it for manual review.
pub async fn capture_receipt(engine: &ErpEngine, req: CaptureReceiptRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"INSERT INTO receipt_captures
           (id, entity_id, image_url, status, captured_by, captured_at)
           VALUES ($1, $2, $3, 'pending', $4, $5)"#,
    )
    .bind(id)
    .bind(engine.entity_id())
    .bind(&req.image_url)
    .bind(serde_json::to_value(&req.captured_by).unwrap_or_default())
    .bind(now)
    .execute(engine.pool())
    .await?;

    Ok(id)
}

/// Process OCR results (called after external OCR service returns).
/// Updates the capture record with extracted data.
pub async fn process_ocr_result(
    engine: &ErpEngine,
    capture_id: Uuid,
    result: OcrResult,
) -> ErpResult<()> {
    sqlx::query(
        r#"UPDATE receipt_captures
           SET ocr_result = $1, status = 'reviewed'
           WHERE id = $2 AND entity_id = $3"#,
    )
    .bind(serde_json::to_value(&result).unwrap_or_default())
    .bind(capture_id)
    .bind(engine.entity_id())
    .execute(engine.pool())
    .await?;

    Ok(())
}

/// Confirm a receipt capture and create a bill from it.
pub async fn confirm_and_create_bill(
    engine: &ErpEngine,
    req: ConfirmReceiptRequest,
    confirmed_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    // Fetch the capture
    let capture_row = sqlx::query_as::<_, CaptureRow>(
        "SELECT id, entity_id, image_url, ocr_result, status FROM receipt_captures WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.capture_id)
    .bind(engine.entity_id())
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
        }],
        notes: Some("Created from receipt capture".to_string()),
    };

    let bill = crate::services::bills::create_bill(engine, bill_req, confirmed_by).await?;

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
