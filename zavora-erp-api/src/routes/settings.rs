use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_MANAGE};
use super::err_response;
use zavora_erp_core::settings::*;
use zavora_erp_core::services::settings as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn get(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::get_settings(&state.engine, ctx.entity_id).await {
        Ok(config) => Ok(Json(serde_json::to_value(config).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SettingsPatch>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "update settings").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::update_settings(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(config) => Ok(Json(serde_json::to_value(config).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
