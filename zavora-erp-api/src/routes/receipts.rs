use axum::{
    extract::{Multipart, State},
    Json,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::auth::{require_role, AuthContext, ROLES_CREATE};
use crate::AppState;
use super::err_response;
use axum::response::{IntoResponse, Response};
use zavora_erp_core::payments::receipt_capture::*;
use zavora_erp_core::services::ocr;
use zavora_erp_core::services::ocr_provider::OcrInput;
use zavora_erp_core::{AgentOrUserId, ErpError};

/// Map an `ErpError` to a concrete HTTP `Response` (the shared mapping returns
/// `impl IntoResponse`; multipart handlers need a named `Response` type).
fn er(e: ErpError) -> Response {
    err_response(e).into_response()
}

/// Maximum accepted receipt upload size (8 MiB). Receipts are photos/scans; this
/// caps both the multipart read and the stored data-URL to keep the row sane.
const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;

/// POST /receipts/capture
///
/// Accepts a `multipart/form-data` upload with a `file` field (image or PDF),
/// stores the image, runs the configured OCR provider **synchronously**, and
/// returns the extracted fields in the shape the review UI expects:
/// `{ capture_id, status, ocr_result }`.
///
/// OCR runs inline (not a background task) because the UI blocks on the result
/// to populate the review form. The provider itself degrades to an empty,
/// low-confidence result if a sidecar is unavailable, so this endpoint always
/// returns a usable review payload rather than failing the upload.
pub async fn capture(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    require_role(ROLES_CREATE, &ctx, "capture receipt").map_err(er)?;
    let entity_id = ctx.entity_id;

    // Read the `file` part.
    let mut bytes: Vec<u8> = Vec::new();
    let mut filename = "receipt".to_string();
    let mut mime_type = "application/octet-stream".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| er(ErpError::ValidationFailed { message: format!("invalid upload: {e}") }))?
    {
        if field.name() == Some("file") {
            if let Some(fname) = field.file_name() {
                filename = fname.to_string();
            }
            if let Some(ct) = field.content_type() {
                mime_type = ct.to_string();
            }
            let data = field.bytes().await.map_err(|e| {
                er(ErpError::ValidationFailed { message: format!("could not read file: {e}") })
            })?;
            bytes = data.to_vec();
        }
    }

    if bytes.is_empty() {
        return Err(er(ErpError::ValidationFailed {
            message: "no file provided (expected a 'file' part)".to_string(),
        }));
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(er(ErpError::ValidationFailed {
            message: format!("file too large (max {} MiB)", MAX_UPLOAD_BYTES / (1024 * 1024)),
        }));
    }

    // Store the image inline as a data URL in receipt_captures.image_url. This
    // avoids standing up object storage for the fast-follow feature while still
    // preserving the original for the review preview and audit.
    let image_url = format!("data:{};base64,{}", mime_type, B64.encode(&bytes));

    let capture_req = CaptureReceiptRequest {
        image_url,
        captured_by: AgentOrUserId::User(ctx.user_id),
    };

    let capture_id = ocr::capture_receipt(&state.engine, entity_id, capture_req)
        .await
        .map_err(er)?;

    // Run OCR synchronously. For digital PDFs (most supplier invoices), pull the
    // text layer locally with Pdfium and apply the receipt heuristics — no sidecar
    // needed. Fall back to the configured provider for images and scanned PDFs (an
    // empty local text layer).
    let is_pdf = mime_type.contains("pdf") || filename.to_lowercase().ends_with(".pdf");
    let local_pdf_text = if is_pdf { crate::routes::pdf_text::extract_pdf_text(&bytes) } else { None };
    let result = match local_pdf_text {
        Some(text) => zavora_erp_core::services::ocr_provider::ocr_from_xberg_rest(
            &serde_json::json!({ "content": text, "detected_languages": ["eng"] }),
        ),
        None => {
            let ocr_input = OcrInput { bytes, mime_type, filename };
            state
                .ocr
                .extract(&ocr_input)
                .await
                .unwrap_or_else(|_| zavora_erp_core::services::ocr_provider::empty_result())
        }
    };

    // Persist the result + vendor match + status, and audit it.
    ocr::process_ocr_result(&state.engine, entity_id, capture_id, result.clone())
        .await
        .map_err(er)?;

    // Re-resolve the suggested vendor name for display (process_ocr_result stored
    // the id; the UI wants the name too).
    let (suggested_vendor_id, suggested_vendor_name) =
        ocr::suggested_vendor_for(&state.engine, entity_id, capture_id)
            .await
            .unwrap_or((None, None));

    let status = if result.confidence < 0.7 { "needs_review" } else { "reviewed" };

    Ok(Json(serde_json::json!({
        "capture_id": capture_id,
        "status": status,
        "ocr_result": ocr_result_to_ui(&result, suggested_vendor_id, suggested_vendor_name),
    })))
}

/// Map the internal [`OcrResult`] into the exact JSON shape the React review
/// page binds to. All optionals collapse to `""`/`0`/empty (never `null`) so the
/// controlled inputs stay controlled; per-field confidence falls back to the
/// overall confidence when a provider does not score fields individually.
fn ocr_result_to_ui(
    r: &OcrResult,
    suggested_vendor_id: Option<Uuid>,
    suggested_vendor_name: Option<String>,
) -> serde_json::Value {
    let overall = r.confidence;
    let line_items: Vec<serde_json::Value> = r
        .line_items
        .iter()
        .map(|li| {
            let qty = li.quantity.unwrap_or(rust_decimal::Decimal::ONE);
            let unit = li.unit_price.unwrap_or(rust_decimal::Decimal::ZERO);
            let amount = li.amount.unwrap_or(qty * unit);
            serde_json::json!({
                "description": li.description,
                "quantity": qty,
                "unit_price": unit,
                "total": amount,
                "confidence": li.confidence,
            })
        })
        .collect();

    serde_json::json!({
        "vendor_name": r.vendor_name.clone().unwrap_or_default(),
        "vendor_name_confidence": r.vendor_name_confidence.unwrap_or(overall),
        "date": r.date.map(|d| d.to_string()).unwrap_or_default(),
        "date_confidence": r.date_confidence.unwrap_or(overall),
        "total": r.total.unwrap_or(rust_decimal::Decimal::ZERO),
        "total_confidence": r.total_confidence.unwrap_or(overall),
        "vat_amount": r.vat_amount.unwrap_or(rust_decimal::Decimal::ZERO),
        "vat_amount_confidence": r.vat_amount_confidence.unwrap_or(overall),
        "currency": r.currency.clone().unwrap_or_default(),
        "currency_confidence": r.currency_confidence.unwrap_or(overall),
        "line_items": line_items,
        "suggested_vendor_id": suggested_vendor_id,
        "suggested_vendor_name": suggested_vendor_name,
    })
}

/// Request body for POST /receipts/confirm.
#[derive(Debug, Deserialize)]
pub struct ConfirmRequest {
    pub capture_id: Uuid,
    pub vendor_id: Uuid,
    pub account_code: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub fx_rate: Option<rust_decimal::Decimal>,
    pub adjustments: Option<ReceiptAdjustments>,
}

/// POST /receipts/confirm
///
/// Accept capture_id, vendor_id, and reviewed adjustments. Create a bill from
/// the confirmed data and set the capture status to "posted".
pub async fn confirm(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConfirmRequest>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    require_role(ROLES_CREATE, &ctx, "confirm receipt").map_err(er)?;

    let confirm_req = ConfirmReceiptRequest {
        capture_id: req.capture_id,
        vendor_id: req.vendor_id,
        account_code: req.account_code,
        currency: req.currency,
        fx_rate: req.fx_rate,
        adjustments: req.adjustments,
    };

    let actor = AgentOrUserId::User(ctx.user_id);
    let bill = ocr::confirm_and_create_bill(&state.engine, ctx.entity_id, confirm_req, &actor)
        .await
        .map_err(er)?;

    Ok(Json(serde_json::json!({
        "bill_id": bill.id,
        "bill_number": bill.number,
        "capture_status": "posted",
    })))
}
