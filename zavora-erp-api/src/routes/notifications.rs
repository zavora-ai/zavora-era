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

// ───────────────────────────────────────────────────────────────────────────
// Notification event preferences (Owner/Admin): per-event enabled + channels.
// ───────────────────────────────────────────────────────────────────────────

/// Build the settings response: the effective event prefs plus which channels
/// are actually configured on this deployment (so the UI can hint when a ticked
/// channel won't deliver).
async fn settings_payload(
    state: &AppState,
    entity_id: Uuid,
) -> Result<serde_json::Value, zavora_erp_core::ErpError> {
    let prefs = zavora_erp_core::services::notification_prefs::get_all(&state.engine, entity_id).await?;
    let channels: Vec<serde_json::Value> =
        zavora_erp_core::services::notification_prefs::channel_availability()
            .into_iter()
            .map(|(ch, configured)| {
                let key = serde_json::to_value(&ch)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default();
                serde_json::json!({ "channel": key, "configured": configured })
            })
            .collect();
    Ok(serde_json::json!({ "events": prefs, "channels": channels }))
}

/// GET /notification-settings — every configurable event's effective preference
/// (stored override or built-in default, flagged `is_default`).
pub async fn get_settings(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use axum::response::IntoResponse;
    require_role(ROLES_MANAGE, &ctx, "view notification settings")
        .map_err(|e| err_response(e).into_response())?;

    match settings_payload(&state, ctx.entity_id).await {
        Ok(payload) => Ok(Json(payload)),
        Err(e) => Err(err_response(e).into_response()),
    }
}

#[derive(Debug, Deserialize)]
pub struct EventPrefInput {
    pub event_type: String,
    pub enabled: bool,
    pub channels: Vec<zavora_erp_core::types::Channel>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsInput {
    pub events: Vec<EventPrefInput>,
}

/// PUT /notification-settings — upsert one or more event preferences. Each entry
/// overrides that event's built-in default for this tenant.
pub async fn update_settings(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateSettingsInput>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use axum::response::IntoResponse;
    require_role(ROLES_MANAGE, &ctx, "update notification settings")
        .map_err(|e| err_response(e).into_response())?;

    for ev in &req.events {
        zavora_erp_core::services::notification_prefs::upsert(
            &state.engine,
            ctx.entity_id,
            &ev.event_type,
            ev.enabled,
            &ev.channels,
            ctx.user_id,
        )
        .await
        .map_err(|e| err_response(e).into_response())?;
    }

    // Return the refreshed full set.
    match settings_payload(&state, ctx.entity_id).await {
        Ok(payload) => Ok(Json(payload)),
        Err(e) => Err(err_response(e).into_response()),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Per-tenant notification providers (Owner/Admin): SMTP / SMS / WhatsApp creds.
// Secrets are write-only — never returned; the response only flags has_secret.
// ───────────────────────────────────────────────────────────────────────────

/// GET /notification-providers — the tenant's configured providers, masked.
pub async fn get_providers(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use axum::response::IntoResponse;
    require_role(ROLES_MANAGE, &ctx, "view notification providers")
        .map_err(|e| err_response(e).into_response())?;

    match zavora_erp_core::services::notification_providers::get_masked(state.engine.pool(), ctx.entity_id).await {
        Ok(list) => Ok(Json(serde_json::json!({
            "providers": list,
            // Surface whether the server can store secrets at all, so the UI can
            // warn when NOTIF_ENC_KEY is missing.
            "encryption_available": zavora_erp_core::crypto::encryption_available(),
        }))),
        Err(e) => Err(err_response(e).into_response()),
    }
}

#[derive(Debug, Deserialize)]
pub struct ProviderInput {
    pub channel: String,
    pub enabled: bool,
    #[serde(default)]
    pub settings: serde_json::Value,
    /// New secret; omit or blank to keep the existing one (write-only).
    #[serde(default)]
    pub secret: Option<String>,
}

/// PUT /notification-providers — upsert one channel's provider config.
pub async fn put_provider(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ProviderInput>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use axum::response::IntoResponse;
    require_role(ROLES_MANAGE, &ctx, "update notification providers")
        .map_err(|e| err_response(e).into_response())?;

    let input = zavora_erp_core::services::notification_providers::UpsertProvider {
        channel: req.channel,
        enabled: req.enabled,
        settings: if req.settings.is_null() { serde_json::json!({}) } else { req.settings },
        secret: req.secret,
    };
    match zavora_erp_core::services::notification_providers::upsert(
        state.engine.pool(),
        ctx.entity_id,
        input,
        ctx.user_id,
    )
    .await
    {
        Ok(masked) => Ok(Json(serde_json::to_value(masked).unwrap_or_default())),
        Err(e) => Err(err_response(e).into_response()),
    }
}

#[derive(Debug, Deserialize)]
pub struct TestProviderInput {
    pub recipient: String,
}

/// POST /notification-providers/{channel}/test — send a one-off test message on
/// the channel to verify the configured (or env-fallback) provider works.
pub async fn test_provider(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(channel): Path<String>,
    Json(req): Json<TestProviderInput>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use axum::response::IntoResponse;
    require_role(ROLES_MANAGE, &ctx, "test notification provider")
        .map_err(|e| err_response(e).into_response())?;

    if req.recipient.trim().is_empty() {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: "A recipient (email or phone) is required to send a test.".to_string(),
        })
        .into_response());
    }

    match zavora_erp_core::services::notification_worker::send_test_message(
        state.engine.pool(),
        ctx.entity_id,
        &channel,
        req.recipient.trim(),
    )
    .await
    {
        Ok(()) => Ok(Json(serde_json::json!({ "ok": true, "message": "Test sent" }))),
        Err(msg) => Err(err_response(zavora_erp_core::ErpError::ValidationFailed { message: msg })
            .into_response()),
    }
}
