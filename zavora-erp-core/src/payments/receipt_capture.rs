use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::AgentOrUserId;

/// Status of a receipt capture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CaptureStatus {
    Pending,
    Processing,
    Reviewed,
    Posted,
    Rejected,
}

/// OCR-extracted line item from a receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrLineItem {
    pub description: String,
    pub quantity: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    /// Line amount. Accepts `total` as an alias on input so the review UI's
    /// `{ ..., total }` line items deserialize directly.
    #[serde(default, alias = "total")]
    pub amount: Option<Decimal>,
    /// Per-line extraction confidence in `[0,1]`. Defaults to `0.0` for older
    /// records that predate confidence capture.
    #[serde(default)]
    pub confidence: f32,
}

/// OCR result from receipt scanning.
///
/// `confidence` is the overall document confidence used to decide whether the
/// capture needs mandatory human review. The optional `*_confidence` fields
/// carry per-field confidence so the review UI can highlight exactly which
/// extracted values are uncertain; they default to the overall `confidence`
/// when a provider does not score fields individually.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub vendor_name: Option<String>,
    pub vendor_pin: Option<String>,
    pub date: Option<NaiveDate>,
    pub total: Option<Decimal>,
    pub vat_amount: Option<Decimal>,
    pub line_items: Vec<OcrLineItem>,
    pub confidence: f32,
    pub raw_text: Option<String>,
    // --- Per-field confidence (optional; default to overall `confidence`). ---
    #[serde(default)]
    pub vendor_name_confidence: Option<f32>,
    #[serde(default)]
    pub date_confidence: Option<f32>,
    #[serde(default)]
    pub total_confidence: Option<f32>,
    #[serde(default)]
    pub vat_amount_confidence: Option<f32>,
}

/// A receipt capture record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptCapture {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub image_url: String,
    pub ocr_result: Option<OcrResult>,
    pub suggested_vendor_id: Option<Uuid>,
    pub suggested_bill_id: Option<Uuid>,
    pub status: CaptureStatus,
    pub captured_by: AgentOrUserId,
    pub captured_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

/// Request to submit a receipt for OCR processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureReceiptRequest {
    pub image_url: String,
    pub captured_by: AgentOrUserId,
}

/// Request to confirm and post an OCR-captured receipt as a bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmReceiptRequest {
    pub capture_id: Uuid,
    pub vendor_id: Uuid,
    pub account_code: Option<String>,
    pub adjustments: Option<ReceiptAdjustments>,
}

/// Manual adjustments to OCR-extracted data before posting. The review UI sends
/// the (possibly corrected) field values back here so the posted bill reflects
/// exactly what the user confirmed — not the raw OCR guess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptAdjustments {
    pub date: Option<NaiveDate>,
    pub total: Option<Decimal>,
    pub vat_amount: Option<Decimal>,
    pub description: Option<String>,
    /// Corrected vendor name (free text); informational — the posted vendor is
    /// chosen by `vendor_id` on the confirm request.
    #[serde(default)]
    pub vendor_name: Option<String>,
    /// Corrected line items. When present and non-empty these become the bill
    /// lines; otherwise a single net line is posted from `total`/`vat_amount`.
    #[serde(default)]
    pub line_items: Option<Vec<OcrLineItem>>,
}
