use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_MANAGE};
use super::err_response;

/// GET /wht-rates — configured WHT rates (the single source of truth).
pub async fn list(
    _ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let rows = sqlx::query_as::<_, (String, rust_decimal::Decimal, rust_decimal::Decimal)>(
        "SELECT category, resident_rate, non_resident_rate FROM wht_rates ORDER BY category",
    )
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();
    let items: Vec<_> = rows
        .into_iter()
        .map(|(category, resident_rate, non_resident_rate)| {
            serde_json::json!({ "category": category, "resident_rate": resident_rate, "non_resident_rate": non_resident_rate })
        })
        .collect();
    Json(serde_json::to_value(items).unwrap_or_default())
}

#[derive(serde::Deserialize)]
pub struct UpdateRateRequest {
    pub category: String,
    pub resident_rate: rust_decimal::Decimal,
    pub non_resident_rate: rust_decimal::Decimal,
}

/// PUT /wht-rates — set the rates for a category. Rates are national, so this is
/// shared; restricted to managers.
pub async fn update(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateRateRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "manage WHT rates").map_err(err_response)?;
    let res = sqlx::query(
        "INSERT INTO wht_rates (category, resident_rate, non_resident_rate) VALUES ($1, $2, $3)
         ON CONFLICT (category) DO UPDATE SET resident_rate = EXCLUDED.resident_rate,
            non_resident_rate = EXCLUDED.non_resident_rate, updated_at = NOW()",
    )
    .bind(req.category.trim())
    .bind(req.resident_rate)
    .bind(req.non_resident_rate)
    .execute(state.engine.pool())
    .await;
    match res {
        Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}
