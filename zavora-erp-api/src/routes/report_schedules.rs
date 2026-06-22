use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_MANAGE};
use super::err_response;

/// GET /report-schedules — schedules for the entity.
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, String, String, bool, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT id, name, report_type, cadence, recipients, is_active, next_run_at, last_run_at
         FROM report_schedules WHERE entity_id = $1 ORDER BY name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();
    let items: Vec<_> = rows.into_iter().map(|(id, name, report_type, cadence, recipients, is_active, next_run_at, last_run_at)| {
        serde_json::json!({ "id": id, "name": name, "report_type": report_type, "cadence": cadence,
            "recipients": recipients, "is_active": is_active, "next_run_at": next_run_at, "last_run_at": last_run_at })
    }).collect();
    Json(serde_json::to_value(items).unwrap_or_default())
}

#[derive(serde::Deserialize)]
pub struct SaveRequest {
    pub id: Option<Uuid>,
    pub name: String,
    pub report_type: String,
    pub cadence: String,
    pub recipients: String,
    pub is_active: Option<bool>,
}

/// POST /report-schedules — create or update a schedule.
pub async fn save(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SaveRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "manage report schedules").map_err(err_response)?;
    let active = req.is_active.unwrap_or(true);
    let res = if let Some(id) = req.id {
        sqlx::query("UPDATE report_schedules SET name=$1, report_type=$2, cadence=$3, recipients=$4, is_active=$5 WHERE id=$6 AND entity_id=$7")
            .bind(&req.name).bind(&req.report_type).bind(&req.cadence).bind(&req.recipients).bind(active).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.map(|_| id)
    } else {
        // New schedules run on the next scheduler tick (next_run_at left NULL).
        sqlx::query_scalar::<_, Uuid>("INSERT INTO report_schedules (entity_id, name, report_type, cadence, recipients, is_active) VALUES ($1,$2,$3,$4,$5,$6) RETURNING id")
            .bind(ctx.entity_id).bind(&req.name).bind(&req.report_type).bind(&req.cadence).bind(&req.recipients).bind(active)
            .fetch_one(state.engine.pool()).await
    };
    match res {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// DELETE /report-schedules/{id}
pub async fn delete(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "manage report schedules").map_err(err_response)?;
    let res = sqlx::query("DELETE FROM report_schedules WHERE id = $1 AND entity_id = $2")
        .bind(id).bind(ctx.entity_id).execute(state.engine.pool()).await;
    match res {
        Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}
