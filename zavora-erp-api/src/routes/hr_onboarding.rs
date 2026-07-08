//! HR onboarding case routes (distinct from `onboarding.rs`, which is financial
//! opening balances). Back-office HR only.

use std::sync::Arc;
use axum::{extract::{Path, State}, Json};
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{AuthContext};
use zavora_erp_core::hr::CreateOnboardingRequest;
use zavora_erp_core::services::hr_onboarding as svc;
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;
fn er(e: ErpError) -> axum::response::Response { use axum::response::IntoResponse; err_response(e).into_response() }

pub async fn list(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let cases = svc::list_cases(&state.engine, ctx.entity_id, "Onboarding").await.map_err(er)?;
    Ok(Json(cases))
}

pub async fn create(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateOnboardingRequest>) -> ApiResult {
    let id = svc::create_onboarding(&state.engine, ctx.entity_id, Some(ctx.user_id), req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn get_one(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let data = svc::get_case(&state.engine, ctx.entity_id, id).await.map_err(er)?;
    Ok(Json(data))
}

#[derive(serde::Deserialize)]
pub struct TaskDone { pub done: bool }
pub async fn set_task(ctx: AuthContext, State(state): State<Arc<AppState>>, Path((_case, task)): Path<(Uuid, Uuid)>, Json(b): Json<TaskDone>) -> ApiResult {
    svc::set_task_done(&state.engine, ctx.entity_id, task, b.done).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn complete(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    svc::complete_case(&state.engine, ctx.entity_id, id).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "complete" })))
}
