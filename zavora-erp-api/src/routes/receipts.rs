use axum::{extract::State, Json};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::auth::{require_role, AuthContext, ROLES_CREATE};
use crate::AppState;
use super::err_response;
use zavora_erp_core::payments::receipt_capture::*;
use zavora_erp_core::services::ocr;
use zavora_erp_core::AgentOrUserId;

/// Request body for POST /receipts/capture.
#[derive(Debug, Deserialize)]
pub struct CaptureRequest {
    pub image_url: String,
}

/// Request body for POST /receipts/confirm.
#[derive(Debug, Deserialize)]
pub struct ConfirmRequest {
    pub capture_id: Uuid,
    pub vendor_id: Uuid,
    pub account_code: Option<String>,
    pub adjustments: Option<ReceiptAdjustments>,
}

/// POST /receipts/capture
///
/// Accept an image upload (as image_url), store in receipt_captures with status "pending",
/// and trigger async OCR processing.
pub async fn capture(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CaptureRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "capture receipt").map_err(err_response)?;
    let entity_id = ctx.entity_id;

    let capture_req = CaptureReceiptRequest {
        image_url: req.image_url,
        captured_by: AgentOrUserId::User(ctx.user_id),
    };

    match ocr::capture_receipt(&state.engine, entity_id, capture_req).await {
        Ok(capture_id) => {
            // Trigger async OCR processing in background
            let state_clone = Arc::clone(&state);
            tokio::spawn(async move {
                let engine = &state_clone.engine;
                // In production this calls Azure AI Content Understanding.
                // For now we just log that OCR was triggered.
                tracing::info!("OCR processing triggered for capture {}", capture_id);
                // Simulate OCR result (in production, replace with actual OCR call)
                let result = OcrResult {
                    vendor_name: None,
                    vendor_pin: None,
                    date: None,
                    total: None,
                    vat_amount: None,
                    line_items: vec![],
                    confidence: 0.0,
                    raw_text: None,
                };
                if let Err(e) = ocr::process_ocr_result(engine, entity_id, capture_id, result).await {
                    tracing::error!("OCR processing failed for capture {}: {}", capture_id, e);
                }
            });

            Ok(Json(serde_json::json!({
                "capture_id": capture_id,
                "status": "pending"
            })))
        }
        Err(e) => Err(err_response(e)),
    }
}

/// POST /receipts/confirm
///
/// Accept capture_id, vendor_id, and manual adjustments. Create a bill from
/// OCR-extracted data and set capture status to "posted".
pub async fn confirm(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConfirmRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "confirm receipt").map_err(err_response)?;

    let confirm_req = ConfirmReceiptRequest {
        capture_id: req.capture_id,
        vendor_id: req.vendor_id,
        account_code: req.account_code,
        adjustments: req.adjustments,
    };

    let actor = AgentOrUserId::User(ctx.user_id);
    match ocr::confirm_and_create_bill(&state.engine, ctx.entity_id, confirm_req, &actor).await {
        Ok(bill_id) => Ok(Json(serde_json::json!({
            "bill_id": bill_id,
            "capture_status": "posted"
        }))),
        Err(e) => Err(err_response(e)),
    }
}
