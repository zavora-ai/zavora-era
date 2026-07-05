//! Vendor-gated portal endpoints (supplier side). Every handler extracts
//! `VendorContext`, which requires a `role = "Vendor"` token and re-checks the
//! account is active on each request. All queries are scoped to the vendor's own
//! `entity_id` + `vendor_id`, so a vendor sees only their own tenders/bids/POs.

use axum::extract::{Path, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use super::err_response;
use crate::middleware::vendor_auth::VendorContext;
use crate::AppState;
use zavora_erp_core::procurement::*;
use zavora_erp_core::services::procurement as svc;
use zavora_erp_core::ErpError;

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

/// POST /api/v1/portal/purchase-orders/{id}/invoice — lodge an invoice against
/// an LPO. Raises a `pending_approval` AP bill in the buyer's books.
pub async fn lodge_invoice(
    ctx: VendorContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<LodgeInvoiceRequest>,
) -> ApiResult {
    let bill = svc::lodge_invoice(&state.engine, ctx.entity_id, ctx.vendor_id, id, req).await.map_err(boxed)?;
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
