//! Approval spend-limits (Delegation of Authority) configuration API.

use axum::{extract::State, Json};
use std::sync::Arc;

use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_APPROVE, ROLES_VIEW};
use crate::AppState;
use zavora_erp_core::services::approval as svc;
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;

fn boxed(e: ErpError) -> axum::response::Response {
    use axum::response::IntoResponse;
    err_response(e).into_response()
}
fn ok<T: serde::Serialize>(v: T) -> ApiResult {
    Ok(Json(serde_json::to_value(v).unwrap_or_default()))
}

/// GET /api/v1/approval-limits
pub async fn list(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    require_role(ROLES_VIEW, &ctx, "view approval limits").map_err(boxed)?;
    let rows = svc::list_limits(&state.engine, ctx.entity_id).await.map_err(boxed)?;
    ok(rows)
}

#[derive(serde::Deserialize)]
pub struct SetLimitRequest {
    pub role: String,
    pub max_amount: Option<rust_decimal::Decimal>,
}

/// PUT /api/v1/approval-limits — set (or clear) a role's ceiling.
pub async fn set(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<SetLimitRequest>) -> ApiResult {
    require_role(ROLES_APPROVE, &ctx, "set approval limits").map_err(boxed)?;
    svc::set_limit(&state.engine, ctx.entity_id, &req.role, req.max_amount).await.map_err(boxed)?;
    ok(serde_json::json!({ "status": "ok" }))
}
