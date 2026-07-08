use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;

/// GET /recurring-journals
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, serde_json::Value, bool, bool, chrono::NaiveDate, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT id, name, cadence, lines, auto_reverse, is_active, next_run_date, last_run_at
         FROM recurring_journals WHERE entity_id = $1 ORDER BY name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();
    let items: Vec<_> = rows.into_iter().map(|(id, name, cadence, lines, auto_reverse, is_active, next_run_date, last_run_at)| {
        serde_json::json!({ "id": id, "name": name, "cadence": cadence, "lines": lines,
            "auto_reverse": auto_reverse, "is_active": is_active, "next_run_date": next_run_date, "last_run_at": last_run_at })
    }).collect();
    Json(serde_json::to_value(items).unwrap_or_default())
}

#[derive(serde::Deserialize)]
pub struct SaveRequest {
    pub id: Option<Uuid>,
    pub name: String,
    pub cadence: String,
    pub lines: serde_json::Value,
    pub auto_reverse: Option<bool>,
    pub is_active: Option<bool>,
    pub next_run_date: chrono::NaiveDate,
}

/// POST /recurring-journals — create or update a template (rejects unbalanced).
pub async fn save(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SaveRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {

    // Validate the template balances.
    let mut dr = rust_decimal::Decimal::ZERO;
    let mut cr = rust_decimal::Decimal::ZERO;
    if let Some(arr) = req.lines.as_array() {
        for l in arr {
            dr += l.get("debit").and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64().map(|f| rust_decimal::Decimal::try_from(f).unwrap_or_default()))).unwrap_or_default();
            cr += l.get("credit").and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64().map(|f| rust_decimal::Decimal::try_from(f).unwrap_or_default()))).unwrap_or_default();
        }
    }
    if (dr - cr).abs() >= rust_decimal::Decimal::new(1, 2) {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: format!("Recurring journal must balance — debits {} vs credits {}", dr, cr),
        }));
    }

    let auto_reverse = req.auto_reverse.unwrap_or(false);
    let is_active = req.is_active.unwrap_or(true);
    let res = if let Some(id) = req.id {
        sqlx::query("UPDATE recurring_journals SET name=$1, cadence=$2, lines=$3, auto_reverse=$4, is_active=$5, next_run_date=$6 WHERE id=$7 AND entity_id=$8")
            .bind(&req.name).bind(&req.cadence).bind(&req.lines).bind(auto_reverse).bind(is_active).bind(req.next_run_date).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.map(|_| id)
    } else {
        sqlx::query_scalar::<_, Uuid>("INSERT INTO recurring_journals (entity_id, name, cadence, lines, auto_reverse, is_active, next_run_date) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id")
            .bind(ctx.entity_id).bind(&req.name).bind(&req.cadence).bind(&req.lines).bind(auto_reverse).bind(is_active).bind(req.next_run_date)
            .fetch_one(state.engine.pool()).await
    };
    match res {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// POST /recurring-journals/run — post any templates due now for this entity.
pub async fn run_now(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match zavora_erp_core::services::scheduler::process_recurring_journals(&state.engine, ctx.entity_id).await {
        Ok(n) => Ok(Json(serde_json::json!({ "posted": n }))),
        Err(e) => Err(err_response(e)),
    }
}

/// DELETE /recurring-journals/{id}
pub async fn delete(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let res = sqlx::query("DELETE FROM recurring_journals WHERE id = $1 AND entity_id = $2")
        .bind(id).bind(ctx.entity_id).execute(state.engine.pool()).await;
    match res {
        Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}
