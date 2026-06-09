use axum::{extract::{Path, Query, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use zavora_erp_core::audit::*;
use zavora_erp_core::services::audit as svc;

#[derive(serde::Deserialize)]
pub struct AuditQueryParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn query(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditQueryParams>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let q = AuditQuery {
        entity_id: state.engine.entity_id(),
        object_type: None,
        object_id: None,
        actor: None,
        event_type: None,
        from: None,
        to: None,
        limit: params.limit.or(Some(50)),
        offset: params.offset,
    };
    match svc::query_events(&state.engine, q).await {
        Ok(page) => Ok(Json(serde_json::to_value(page).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn for_object(
    State(state): State<Arc<AppState>>,
    Path((object_type, object_id)): Path<(String, Uuid)>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let q = AuditQuery {
        entity_id: state.engine.entity_id(),
        object_type: Some(object_type),
        object_id: Some(object_id),
        actor: None,
        event_type: None,
        from: None,
        to: None,
        limit: Some(100),
        offset: Some(0),
    };
    match svc::query_events(&state.engine, q).await {
        Ok(page) => Ok(Json(serde_json::to_value(page).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
