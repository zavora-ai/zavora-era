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
use crate::middleware::auth::{require_role, AuthContext, ROLES_MANAGE};
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

// ───────────────────────────────────────────────────────────────────────────
// Admin delivery history — read-only view across ALL channels (Owner/Admin).
//
// The in-app inbox above is scoped to `channel = 'in_app'`. This view surfaces
// the full delivery record written by the notification worker — email, SMS,
// WhatsApp and in-app — so an admin can answer "did it actually send, to whom,
// when, and why did it fail?". Read-only; never mutates.
// ───────────────────────────────────────────────────────────────────────────

/// One delivery-history row (all channels, including delivery metadata).
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DeliveryRow {
    pub id: Uuid,
    pub event_type: String,
    pub channel: String,
    pub recipient: String,
    pub subject: Option<String>,
    pub status: String,
    pub related_type: Option<String>,
    pub related_id: Option<Uuid>,
    pub scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub sent_at: Option<chrono::DateTime<chrono::Utc>>,
    pub delivered_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Filters for the delivery-history list. All optional; combined with AND.
#[derive(Debug, Default, Deserialize)]
pub struct DeliveryParams {
    /// email | sms | whatsapp | in_app
    pub channel: Option<String>,
    /// queued | sent | delivered | failed | read
    pub status: Option<String>,
    pub event_type: Option<String>,
    /// Substring match on recipient (case-insensitive).
    pub search: Option<String>,
    /// Inclusive lower/upper bounds on created_at (RFC3339).
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// GET /notifications/delivery — paginated delivery history across all channels
/// (Owner/Admin only). Newest first; filterable by channel/status/event/recipient/date.
pub async fn delivery_list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Query(params): Query<DeliveryParams>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use axum::response::IntoResponse;
    require_role(ROLES_MANAGE, &ctx, "view notification delivery history")
        .map_err(|e| err_response(e).into_response())?;

    let page = PaginationParams { limit: params.limit, offset: params.offset };

    // Build a parameterised WHERE clause. Bind params positionally in the same
    // order they are appended so the query is injection-safe.
    let mut conds = vec!["entity_id = $1".to_string()];
    let mut idx = 2;
    if params.channel.is_some() { conds.push(format!("channel = ${idx}")); idx += 1; }
    if params.status.is_some() { conds.push(format!("status = ${idx}")); idx += 1; }
    if params.event_type.is_some() { conds.push(format!("event_type = ${idx}")); idx += 1; }
    if params.search.is_some() { conds.push(format!("recipient ILIKE ${idx}")); idx += 1; }
    if params.from.is_some() { conds.push(format!("created_at >= ${idx}")); idx += 1; }
    if params.to.is_some() { conds.push(format!("created_at <= ${idx}")); idx += 1; }
    let where_clause = conds.join(" AND ");

    let search_like = params.search.as_ref().map(|s| format!("%{s}%"));

    // Bind helper closures can't easily be reused across two queries with sqlx,
    // so bind inline for both the count and the page.
    let count_sql = format!("SELECT COUNT(*) FROM notifications WHERE {where_clause}");
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql).bind(ctx.entity_id);
    if let Some(ref v) = params.channel { count_q = count_q.bind(v.clone()); }
    if let Some(ref v) = params.status { count_q = count_q.bind(v.clone()); }
    if let Some(ref v) = params.event_type { count_q = count_q.bind(v.clone()); }
    if let Some(ref v) = search_like { count_q = count_q.bind(v.clone()); }
    if let Some(v) = params.from { count_q = count_q.bind(v); }
    if let Some(v) = params.to { count_q = count_q.bind(v); }
    let total = count_q.fetch_one(state.engine.pool()).await.unwrap_or(0);

    let list_sql = format!(
        "SELECT id, event_type, channel, recipient, subject, status, related_type, related_id, \
                scheduled_at, sent_at, delivered_at, error, created_at \
         FROM notifications WHERE {where_clause} \
         ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
        idx, idx + 1
    );
    let mut list_q = sqlx::query_as::<_, DeliveryRow>(&list_sql).bind(ctx.entity_id);
    if let Some(ref v) = params.channel { list_q = list_q.bind(v.clone()); }
    if let Some(ref v) = params.status { list_q = list_q.bind(v.clone()); }
    if let Some(ref v) = params.event_type { list_q = list_q.bind(v.clone()); }
    if let Some(ref v) = search_like { list_q = list_q.bind(v.clone()); }
    if let Some(v) = params.from { list_q = list_q.bind(v); }
    if let Some(v) = params.to { list_q = list_q.bind(v); }
    list_q = list_q.bind(page.effective_limit()).bind(page.effective_offset());

    match list_q.fetch_all(state.engine.pool()).await {
        Ok(rows) => Ok(Json(
            serde_json::to_value(PaginatedResponse::new(rows, total, &page)).unwrap_or_default(),
        )),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e)).into_response()),
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct StatusCount {
    status: String,
    count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ChannelCount {
    channel: String,
    count: i64,
}

/// GET /notifications/delivery/stats — summary counts by status and channel
/// (Owner/Admin only). Drives the admin dashboard cards.
pub async fn delivery_stats(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use axum::response::IntoResponse;
    require_role(ROLES_MANAGE, &ctx, "view notification delivery stats")
        .map_err(|e| err_response(e).into_response())?;

    let by_status = sqlx::query_as::<_, StatusCount>(
        "SELECT status, COUNT(*) AS count FROM notifications WHERE entity_id = $1 GROUP BY status",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();

    let by_channel = sqlx::query_as::<_, ChannelCount>(
        "SELECT channel, COUNT(*) AS count FROM notifications WHERE entity_id = $1 GROUP BY channel",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();

    let total: i64 = by_status.iter().map(|s| s.count).sum();
    let failed: i64 = by_status
        .iter()
        .find(|s| s.status == "failed")
        .map(|s| s.count)
        .unwrap_or(0);

    Ok(Json(serde_json::json!({
        "total": total,
        "failed": failed,
        "by_status": by_status,
        "by_channel": by_channel,
    })))
}
