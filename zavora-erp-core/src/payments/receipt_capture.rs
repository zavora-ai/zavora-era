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
    pub amount: Option<Decimal>,
}

/// OCR result from receipt scanning.
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

/// Manual adjustments to OCR-extracted data before posting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptAdjustments {
    pub date: Option<NaiveDate>,
    pub total: Option<Decimal>,
    pub vat_amount: Option<Decimal>,
    pub description: Option<String>,
}
