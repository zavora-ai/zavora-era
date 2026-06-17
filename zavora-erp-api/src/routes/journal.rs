use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_POST_JOURNAL};
use super::err_response;
use zavora_erp_core::ledger::journal::*;
use zavora_erp_core::{AgentOrUserId, PostingRequest};

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, JournalEntryRow>(
        "SELECT * FROM journal_entries WHERE entity_id = $1 ORDER BY date DESC, number DESC LIMIT 100",
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

/// POST /journal-entries/{id}/reverse — book a linked reversing entry.
/// A posted journal entry is immutable; correcting it means posting a reversal.
pub async fn reverse(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_POST_JOURNAL, &ctx, "reverse journal entry").map_err(err_response)?;
    let reason = req.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());
    let reversal_date = req.get("date").and_then(|v| v.as_str()).and_then(|s| s.parse::<chrono::NaiveDate>().ok());
    let actor = AgentOrUserId::User(ctx.user_id);
    match zavora_erp_core::services::journal::reverse_journal_entry(&state.engine, ctx.entity_id, id, reason, reversal_date, actor).await {
        Ok(entry) => Ok(Json(serde_json::json!({
            "reversing_entry_id": entry.id,
            "reversing_number": entry.number,
        }))),
        Err(e) => Err(err_response(e)),
    }
}
