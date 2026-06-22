use axum::{extract::{Query, State}, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_CREATE};
use zavora_erp_core::assets::*;
use zavora_erp_core::services::assets as svc;
use zavora_erp_core::AgentOrUserId;

/// Optional targeting for a depreciation run; defaults to the period covering today.
#[derive(Debug, Default, serde::Deserialize)]
pub struct DepreciationParams {
    pub date: Option<chrono::NaiveDate>,
    pub period_id: Option<uuid::Uuid>,
}

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, FixedAssetRow>(
        "SELECT * FROM fixed_assets WHERE entity_id = $1 ORDER BY asset_number",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAssetRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create fixed asset").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_asset(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /assets/depreciation/run — post one month of depreciation for every active
/// asset into the period covering today (or the supplied `date`/`period_id`).
pub async fn run_depreciation(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Query(params): Query<DepreciationParams>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "run depreciation").map_err(err_response)?;

    let period_id = match params.period_id {
        Some(pid) => pid,
        None => {
            let date = params.date.unwrap_or_else(|| chrono::Utc::now().date_naive());
            zavora_erp_core::services::periods::period_for_date(&state.engine, ctx.entity_id, date)
                .await
                .map_err(err_response)?
                .id
        }
    };

    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::run_depreciation(&state.engine, ctx.entity_id, period_id, &actor).await {
        Ok(ids) => Ok(Json(serde_json::json!({ "depreciated": ids.len(), "asset_ids": ids }))),
        Err(e) => Err(err_response(e)),
    }
}
