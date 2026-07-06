//! Procurement (P2P) domain — tenders/RFQ, bids, purchase orders (LPO), and the
//! external vendor-portal principal (`vendor_users`).
//!
//! Row structs are served directly by the API (status kept as `String`, like
//! `ap::BillRow`); request structs are the create/action payloads. Business
//! logic and state transitions live in `services::procurement`.

pub mod document;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ── Purchase requisitions (self-service front-door) ─────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PurchaseRequisitionRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub title: String,
    pub justification: Option<String>,
    pub department: Option<String>,
    pub cost_center: Option<String>,
    pub currency: String,
    pub needed_by: Option<NaiveDate>,
    pub estimated_total: Decimal,
    pub status: String, // draft|submitted|approved|rejected|converted|cancelled
    pub requested_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub converted_to_type: Option<String>,
    pub converted_to_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PurchaseRequisitionLineRow {
    pub id: Uuid,
    pub pr_id: Uuid,
    pub description: String,
    pub quantity: Decimal,
    pub uom: String,
    pub estimated_unit_price: Decimal,
    pub account_code: Option<String>,
    pub line_total: Decimal,
    pub line_no: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRequisitionRequest {
    pub title: String,
    pub justification: Option<String>,
    pub department: Option<String>,
    pub cost_center: Option<String>,
    pub currency: Option<String>,
    pub needed_by: Option<NaiveDate>,
    pub notes: Option<String>,
    #[serde(default)]
    pub lines: Vec<CreateRequisitionLineRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRequisitionLineRequest {
    pub description: String,
    #[serde(default = "one")]
    pub quantity: Decimal,
    #[serde(default = "unit")]
    pub uom: String,
    #[serde(default)]
    pub estimated_unit_price: Decimal,
    pub account_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RejectRequisitionRequest {
    pub reason: Option<String>,
}

/// Convert an approved requisition into a sourcing document. For a direct PO a
/// `vendor_id` is required; for a tender it is omitted.
#[derive(Debug, Clone, Deserialize)]
pub struct ConvertRequisitionRequest {
    pub target: String, // "tender" | "purchase_order"
    pub vendor_id: Option<Uuid>,
    pub delivery_date: Option<NaiveDate>,
    pub closing_date: Option<NaiveDate>,
}

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

/// **Direct procurement** — raise an LPO straight against a vendor master,
/// without a tender/bid (single-source or spot purchase). The vendor need not
/// have a portal login; they receive the LPO out-of-band and staff enter the
/// eventual bill on the AP side.
#[derive(Debug, Clone, Deserialize)]
pub struct CreatePurchaseOrderRequest {
    pub vendor_id: Uuid,
    pub currency: Option<String>,
    pub delivery_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub lines: Vec<CreatePurchaseOrderLineRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePurchaseOrderLineRequest {
    pub description: String,
    #[serde(default = "one")]
    pub quantity: Decimal,
    #[serde(default = "unit")]
    pub uom: String,
    pub unit_price: Decimal,
    pub account_code: Option<String>,
    pub tax_treatment: Option<String>,
}

// ── Goods receipts (GRN) + 3-way match ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GoodsReceiptRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub po_id: Uuid,
    pub receipt_date: NaiveDate,
    pub received_by: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GoodsReceiptLineRow {
    pub id: Uuid,
    pub grn_id: Uuid,
    pub po_line_id: Option<Uuid>,
    pub description: String,
    pub quantity_received: Decimal,
    pub line_no: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateGrnRequest {
    pub receipt_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub lines: Vec<CreateGrnLineRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateGrnLineRequest {
    pub po_line_id: Option<Uuid>,
    pub description: String,
    pub quantity_received: Decimal,
}

/// One line of the 3-way match report: ordered (PO) vs received (GRN) vs billed
/// (invoices). `status` is `matched`, `over_billed` (billed > received) or
/// `price_variance` (billed unit price differs from the PO beyond tolerance).
#[derive(Debug, Clone, Serialize)]
pub struct ThreeWayMatchLine {
    pub description: String,
    pub ordered_qty: Decimal,
    pub received_qty: Decimal,
    pub billed_qty: Decimal,
    pub po_unit_price: Decimal,
    pub billed_unit_price: Decimal,
    pub status: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreeWayMatch {
    pub po_id: Uuid,
    pub matched: bool,
    pub lines: Vec<ThreeWayMatchLine>,
    pub exceptions: Vec<String>,
}

fn one() -> Decimal {
    Decimal::ONE
}
fn unit() -> String {
    "unit".to_string()
}
