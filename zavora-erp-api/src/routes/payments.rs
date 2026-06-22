use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_CREATE};
use super::err_response;
use zavora_erp_core::payments::*;
use zavora_erp_core::services::payments as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    axum::extract::Query(page): axum::extract::Query<crate::routes::pagination::PaginationParams>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payments WHERE entity_id = $1")
        .bind(ctx.entity_id).fetch_one(state.engine.pool()).await.unwrap_or(0);
    let rows = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE entity_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(ctx.entity_id).bind(page.effective_limit()).bind(page.effective_offset())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(crate::routes::pagination::PaginatedResponse::new(r, total, &page)).unwrap_or_default())),
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
    match svc::record_payment(&state.engine, ctx.entity_id, req, &actor).await {
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
    match svc::record_mpesa_payment(&state.engine, state.engine.entity_id(), req.invoice_id, req.callback).await {
        Ok(payment) => Ok(Json(serde_json::to_value(payment).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

#[derive(serde::Deserialize)]
pub struct MpesaStkPushBody {
    pub invoice_id: uuid::Uuid,
    #[serde(default)]
    #[allow(dead_code)] // part of the request contract; used once a Daraja gateway is configured
    pub phone: Option<String>,
}

/// POST /payments/mpesa-stk-push — initiate an STK Push for an invoice.
///
/// Validates the invoice and the entity's M-Pesa configuration. Daraja gateway calls
/// require provider credentials that are not part of this deployment, so when M-Pesa is
/// not enabled this returns a clear, actionable error instead of failing opaquely.
pub async fn mpesa_stk_push(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<MpesaStkPushBody>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "initiate M-Pesa payment").map_err(err_response)?;

    // Ensure the invoice exists and belongs to this entity.
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM invoices WHERE id = $1 AND entity_id = $2)",
    )
    .bind(req.invoice_id)
    .bind(ctx.entity_id)
    .fetch_one(state.engine.pool())
    .await
    .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)))?;

    if !exists {
        return Err(err_response(zavora_erp_core::ErpError::NotFound {
            entity_type: "Invoice".to_string(),
            id: req.invoice_id,
        }));
    }

    if !state.engine.config().payment_config.mpesa_enabled {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: "M-Pesa is not enabled in payment settings. Enable it and configure Daraja credentials to use STK Push.".to_string(),
        }));
    }

    // Daraja gateway integration is provisioned outside this deployment.
    Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
        message: "M-Pesa STK Push gateway is not configured for this deployment.".to_string(),
    }))
}

/// POST /payments/apply — Apply unapplied payment funds to a target document.
pub async fn apply_unapplied(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApplyPaymentRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "apply unapplied payment").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::apply_unapplied_payment(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(payment) => Ok(Json(serde_json::to_value(payment).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
