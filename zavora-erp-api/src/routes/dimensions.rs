use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;

/// GET /dimensions — dimension types, each with its values nested.
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let types = sqlx::query_as::<_, (String, String, bool)>(
        "SELECT code, name, is_active FROM dimension_types WHERE entity_id = $1 ORDER BY code",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();

    let values = sqlx::query_as::<_, (String, String, String, bool)>(
        "SELECT type_code, code, name, is_active FROM dimension_values WHERE entity_id = $1 ORDER BY type_code, code",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();

    let out: Vec<_> = types
        .into_iter()
        .map(|(code, name, is_active)| {
            let vals: Vec<_> = values
                .iter()
                .filter(|(tc, ..)| *tc == code)
                .map(|(_, vc, vn, va)| serde_json::json!({ "code": vc, "name": vn, "is_active": va }))
                .collect();
            serde_json::json!({ "code": code, "name": name, "is_active": is_active, "values": vals })
        })
        .collect();

    Json(serde_json::to_value(out).unwrap_or_default())
}

#[derive(serde::Deserialize)]
pub struct CreateTypeRequest {
    pub code: String,
    pub name: String,
}

/// POST /dimension-types — define a dimension type (e.g. Cost Centre).
pub async fn create_type(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTypeRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let res = sqlx::query(
        "INSERT INTO dimension_types (entity_id, code, name) VALUES ($1, $2, $3)
         ON CONFLICT (entity_id, code) DO UPDATE SET name = EXCLUDED.name",
    )
    .bind(ctx.entity_id)
    .bind(req.code.trim())
    .bind(req.name.trim())
    .execute(state.engine.pool())
    .await;
    match res {
        Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

#[derive(serde::Deserialize)]
pub struct CreateValueRequest {
    pub type_code: String,
    pub code: String,
    pub name: String,
}

/// POST /dimension-values — add a value to a dimension type.
pub async fn create_value(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateValueRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let res = sqlx::query(
        "INSERT INTO dimension_values (entity_id, type_code, code, name) VALUES ($1, $2, $3, $4)
         ON CONFLICT (entity_id, type_code, code) DO UPDATE SET name = EXCLUDED.name",
    )
    .bind(ctx.entity_id)
    .bind(req.type_code.trim())
    .bind(req.code.trim())
    .bind(req.name.trim())
    .execute(state.engine.pool())
    .await;
    match res {
        Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}
