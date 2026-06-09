use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use zavora_erp_core::settings::*;
use zavora_erp_core::services::settings as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn get(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::get_settings(&state.engine).await {
        Ok(config) => Ok(Json(serde_json::to_value(config).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SettingsPatch>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::update_settings(&state.engine, req, &actor).await {
        Ok(config) => Ok(Json(serde_json::to_value(config).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
