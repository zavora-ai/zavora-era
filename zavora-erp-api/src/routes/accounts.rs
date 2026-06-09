use axum::{extract::{Path, State}, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use zavora_erp_core::ledger::account::*;
use zavora_erp_core::services::accounts as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::list_accounts(&state.engine, true).await {
        Ok(accounts) => Ok(Json(serde_json::to_value(accounts).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::get_account(&state.engine, &code).await {
        Ok(account) => Ok(Json(serde_json::to_value(account).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::create_account(&state.engine, req, &actor).await {
        Ok(account) => Ok(Json(serde_json::to_value(account).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn seed(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::seed_coa(&state.engine, &zavora_erp_core::ledger::CoaTemplate::KenyaStandard, &actor).await {
        Ok(count) => Ok(Json(serde_json::json!({ "seeded": count }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Json(req): Json<UpdateAccountRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::update_account(&state.engine, &code, req).await {
        Ok(account) => Ok(Json(serde_json::to_value(account).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
