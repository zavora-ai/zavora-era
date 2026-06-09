use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use zavora_erp_core::ledger::journal::*;
use zavora_erp_core::{AgentOrUserId, PostingRequest};

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateJournalEntryRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let posting_req = PostingRequest {
        entry: req,
        posted_by: AgentOrUserId::Agent("api".to_string()),
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
