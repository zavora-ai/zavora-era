//! Procurement (P2P) domain — tenders/RFQ, bids, purchase orders (LPO), and the
//! external vendor-portal principal (`vendor_users`).
//!
//! Row structs are served directly by the API (status kept as `String`, like
//! `ap::BillRow`); request structs are the create/action payloads. Business
//! logic and state transitions live in `services::procurement`.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ── Vendor users (portal principals) ────────────────────────────────────────

/// A vendor-portal login. Public projection — never includes `password_hash`.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct VendorUserRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub company_name: String,
    pub kra_pin: Option<String>,
    pub phone: Option<String>,
    pub status: String, // pending|active|suspended|rejected
    pub vendor_id: Option<Uuid>,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterVendorRequest {
    pub company_name: String,
    pub display_name: String,
    pub email: String,
    pub password: String,
    pub kra_pin: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VendorLoginRequest {
    pub email: String,
    pub password: String,
}

/// Buyer approving a registration: optionally link an existing vendor master,
/// otherwise a new one is created from the registration details.
#[derive(Debug, Clone, Deserialize)]
pub struct ApproveVendorRequest {
    #[serde(default)]
    pub vendor_id: Option<Uuid>,
}

// ── Tenders / RFQ ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TenderRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub closing_date: Option<NaiveDate>,
    pub status: String, // draft|open|closed|awarded|cancelled
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TenderLineRow {
    pub id: Uuid,
    pub tender_id: Uuid,
    pub description: String,
    pub quantity: Decimal,
    pub uom: String,
    pub line_no: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTenderRequest {
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub closing_date: Option<NaiveDate>,
    #[serde(default)]
    pub lines: Vec<CreateTenderLineRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTenderLineRequest {
    pub description: String,
    #[serde(default = "one")]
    pub quantity: Decimal,
    #[serde(default = "unit")]
    pub uom: String,
}

// ── Bids ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BidRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub tender_id: Uuid,
    pub vendor_id: Uuid,
    pub currency: String,
    pub total_amount: Decimal,
    pub notes: Option<String>,
    pub status: String, // submitted|shortlisted|awarded|rejected|withdrawn
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BidLineRow {
    pub id: Uuid,
    pub bid_id: Uuid,
    pub tender_line_id: Option<Uuid>,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub line_no: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitBidRequest {
    pub currency: Option<String>,
    pub notes: Option<String>,
    pub lines: Vec<SubmitBidLineRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitBidLineRequest {
    pub tender_line_id: Option<Uuid>,
    pub description: String,
    #[serde(default = "one")]
    pub quantity: Decimal,
    pub unit_price: Decimal,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AwardTenderRequest {
    pub bid_id: Uuid,
    pub delivery_date: Option<NaiveDate>,
    pub notes: Option<String>,
}

// ── Purchase orders (LPO) ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PurchaseOrderRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub vendor_id: Uuid,
    pub tender_id: Option<Uuid>,
    pub bid_id: Option<Uuid>,
    pub currency: String,
    pub fx_rate: Decimal,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub gross_total: Decimal,
    pub status: String, // issued|acknowledged|partially_invoiced|invoiced|closed|cancelled
    pub issue_date: NaiveDate,
    pub delivery_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PurchaseOrderLineRow {
    pub id: Uuid,
    pub po_id: Uuid,
    pub description: String,
    pub quantity: Decimal,
    pub uom: String,
    pub unit_price: Decimal,
    pub tax_treatment: Option<String>,
    pub account_code: Option<String>,
    pub line_total: Decimal,
    pub line_no: i32,
}

/// A vendor lodging an invoice against one of their LPOs. Raises a
/// `pending_approval` bill in the buyer's AP linked by `po_id`.
#[derive(Debug, Clone, Deserialize)]
pub struct LodgeInvoiceRequest {
    pub vendor_invoice_number: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub notes: Option<String>,
    /// Optional line overrides; when empty the LPO lines are billed as-is.
    #[serde(default)]
    pub lines: Vec<LodgeInvoiceLineRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LodgeInvoiceLineRequest {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub account_code: Option<String>,
}

fn one() -> Decimal {
    Decimal::ONE
}
fn unit() -> String {
    "unit".to_string()
}
