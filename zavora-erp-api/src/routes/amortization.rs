use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::auth::AuthContext;
use crate::AppState;
use super::err_response;
use zavora_erp_core::amortization::CreateScheduleRequest;
use zavora_erp_core::services::amortization as svc;
use zavora_erp_core::AgentOrUserId;

/// GET /amortization — list the tenant's schedules.
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::list_schedules(&state.engine, ctx.entity_id).await {
        Ok(rows) => Ok(Json(serde_json::to_value(rows).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /amortization — create a schedule.
pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateScheduleRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_schedule(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /amortization/{id}/cancel — cancel a schedule.
pub async fn cancel(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::cancel_schedule(&state.engine, ctx.entity_id, id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "cancelled" }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /amortization/run — post any due installments now (catch-up to today).
pub async fn run(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::User(ctx.user_id);
    let today = chrono::Utc::now().date_naive();
    match svc::run_amortization(&state.engine, ctx.entity_id, today, &actor).await {
        Ok(ids) => Ok(Json(serde_json::json!({ "posted_schedules": ids.len() }))),
        Err(e) => Err(err_response(e)),
    }
}
