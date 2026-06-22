//! In-app notification inbox endpoints.
//!
//! The notification worker persists rows with `channel = 'in_app'`; this module
//! exposes them as a per-entity inbox with read tracking. Read state is the
//! presence of `read_at` (and `status = 'read'`).

use axum::{extract::{Path, Query, State}, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::AuthContext;
use super::err_response;
use super::pagination::{PaginatedResponse, PaginationParams};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct NotificationRow {
    pub id: Uuid,
    pub event_type: String,
    pub subject: Option<String>,
    pub body: String,
    pub related_type: Option<String>,
    pub related_id: Option<Uuid>,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// NOTE: serde_urlencoded (used by axum's Query) does not support `#[serde(flatten)]`,
// so the pagination fields are declared explicitly and converted below.
#[derive(Debug, Default, Deserialize)]
pub struct ListParams {
    #[serde(default)]
    pub unread_only: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// GET /notifications — list in-app notifications (newest first), optionally unread-only.
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let page = PaginationParams { limit: params.limit, offset: params.offset };
    let unread_clause = if params.unread_only { " AND read_at IS NULL" } else { "" };

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM notifications WHERE entity_id = $1 AND channel = 'in_app'{unread_clause}"
    ))
    .bind(ctx.entity_id)
    .fetch_one(state.engine.pool())
    .await
    .unwrap_or(0);

    let rows = sqlx::query_as::<_, NotificationRow>(&format!(
        "SELECT id, event_type, subject, body, related_type, related_id, read_at, created_at \
         FROM notifications WHERE entity_id = $1 AND channel = 'in_app'{unread_clause} \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3"
    ))
    .bind(ctx.entity_id)
    .bind(page.effective_limit())
    .bind(page.effective_offset())
    .fetch_all(state.engine.pool())
    .await;

    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(PaginatedResponse::new(r, total, &page)).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// GET /notifications/unread-count — number of unread in-app notifications.
pub async fn unread_count(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE entity_id = $1 AND channel = 'in_app' AND read_at IS NULL",
    )
    .bind(ctx.entity_id)
    .fetch_one(state.engine.pool())
    .await
    .unwrap_or(0);
    Ok::<_, axum::http::StatusCode>(Json(serde_json::json!({ "count": count })))
}

/// PATCH /notifications/{id}/read — mark one notification read (idempotent).
pub async fn mark_read(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let res = sqlx::query(
        "UPDATE notifications SET read_at = COALESCE(read_at, NOW()), status = 'read' \
         WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(ctx.entity_id)
    .execute(state.engine.pool())
    .await;
    match res {
        Ok(r) if r.rows_affected() == 0 => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Notification".into(), id })),
        Ok(_) => Ok(Json(serde_json::json!({ "id": id, "read": true }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// POST /notifications/mark-all-read — mark all unread in-app notifications read.
pub async fn mark_all_read(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let res = sqlx::query(
        "UPDATE notifications SET read_at = NOW(), status = 'read' \
         WHERE entity_id = $1 AND channel = 'in_app' AND read_at IS NULL",
    )
    .bind(ctx.entity_id)
    .execute(state.engine.pool())
    .await;
    match res {
        Ok(r) => Ok(Json(serde_json::json!({ "marked": r.rows_affected() }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}
