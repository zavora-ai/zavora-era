//! Document attachments API — upload/list/download/delete files linked to a
//! record (bill, invoice, payment, …).

use axum::{
    extract::{Multipart, Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::auth::{AuthContext};
use crate::AppState;
use super::err_response;
use axum::response::{IntoResponse, Response};
use zavora_erp_core::services::attachments as svc;
use zavora_erp_core::{AgentOrUserId, ErpError};

fn er(e: ErpError) -> Response {
    err_response(e).into_response()
}

/// Max attachment size (12 MiB) — supplier invoices/scans are small.
const MAX_BYTES: usize = 12 * 1024 * 1024;

#[derive(Deserialize)]
pub struct ListQuery {
    pub linked_type: String,
    pub linked_id: Uuid,
}

/// POST /attachments — multipart upload. Parts: `file`, `linked_type`, `linked_id`.
pub async fn upload(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, Response> {

    let mut bytes: Vec<u8> = Vec::new();
    let mut filename = "attachment".to_string();
    let mut mime_type = "application/octet-stream".to_string();
    let mut linked_type: Option<String> = None;
    let mut linked_id: Option<Uuid> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| er(ErpError::ValidationFailed { message: format!("invalid upload: {e}") }))?
    {
        match field.name() {
            Some("file") => {
                if let Some(f) = field.file_name() { filename = f.to_string(); }
                if let Some(ct) = field.content_type() { mime_type = ct.to_string(); }
                let data = field.bytes().await.map_err(|e| {
                    er(ErpError::ValidationFailed { message: format!("could not read file: {e}") })
                })?;
                bytes = data.to_vec();
            }
            Some("linked_type") => {
                linked_type = field.text().await.ok();
            }
            Some("linked_id") => {
                linked_id = field.text().await.ok().and_then(|s| Uuid::parse_str(s.trim()).ok());
            }
            _ => {}
        }
    }

    if bytes.is_empty() {
        return Err(er(ErpError::ValidationFailed { message: "no file provided".into() }));
    }
    if bytes.len() > MAX_BYTES {
        return Err(er(ErpError::ValidationFailed { message: format!("file too large (max {} MiB)", MAX_BYTES / (1024 * 1024)) }));
    }
    let (linked_type, linked_id) = match (linked_type, linked_id) {
        (Some(t), Some(id)) if !t.trim().is_empty() => (t, id),
        _ => return Err(er(ErpError::ValidationFailed { message: "linked_type and linked_id are required".into() })),
    };

    let meta = svc::upload(
        &state.engine, ctx.entity_id, &linked_type, linked_id, &filename, &mime_type, &bytes,
        &AgentOrUserId::User(ctx.user_id),
    )
    .await
    .map_err(er)?;

    Ok(Json(serde_json::to_value(meta).unwrap_or_default()))
}

/// GET /attachments?linked_type=&linked_id= — list metadata for a record.
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, impl IntoResponse> {
    match svc::list(&state.engine, ctx.entity_id, &q.linked_type, q.linked_id).await {
        Ok(rows) => Ok(Json(serde_json::to_value(rows).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// GET /attachments/{id} — the file as a data-URL for preview/download.
pub async fn get_one(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl IntoResponse> {
    match svc::get_data(&state.engine, ctx.entity_id, id).await {
        Ok((filename, mime_type, data_url)) => Ok(Json(serde_json::json!({
            "filename": filename, "mime_type": mime_type, "data_url": data_url,
        }))),
        Err(e) => Err(err_response(e)),
    }
}

/// DELETE /attachments/{id}
pub async fn delete(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl IntoResponse> {
    match svc::delete(&state.engine, ctx.entity_id, id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "deleted" }))),
        Err(e) => Err(err_response(e)),
    }
}
