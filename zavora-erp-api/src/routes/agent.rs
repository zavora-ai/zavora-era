use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{require_role, AuthContext, ROLES_POST_JOURNAL, ROLES_VIEW};
use super::err_response;
use zavora_erp_core::{PostingRequest, reporting::ReportRequest};

/// Agent posting endpoint — spec section 27.
///
/// The agentic layer authenticates with the same identity headers as any other client.
/// Posting to the GL requires a journal-posting role.
pub async fn post_from_agent(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<PostingRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_POST_JOURNAL, &ctx, "post journal entry (agent)").map_err(err_response)?;
    match state.engine.post_from_agent(req).await {
        Ok(result) => Ok(Json(serde_json::to_value(result).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// Agent report endpoint — spec section 27.
pub async fn run_report(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReportRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_VIEW, &ctx, "run report (agent)").map_err(err_response)?;
    match state.engine.run_report(req).await {
        Ok(data) => Ok(Json(serde_json::to_value(data).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
