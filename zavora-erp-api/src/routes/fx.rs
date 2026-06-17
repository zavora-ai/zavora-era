use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_MANAGE};
use zavora_erp_core::fx::*;
use zavora_erp_core::services::fx as svc;

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, ExchangeRateRow>(
        "SELECT * FROM exchange_rates WHERE entity_id = $1 ORDER BY rate_date DESC LIMIT 100",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn upsert(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertRateRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "upsert FX rate").map_err(err_response)?;
    match svc::upsert_rate(&state.engine, ctx.entity_id, req).await {
        Ok(rate) => Ok(Json(serde_json::to_value(rate).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn revaluation(
    ctx: AuthContext,
    State(_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    if let Err(e) = require_role(ROLES_MANAGE, &ctx, "run FX revaluation") { return Err(err_response(e)); }
    // TODO: implement FX revaluation
    Ok(Json(serde_json::json!({ "status": "todo", "message": "FX revaluation not yet implemented" })))
}
