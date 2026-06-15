use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_POST_JOURNAL};
use super::err_response;
use zavora_erp_core::ledger::journal::*;
use zavora_erp_core::{AgentOrUserId, PostingRequest};

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, JournalEntryRow>(
        "SELECT * FROM journal_entries WHERE entity_id = $1 ORDER BY date DESC, number DESC LIMIT 100",
    )
    .bind(state.engine.entity_id())
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
    Json(req): Json<CreateJournalEntryRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_POST_JOURNAL, &ctx, "post journal entry").map_err(err_response)?;
    let posting_req = PostingRequest {
        entry: req,
        posted_by: AgentOrUserId::User(ctx.user_id),
    };
    match state.engine.post_from_agent(posting_req).await {
        Ok(result) => Ok(Json(serde_json::to_value(result).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn validate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateJournalEntryRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match state.engine.validate_entry(&req).await {
        Ok(report) => Ok(Json(serde_json::to_value(report).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
