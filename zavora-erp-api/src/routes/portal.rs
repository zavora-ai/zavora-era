//! Vendor-gated portal endpoints (supplier side). Every handler extracts
//! `VendorContext`, which requires a `role = "Vendor"` token and re-checks the
//! account is active on each request. All queries are scoped to the vendor's own
//! `entity_id` + `vendor_id`, so a vendor sees only their own tenders/bids/POs.

use axum::extract::{Multipart, Path, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use super::err_response;
use crate::middleware::vendor_auth::VendorContext;
use crate::AppState;
use zavora_erp_core::procurement::*;
use zavora_erp_core::services::procurement as svc;
use zavora_erp_core::{AgentOrUserId, ErpError};

/// Max eTIMS attachment size (12 MiB) — matches the staff attachments cap.
const MAX_ETIMS_BYTES: usize = 12 * 1024 * 1024;
/// Accepted eTIMS invoice file types (PDF export or a photo/scan of the receipt).
const ETIMS_MIME_ALLOW: &[&str] = &["application/pdf", "image/jpeg", "image/png", "image/webp"];

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;

fn boxed(e: ErpError) -> axum::response::Response {
    use axum::response::IntoResponse;
    err_response(e).into_response()
}
fn ok<T: serde::Serialize>(v: T) -> ApiResult {
    Ok(Json(serde_json::to_value(v).unwrap_or_default()))
}

// ── Tenders (open ones the vendor can bid on) ───────────────────────────────

/// GET /api/v1/portal/tenders — currently open tenders for the vendor's tenant.
pub async fn open_tenders(ctx: VendorContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query_as::<_, TenderRow>(
        "SELECT * FROM tenders WHERE entity_id=$1 AND status='open' ORDER BY closing_date NULLS LAST, created_at DESC",
    )
    .bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| boxed(ErpError::Database(e)))?;
    ok(rows)
}

/// GET /api/v1/portal/tenders/{id} — a tender + its line items to price.
pub async fn get_tender(ctx: VendorContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let pool = state.engine.pool();
    let tender = sqlx::query_as::<_, TenderRow>(
        "SELECT * FROM tenders WHERE id=$1 AND entity_id=$2 AND status IN ('open','awarded','closed')",
    )
    .bind(id).bind(ctx.entity_id).fetch_optional(pool).await.map_err(|e| boxed(ErpError::Database(e)))?
    .ok_or_else(|| boxed(ErpError::NotFound { entity_type: "tender".into(), id }))?;
    let lines = sqlx::query_as::<_, TenderLineRow>("SELECT * FROM tender_lines WHERE tender_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(pool).await.unwrap_or_default();
    // Include the vendor's own bid, if any (so the UI can show/edit it).
    let my_bid = sqlx::query_as::<_, BidRow>("SELECT * FROM bids WHERE tender_id=$1 AND vendor_id=$2")
        .bind(id).bind(ctx.vendor_id).fetch_optional(pool).await.ok().flatten();
    ok(serde_json::json!({ "tender": tender, "lines": lines, "my_bid": my_bid }))
}

/// POST /api/v1/portal/tenders/{id}/bid — submit (or replace) a bid.
pub async fn submit_bid(
    ctx: VendorContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<SubmitBidRequest>,
) -> ApiResult {
    let bid = svc::submit_bid(&state.engine, ctx.entity_id, id, ctx.vendor_id, req).await.map_err(boxed)?;
    ok(bid)
}

/// GET /api/v1/portal/bids — the vendor's own bids.
pub async fn my_bids(ctx: VendorContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query_as::<_, BidRow>(
        "SELECT * FROM bids WHERE entity_id=$1 AND vendor_id=$2 ORDER BY submitted_at DESC",
    )
    .bind(ctx.entity_id).bind(ctx.vendor_id).fetch_all(state.engine.pool()).await.map_err(|e| boxed(ErpError::Database(e)))?;
    ok(rows)
}

// ── Purchase orders (LPOs awarded to the vendor) ────────────────────────────

pub async fn my_purchase_orders(ctx: VendorContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query_as::<_, PurchaseOrderRow>(
        "SELECT * FROM purchase_orders WHERE entity_id=$1 AND vendor_id=$2 ORDER BY created_at DESC",
    )
    .bind(ctx.entity_id).bind(ctx.vendor_id).fetch_all(state.engine.pool()).await.map_err(|e| boxed(ErpError::Database(e)))?;
    ok(rows)
}

pub async fn get_purchase_order(ctx: VendorContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let pool = state.engine.pool();
    let po = sqlx::query_as::<_, PurchaseOrderRow>(
        "SELECT * FROM purchase_orders WHERE id=$1 AND entity_id=$2 AND vendor_id=$3",
    )
    .bind(id).bind(ctx.entity_id).bind(ctx.vendor_id).fetch_optional(pool).await.map_err(|e| boxed(ErpError::Database(e)))?
    .ok_or_else(|| boxed(ErpError::NotFound { entity_type: "purchase order".into(), id }))?;
    let lines = sqlx::query_as::<_, PurchaseOrderLineRow>("SELECT * FROM purchase_order_lines WHERE po_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(pool).await.unwrap_or_default();
    ok(serde_json::json!({ "purchase_order": po, "lines": lines }))
}

/// GET /api/v1/portal/purchase-orders/{id}/document?format=html|pdf — the vendor's
/// copy of the legal LPO. Same renderer as the buyer's, scoped to this vendor's
/// own POs so a supplier can only fetch their own orders.
pub async fn purchase_order_document(
    ctx: VendorContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<crate::routes::procurement::DocumentQuery>,
) -> axum::response::Response {
    // Ownership gate: 404 unless this LPO belongs to the calling vendor.
    let owns = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM purchase_orders WHERE id=$1 AND entity_id=$2 AND vendor_id=$3",
    )
    .bind(id).bind(ctx.entity_id).bind(ctx.vendor_id)
    .fetch_one(state.engine.pool()).await.unwrap_or(0);
    if owns == 0 {
        return boxed(ErpError::NotFound { entity_type: "purchase order".into(), id });
    }
    crate::routes::procurement::render_po_document(&state, ctx.entity_id, id, q.format.as_deref() == Some("pdf")).await
}

/// POST /api/v1/portal/purchase-orders/{id}/invoice — lodge an invoice against an
/// LPO. **Multipart** because a valid Kenyan supplier invoice is an eTIMS
/// (electronic Tax Invoice) document: both the eTIMS invoice number and the
/// eTIMS invoice file are mandatory. Raises a `pending_approval` AP bill and
/// attaches the eTIMS file to it, so the buyer sees the source document.
///
/// Parts: `vendor_invoice_number` (required, the eTIMS number), `etims_file`
/// (required), `issue_date` (optional), `notes` (optional).
pub async fn lodge_invoice(
    ctx: VendorContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult {
    let mut vendor_invoice_number: Option<String> = None;
    let mut issue_date: Option<chrono::NaiveDate> = None;
    let mut notes: Option<String> = None;
    let mut file_bytes: Vec<u8> = Vec::new();
    let mut file_name = "etims-invoice".to_string();
    let mut file_mime = "application/octet-stream".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| boxed(ErpError::ValidationFailed { message: format!("invalid upload: {e}") }))?
    {
        match field.name().unwrap_or("") {
            "etims_file" => {
                if let Some(fname) = field.file_name() {
                    if !fname.is_empty() { file_name = fname.to_string(); }
                }
                if let Some(ct) = field.content_type() {
                    file_mime = ct.to_string();
                }
                let data = field.bytes().await.map_err(|e| boxed(ErpError::ValidationFailed { message: format!("could not read file: {e}") }))?;
                if data.len() > MAX_ETIMS_BYTES {
                    return Err(boxed(ErpError::ValidationFailed { message: "eTIMS file exceeds 12 MB".into() }));
                }
                file_bytes = data.to_vec();
            }
            "vendor_invoice_number" => {
                let v = field.text().await.unwrap_or_default();
                let v = v.trim().to_string();
                if !v.is_empty() { vendor_invoice_number = Some(v); }
            }
            "issue_date" => {
                let v = field.text().await.unwrap_or_default();
                if !v.trim().is_empty() {
                    issue_date = chrono::NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d").ok();
                }
            }
            "notes" => {
                let v = field.text().await.unwrap_or_default();
                if !v.trim().is_empty() { notes = Some(v.trim().to_string()); }
            }
            _ => { let _ = field.bytes().await; }
        }
    }

    // Mandatory-eTIMS gate — enforced server-side so no client can bypass it.
    if vendor_invoice_number.is_none() {
        return Err(boxed(ErpError::ValidationFailed { message: "eTIMS invoice number is required".into() }));
    }
    if file_bytes.is_empty() {
        return Err(boxed(ErpError::ValidationFailed { message: "an eTIMS invoice file (PDF or image) must be attached".into() }));
    }
    if !ETIMS_MIME_ALLOW.contains(&file_mime.as_str()) {
        return Err(boxed(ErpError::ValidationFailed {
            message: format!("unsupported eTIMS file type '{file_mime}' — attach a PDF, JPG or PNG"),
        }));
    }

    let bill = svc::lodge_invoice(
        &state.engine,
        ctx.entity_id,
        ctx.vendor_id,
        id,
        LodgeInvoiceRequest { vendor_invoice_number, issue_date, notes, lines: Vec::new() },
    )
    .await
    .map_err(boxed)?;

    // Attach the eTIMS file to the bill so the buyer sees the source document on
    // the staff Bills page (which lists attachments of linked_type "bill").
    let uploader = AgentOrUserId::Agent(format!("vendor:{}", ctx.vendor_id));
    zavora_erp_core::services::attachments::upload(
        &state.engine, ctx.entity_id, "bill", bill.id, &file_name, &file_mime, &file_bytes, &uploader,
    )
    .await
    .map_err(boxed)?;

    ok(bill)
}

/// GET /api/v1/portal/invoices — bills the vendor has lodged (their AP with us).
pub async fn my_invoices(ctx: VendorContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query_as::<_, zavora_erp_core::ap::BillRow>(
        "SELECT * FROM bills WHERE entity_id=$1 AND vendor_id=$2 ORDER BY created_at DESC",
    )
    .bind(ctx.entity_id).bind(ctx.vendor_id).fetch_all(state.engine.pool()).await.map_err(|e| boxed(ErpError::Database(e)))?;
    ok(rows)
}

/// GET /api/v1/portal/statement — the vendor's running statement: every bill
/// with its outstanding balance, plus totals.
pub async fn statement(ctx: VendorContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let pool = state.engine.pool();
    let rows = sqlx::query_as::<_, zavora_erp_core::ap::BillRow>(
        "SELECT * FROM bills WHERE entity_id=$1 AND vendor_id=$2 ORDER BY issue_date ASC, created_at ASC",
    )
    .bind(ctx.entity_id).bind(ctx.vendor_id).fetch_all(pool).await.map_err(|e| boxed(ErpError::Database(e)))?;

    let totals: (Option<rust_decimal::Decimal>, Option<rust_decimal::Decimal>) = sqlx::query_as(
        "SELECT COALESCE(SUM(gross_total),0), COALESCE(SUM(balance_due),0) FROM bills WHERE entity_id=$1 AND vendor_id=$2",
    )
    .bind(ctx.entity_id).bind(ctx.vendor_id).fetch_one(pool).await.unwrap_or((None, None));

    let vendor = sqlx::query_as::<_, VendorUserRow>(
        "SELECT id, entity_id, email, display_name, company_name, kra_pin, phone, status, vendor_id, last_login, created_at \
         FROM vendor_users WHERE id=$1 AND entity_id=$2",
    )
    .bind(ctx.vendor_user_id).bind(ctx.entity_id).fetch_optional(pool).await.ok().flatten();

    ok(serde_json::json!({
        "vendor": vendor,
        "bills": rows,
        "total_billed": totals.0,
        "total_outstanding": totals.1,
    }))
}
