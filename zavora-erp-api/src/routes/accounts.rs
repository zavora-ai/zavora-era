use axum::{extract::{Path, State}, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_MANAGE};
use zavora_erp_core::ledger::account::*;
use zavora_erp_core::services::accounts as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::list_accounts(&state.engine, ctx.entity_id, true).await {
        Ok(accounts) => Ok(Json(serde_json::to_value(accounts).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn get(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::get_account(&state.engine, ctx.entity_id, &code).await {
        Ok(account) => Ok(Json(serde_json::to_value(account).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAccountRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "create account").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_account(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(account) => Ok(Json(serde_json::to_value(account).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn seed(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "seed chart of accounts").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::seed_coa(&state.engine, ctx.entity_id, &zavora_erp_core::ledger::CoaTemplate::KenyaStandard, &actor).await {
        Ok(count) => Ok(Json(serde_json::json!({ "seeded": count }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Json(req): Json<UpdateAccountRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "update account").map_err(err_response)?;
    match svc::update_account(&state.engine, ctx.entity_id, &code, req).await {
        Ok(account) => Ok(Json(serde_json::to_value(account).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
