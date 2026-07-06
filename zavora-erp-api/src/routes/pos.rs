//! Point of Sale API — shift sessions, sales, and the Z-report.

use axum::extract::{Path, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_CREATE, ROLES_VIEW};
use crate::AppState;
use zavora_erp_core::services::pos as svc;
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;

fn boxed(e: ErpError) -> axum::response::Response {
    use axum::response::IntoResponse;
    err_response(e).into_response()
}
fn ok<T: serde::Serialize>(v: T) -> ApiResult {
    Ok(Json(serde_json::to_value(v).unwrap_or_default()))
}

/// GET /api/v1/pos/session — the caller's currently open till (or null).
pub async fn current_session(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view pos session").map_err(boxed)?;
    let s = svc::get_open_session(&state.engine, ctx.entity_id, ctx.user_id).await.map_err(boxed)?;
    ok(s)
}

/// GET /api/v1/pos/sessions — recent shift sessions.
pub async fn list_sessions(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view pos sessions").map_err(boxed)?;
    ok(svc::list_sessions(&state.engine, ctx.entity_id).await.map_err(boxed)?)
}

/// POST /api/v1/pos/session/open — open a till with an opening float.
pub async fn open_session(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<svc::OpenSessionRequest>) -> ApiResult {
    require_role(ROLES_CREATE, &ctx, "open pos session").map_err(boxed)?;
    ok(svc::open_session(&state.engine, ctx.entity_id, req, ctx.user_id).await.map_err(boxed)?)
}

/// POST /api/v1/pos/session/{id}/sale — complete a sale on an open till.
pub async fn complete_sale(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<svc::CompleteSaleRequest>) -> ApiResult {
    require_role(ROLES_CREATE, &ctx, "complete pos sale").map_err(boxed)?;
    ok(svc::complete_sale(&state.engine, ctx.entity_id, id, req, ctx.user_id).await.map_err(boxed)?)
}

/// GET /api/v1/pos/session/{id}/z-report — sales by tender + expected cash.
pub async fn z_report(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view z-report").map_err(boxed)?;
    ok(svc::z_report(&state.engine, ctx.entity_id, id).await.map_err(boxed)?)
}

/// POST /api/v1/pos/session/{id}/close — close the till and reconcile cash.
pub async fn close_session(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<svc::CloseSessionRequest>) -> ApiResult {
    require_role(ROLES_CREATE, &ctx, "close pos session").map_err(boxed)?;
    ok(svc::close_session(&state.engine, ctx.entity_id, id, req, ctx.user_id).await.map_err(boxed)?)
}

#[derive(serde::Deserialize)]
pub struct ReceiptQuery {
    pub tendered: Option<rust_decimal::Decimal>,
}

/// GET /api/v1/pos/receipt/{invoice_id}?tendered=X — the ETR/eTIMS tax receipt
/// as an 80mm thermal HTML page (auto-prints on load).
pub async fn receipt(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, axum::extract::Query(q): axum::extract::Query<ReceiptQuery>) -> axum::response::Response {
    use axum::response::IntoResponse;
    if let Err(e) = require_role(ROLES_VIEW, &ctx, "print receipt") { return boxed(e); }
    match svc::pos_receipt_html(&state.engine, ctx.entity_id, id, q.tendered).await {
        Ok(html) => axum::response::Html(html).into_response(),
        Err(e) => boxed(e),
    }
}
