use axum::{extract::{Query, State}, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{AuthContext};
use zavora_erp_core::fx::*;
use zavora_erp_core::services::fx as svc;
use zavora_erp_core::AgentOrUserId;

/// Optional `?date=` for the revaluation; defaults to today.
#[derive(Debug, Default, serde::Deserialize)]
pub struct RevaluationParams {
    pub date: Option<chrono::NaiveDate>,
}

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, ExchangeRateRow>(
        "SELECT * FROM exchange_rates WHERE entity_id = $1 ORDER BY rate_date DESC LIMIT 100",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn upsert(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertRateRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::upsert_rate(&state.engine, ctx.entity_id, req).await {
        Ok(rate) => Ok(Json(serde_json::to_value(rate).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// DELETE /fx-rates/{id} — remove a single exchange-rate row, scoped to the tenant.
pub async fn delete(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let result = sqlx::query("DELETE FROM exchange_rates WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(ctx.entity_id)
        .execute(state.engine.pool())
        .await;
    match result {
        Ok(r) if r.rows_affected() == 0 => Err(err_response(zavora_erp_core::ErpError::NotFound {
            entity_type: "ExchangeRate".to_string(),
            id,
        })),
        Ok(_) => Ok(Json(serde_json::json!({ "status": "deleted", "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// POST /fx/revaluation — revalue open foreign-currency balances at the latest
/// rate as of `date` (default today), posting unrealised FX gain/loss plus an
/// auto-reversal in the next period.
pub async fn revaluation(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Query(params): Query<RevaluationParams>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {

    let rate_date = params.date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    let period = zavora_erp_core::services::periods::period_for_date(&state.engine, ctx.entity_id, rate_date)
        .await
        .map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);

    match svc::run_fx_revaluation(&state.engine, ctx.entity_id, period.id, rate_date, actor).await {
        Ok(entry_id) => Ok(Json(serde_json::json!({ "status": "posted", "journal_entry_id": entry_id }))),
        Err(e) => Err(err_response(e)),
    }
}
