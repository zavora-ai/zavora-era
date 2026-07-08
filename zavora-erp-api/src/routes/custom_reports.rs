use axum::{extract::{Path, Query, State}, Json};
use chrono::Datelike;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;

/// GET /custom-reports — saved definitions (id + name only).
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM custom_report_definitions WHERE entity_id = $1 ORDER BY name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();
    let items: Vec<_> = rows.into_iter().map(|(id, name)| serde_json::json!({ "id": id, "name": name })).collect();
    Json(serde_json::to_value(items).unwrap_or_default())
}

/// GET /custom-reports/{id} — full definition.
pub async fn get(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, (Uuid, String, serde_json::Value)>(
        "SELECT id, name, definition FROM custom_report_definitions WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(ctx.entity_id)
    .fetch_optional(state.engine.pool())
    .await;
    match row {
        Ok(Some((id, name, definition))) => Ok(Json(serde_json::json!({ "id": id, "name": name, "definition": definition }))),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "CustomReport".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

#[derive(serde::Deserialize)]
pub struct SaveRequest {
    pub id: Option<Uuid>,
    pub name: String,
    pub definition: serde_json::Value,
}

/// POST /custom-reports — create or update a definition.
pub async fn save(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SaveRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let res = if let Some(id) = req.id {
        sqlx::query("UPDATE custom_report_definitions SET name = $1, definition = $2, updated_at = NOW() WHERE id = $3 AND entity_id = $4")
            .bind(&req.name).bind(&req.definition).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.map(|_| id)
    } else {
        sqlx::query_scalar::<_, Uuid>("INSERT INTO custom_report_definitions (entity_id, name, definition) VALUES ($1, $2, $3) RETURNING id")
            .bind(ctx.entity_id).bind(&req.name).bind(&req.definition)
            .fetch_one(state.engine.pool()).await
    };
    match res {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// DELETE /custom-reports/{id}
pub async fn delete(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let res = sqlx::query("DELETE FROM custom_report_definitions WHERE id = $1 AND entity_id = $2")
        .bind(id).bind(ctx.entity_id).execute(state.engine.pool()).await;
    match res {
        Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

#[derive(serde::Deserialize)]
pub struct RunQuery {
    pub from: Option<chrono::NaiveDate>,
    pub to: Option<chrono::NaiveDate>,
}

/// GET /custom-reports/{id}/run — compute each row's amount over the period.
/// account_range rows sum GL movement in the chosen natural sign; subtotal rows
/// sum referenced rows by key; header rows carry no amount.
pub async fn run(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(q): Query<RunQuery>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let today = chrono::Utc::now().date_naive();
    let period_to = q.to.unwrap_or(today);
    let period_from = q.from.unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(period_to.year(), 1, 1).unwrap_or(today));

    let row = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT name, definition FROM custom_report_definitions WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await;
    let (name, definition) = match row {
        Ok(Some(r)) => r,
        Ok(None) => return Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "CustomReport".into(), id })),
        Err(e) => return Err(err_response(zavora_erp_core::ErpError::Database(e))),
    };

    let rows = definition.get("rows").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut amounts: HashMap<String, rust_decimal::Decimal> = HashMap::new();
    let mut out_rows = Vec::new();

    for r in &rows {
        let key = r.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let kind = r.get("kind").and_then(|v| v.as_str()).unwrap_or("header");
        let label = r.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let bold = r.get("bold").and_then(|v| v.as_bool()).unwrap_or(kind == "subtotal");

        let amount: Option<rust_decimal::Decimal> = match kind {
            "accounts" => {
                let from_code = r.get("from_code").and_then(|v| v.as_str()).unwrap_or("");
                let to_code = r.get("to_code").and_then(|v| v.as_str()).unwrap_or("");
                let credit_natural = r.get("sign").and_then(|v| v.as_str()) == Some("credit");
                let movement = sqlx::query_scalar::<_, rust_decimal::Decimal>(
                    "SELECT COALESCE(SUM(COALESCE(functional_debit,0) - COALESCE(functional_credit,0)), 0)
                     FROM journal_lines
                     WHERE entity_id = $1 AND entry_date BETWEEN $2 AND $3
                       AND account_code >= $4 AND account_code <= $5",
                )
                .bind(ctx.entity_id).bind(period_from).bind(period_to).bind(from_code).bind(to_code)
                .fetch_one(state.engine.pool()).await.unwrap_or(rust_decimal::Decimal::ZERO);
                let val = if credit_natural { -movement } else { movement };
                amounts.insert(key.clone(), val);
                Some(val)
            }
            "subtotal" => {
                let refs = r.get("refs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let mut sum = rust_decimal::Decimal::ZERO;
                for rf in refs {
                    if let Some(k) = rf.as_str() {
                        sum += amounts.get(k).copied().unwrap_or(rust_decimal::Decimal::ZERO);
                    }
                }
                amounts.insert(key.clone(), sum);
                Some(sum)
            }
            _ => None, // header
        };

        out_rows.push(serde_json::json!({ "key": key, "kind": kind, "label": label, "bold": bold, "amount": amount }));
    }

    Ok(Json(serde_json::json!({
        "name": name,
        "period_from": period_from,
        "period_to": period_to,
        "rows": out_rows,
    })))
}
