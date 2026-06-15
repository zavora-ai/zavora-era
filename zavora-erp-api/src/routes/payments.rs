use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_CREATE};
use super::err_response;
use zavora_erp_core::payments::*;
use zavora_erp_core::services::payments as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn record(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<RecordPaymentRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "record payment").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::record_payment(&state.engine, req, &actor).await {
        Ok(payment) => Ok(Json(serde_json::to_value(payment).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

#[derive(serde::Deserialize)]
pub struct MpesaCallbackWrapper {
    pub invoice_id: uuid::Uuid,
    #[serde(flatten)]
    pub callback: MpesaCallback,
}

pub async fn mpesa_callback(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MpesaCallbackWrapper>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::record_mpesa_payment(&state.engine, req.invoice_id, req.callback).await {
        Ok(payment) => Ok(Json(serde_json::to_value(payment).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /payments/apply — Apply unapplied payment funds to a target document.
pub async fn apply_unapplied(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApplyPaymentRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "apply unapplied payment").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::apply_unapplied_payment(&state.engine, req, &actor).await {
        Ok(payment) => Ok(Json(serde_json::to_value(payment).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
