//! Staff-side procurement endpoints (buyer): vendor-application review, tenders,
//! bids, award → LPO, purchase-order views. All gated by `AuthContext` +
//! `require_role`; a Vendor token can never reach these (its role is unknown to
//! the staff auth layer).

use axum::extract::{Path, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_APPROVE, ROLES_CREATE, ROLES_VIEW};
use crate::AppState;
use zavora_erp_core::procurement::*;
use zavora_erp_core::services::procurement as svc;
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;

fn ok<T: serde::Serialize>(v: T) -> ApiResult {
    Ok(Json(serde_json::to_value(v).unwrap_or_default()))
}

// ── Vendor applications ─────────────────────────────────────────────────────

/// GET /api/v1/vendor-applications — pending (and recent) registrations.
pub async fn list_applications(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view vendor applications").map_err(err_response_boxed)?;
    let rows = sqlx::query_as::<_, VendorUserRow>(
        "SELECT id, entity_id, email, display_name, company_name, kra_pin, phone, status, vendor_id, last_login, created_at \
         FROM vendor_users WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    ok(rows)
}

/// POST /api/v1/vendor-applications/{id}/approve — activate the login and link
/// (or create) the `vendors` master it will transact under.
pub async fn approve_application(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveVendorRequest>,
) -> ApiResult {
    require_role(ROLES_APPROVE, &ctx, "approve vendor").map_err(err_response_boxed)?;
    let pool = state.engine.pool();

    let app = sqlx::query_as::<_, VendorUserRow>(
        "SELECT id, entity_id, email, display_name, company_name, kra_pin, phone, status, vendor_id, last_login, created_at \
         FROM vendor_users WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id).fetch_optional(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?
    .ok_or_else(|| err_response_boxed(ErpError::NotFound { entity_type: "vendor application".into(), id }))?;

    // Link the caller-supplied master, else create one from the registration.
    let vendor_id = if let Some(v) = req.vendor_id {
        v
    } else {
        let vid = Uuid::new_v4();
        // `vendors.email`/`.phone` are JSONB arrays of contact strings.
        let emails = serde_json::json!([app.email]);
        let phones = app.phone.as_ref().map(|p| serde_json::json!([p])).unwrap_or_else(|| serde_json::json!([]));
        sqlx::query(
            "INSERT INTO vendors (id, entity_id, name, email, phone, kra_pin, is_active) \
             VALUES ($1,$2,$3,$4,$5,$6,true)",
        )
        .bind(vid).bind(ctx.entity_id).bind(&app.company_name).bind(emails).bind(phones).bind(&app.kra_pin)
        .execute(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
        vid
    };

    sqlx::query("UPDATE vendor_users SET status='active', vendor_id=$1 WHERE id=$2 AND entity_id=$3")
        .bind(vendor_id).bind(id).bind(ctx.entity_id)
        .execute(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;

    ok(serde_json::json!({ "id": id, "status": "active", "vendor_id": vendor_id }))
}

/// POST /api/v1/vendor-applications/{id}/reject
pub async fn reject_application(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult {
    require_role(ROLES_APPROVE, &ctx, "reject vendor").map_err(err_response_boxed)?;
    let n = sqlx::query("UPDATE vendor_users SET status='rejected' WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(ctx.entity_id).execute(state.engine.pool()).await
        .map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    if n.rows_affected() == 0 {
        return Err(err_response_boxed(ErpError::NotFound { entity_type: "vendor application".into(), id }));
    }
    ok(serde_json::json!({ "id": id, "status": "rejected" }))
}

// ── Tenders ─────────────────────────────────────────────────────────────────

pub async fn list_tenders(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view tenders").map_err(err_response_boxed)?;
    let rows = sqlx::query_as::<_, TenderRow>("SELECT * FROM tenders WHERE entity_id=$1 ORDER BY created_at DESC")
        .bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    ok(rows)
}

pub async fn get_tender(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view tender").map_err(err_response_boxed)?;
    let pool = state.engine.pool();
    let tender = sqlx::query_as::<_, TenderRow>("SELECT * FROM tenders WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(ctx.entity_id).fetch_optional(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?
        .ok_or_else(|| err_response_boxed(ErpError::NotFound { entity_type: "tender".into(), id }))?;
    let lines = sqlx::query_as::<_, TenderLineRow>("SELECT * FROM tender_lines WHERE tender_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(pool).await.unwrap_or_default();
    ok(serde_json::json!({ "tender": tender, "lines": lines }))
}

pub async fn create_tender(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateTenderRequest>) -> ApiResult {
    require_role(ROLES_CREATE, &ctx, "create tender").map_err(err_response_boxed)?;
    let row = svc::create_tender(&state.engine, ctx.entity_id, req, ctx.user_id).await.map_err(err_response_boxed)?;
    ok(row)
}

pub async fn publish_tender(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    require_role(ROLES_CREATE, &ctx, "publish tender").map_err(err_response_boxed)?;
    svc::publish_tender(&state.engine, ctx.entity_id, id).await.map_err(err_response_boxed)?;
    ok(serde_json::json!({ "id": id, "status": "open" }))
}

/// GET /api/v1/tenders/{id}/bids — submitted bids for a tender (buyer view).
pub async fn list_bids(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view bids").map_err(err_response_boxed)?;
    let rows = sqlx::query_as::<_, BidRow>(
        "SELECT * FROM bids WHERE tender_id=$1 AND entity_id=$2 ORDER BY total_amount ASC",
    )
    .bind(id).bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    ok(rows)
}

/// POST /api/v1/tenders/{id}/award — award a bid, build the LPO.
pub async fn award_tender(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<AwardTenderRequest>,
) -> ApiResult {
    require_role(ROLES_APPROVE, &ctx, "award tender").map_err(err_response_boxed)?;
    let po = svc::award_tender(&state.engine, ctx.entity_id, id, req, ctx.user_id).await.map_err(err_response_boxed)?;
    ok(po)
}

// ── Purchase orders ─────────────────────────────────────────────────────────

pub async fn list_purchase_orders(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view purchase orders").map_err(err_response_boxed)?;
    let rows = sqlx::query_as::<_, PurchaseOrderRow>("SELECT * FROM purchase_orders WHERE entity_id=$1 ORDER BY created_at DESC")
        .bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    ok(rows)
}

pub async fn get_purchase_order(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view purchase order").map_err(err_response_boxed)?;
    let pool = state.engine.pool();
    let po = sqlx::query_as::<_, PurchaseOrderRow>("SELECT * FROM purchase_orders WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(ctx.entity_id).fetch_optional(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?
        .ok_or_else(|| err_response_boxed(ErpError::NotFound { entity_type: "purchase order".into(), id }))?;
    let lines = sqlx::query_as::<_, PurchaseOrderLineRow>("SELECT * FROM purchase_order_lines WHERE po_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(pool).await.unwrap_or_default();
    ok(serde_json::json!({ "purchase_order": po, "lines": lines }))
}

/// `err_response` returns `impl IntoResponse`; box it to a concrete `Response`
/// so it can be used with the `?` operator in these handlers.
fn err_response_boxed(e: ErpError) -> axum::response::Response {
    use axum::response::IntoResponse;
    err_response(e).into_response()
}
