use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_CREATE, ROLES_SEND};
use super::err_response;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, zavora_erp_core::invoicing::EstimateRow>(
        "SELECT * FROM estimates WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_one(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, zavora_erp_core::invoicing::EstimateRow>(
        "SELECT * FROM estimates WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(ctx.entity_id)
    .fetch_optional(state.engine.pool())
    .await;

    // Estimate lines (estimate_lines.estimate_id aliased to invoice_id for the shared row type).
    let lines = sqlx::query_as::<_, zavora_erp_core::invoicing::InvoiceLineRow>(
        "SELECT id, estimate_id AS invoice_id, product_id, description, quantity, unit_price, \
                discount_percent, account_code, vat_treatment, line_total, vat_amount \
         FROM estimate_lines WHERE estimate_id = $1",
    )
    .bind(id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();

    match row {
        Ok(Some(est)) => Ok(Json(serde_json::json!({
            "estimate": serde_json::to_value(est).unwrap_or_default(),
            "lines": serde_json::to_value(lines).unwrap_or_default(),
        }))),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Estimate".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// POST /estimates — create a draft estimate (quote).
pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<zavora_erp_core::invoicing::CreateEstimateRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create estimate").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match zavora_erp_core::services::invoicing::create_estimate(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id, "status": "draft" }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /estimates/{id}/convert — convert an accepted estimate into an invoice.
pub async fn convert(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "convert estimate").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match zavora_erp_core::services::invoicing::convert_estimate_to_invoice(&state.engine, ctx.entity_id, id, &actor).await {
        Ok(invoice_id) => Ok(Json(serde_json::json!({ "status": "converted", "estimate_id": id, "invoice_id": invoice_id }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /estimates/{id}/send — mark a draft estimate as sent to the customer.
pub async fn send(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_SEND, &ctx, "send estimate").map_err(|e| {
        use axum::response::IntoResponse;
        err_response(e).into_response()
    })?;
    transition_estimate(&state, ctx.entity_id, id, &["draft", "sent"], "sent").await
}

/// POST /estimates/{id}/accept — record customer acceptance of the quote.
pub async fn accept(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_SEND, &ctx, "accept estimate").map_err(|e| {
        use axum::response::IntoResponse;
        err_response(e).into_response()
    })?;
    transition_estimate(&state, ctx.entity_id, id, &["draft", "sent"], "accepted").await
}

/// POST /estimates/{id}/decline — record customer decline of the quote.
pub async fn decline(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_SEND, &ctx, "decline estimate").map_err(|e| {
        use axum::response::IntoResponse;
        err_response(e).into_response()
    })?;
    transition_estimate(&state, ctx.entity_id, id, &["draft", "sent", "accepted"], "declined").await
}

/// Apply a guarded status transition to an estimate. Quotes are purely
/// commercial documents (not transmitted to KRA eTIMS), so transitions are
/// free-form aside from the from-state guard and the terminal `converted` state.
async fn transition_estimate(
    state: &Arc<AppState>,
    entity_id: Uuid,
    id: Uuid,
    allowed_from: &[&str],
    to: &str,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use axum::response::IntoResponse;
    let current = sqlx::query_scalar::<_, String>(
        "SELECT status FROM estimates WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(entity_id)
    .fetch_optional(state.engine.pool())
    .await
    .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)).into_response())?
    .ok_or_else(|| err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Estimate".into(), id }).into_response())?;

    if !allowed_from.contains(&current.as_str()) {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: format!("Cannot move estimate from '{current}' to '{to}'"),
        })
        .into_response());
    }

    sqlx::query("UPDATE estimates SET status = $1 WHERE id = $2 AND entity_id = $3")
        .bind(to)
        .bind(id)
        .bind(entity_id)
        .execute(state.engine.pool())
        .await
        .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)).into_response())?;

    Ok(Json(serde_json::json!({ "id": id, "status": to })))
}
