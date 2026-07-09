use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;
use zavora_erp_core::payments::*;
use zavora_erp_core::services::payments as svc;
use zavora_erp_core::AgentOrUserId;

/// GET /payments/{id} — single payment (used by the receipt preview).
pub async fn get_one(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(ctx.entity_id)
    .fetch_optional(state.engine.pool())
    .await;
    match row {
        Ok(Some(r)) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Payment".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

// Flat query struct: `#[serde(flatten)]` is NOT supported by the urlencoded
// query deserializer, so list the fields explicitly.
#[derive(serde::Deserialize)]
pub struct PaymentListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// `status=unapplied` returns only payments that still carry unapplied credit.
    pub status: Option<String>,
}

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<PaymentListQuery>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let page = crate::routes::pagination::PaginationParams { limit: q.limit, offset: q.offset };
    let page = &page;
    // The "Unapplied" tab requests only payments that still have credit to allocate.
    let filter = if q.status.as_deref() == Some("unapplied") { " AND unapplied > 0" } else { "" };

    let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM payments WHERE entity_id = $1{filter}"))
        .bind(ctx.entity_id).fetch_one(state.engine.pool()).await.unwrap_or(0);
    let rows = sqlx::query_as::<_, PaymentRow>(
        &format!("SELECT * FROM payments WHERE entity_id = $1{filter} ORDER BY created_at DESC LIMIT $2 OFFSET $3"),
    )
    .bind(ctx.entity_id).bind(page.effective_limit()).bind(page.effective_offset())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(crate::routes::pagination::PaginatedResponse::new(r, total, page)).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn record(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<RecordPaymentRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
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

#[derive(serde::Deserialize, Default)]
pub struct MpesaCallbackAuth {
    /// Shared secret embedded in the registered callback URL (`?token=…`).
    #[serde(default)]
    pub token: Option<String>,
}

/// Authenticate a Daraja callback. Daraja does not sign payloads, so we rely on
/// (1) a hard-to-guess secret in the registered callback URL and (2) an optional
/// source-IP allowlist. Both are env-driven and OFF by default (dev), but a
/// production deployment MUST set at least `MPESA_CALLBACK_SECRET`. Returns the
/// refusal message when the request fails a configured check.
fn authenticate_mpesa_callback(peer_ip: std::net::IpAddr, token: Option<&str>) -> Result<(), String> {
    if let Ok(secret) = std::env::var("MPESA_CALLBACK_SECRET") {
        let secret = secret.trim();
        if !secret.is_empty() && token.map(str::trim) != Some(secret) {
            return Err("Invalid or missing callback token".to_string());
        }
    }
    if let Ok(allowed) = std::env::var("MPESA_CALLBACK_ALLOWED_IPS") {
        let allowed = allowed.trim();
        if !allowed.is_empty() {
            let ip = peer_ip.to_string();
            let ok = allowed.split(',').map(str::trim).any(|a| a == ip);
            if !ok {
                return Err(format!("Source IP {ip} is not in the M-Pesa callback allowlist"));
            }
        }
    }
    Ok(())
}

pub async fn mpesa_callback(
    State(state): State<Arc<AppState>>,
    axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<std::net::SocketAddr>,
    axum::extract::Query(auth): axum::extract::Query<MpesaCallbackAuth>,
    Json(req): Json<MpesaCallbackWrapper>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    // Daraja calls this unauthenticated, so verify the URL secret / IP allowlist
    // before touching the ledger — otherwise anyone who learns an invoice id can
    // forge a "paid" callback.
    if let Err(msg) = authenticate_mpesa_callback(peer.ip(), auth.token.as_deref()) {
        tracing::warn!(peer = %peer.ip(), "rejected M-Pesa callback: {msg}");
        return Err(err_response(zavora_erp_core::ErpError::Unauthorized { message: msg }));
    }

    // The tenant cannot be derived from a JWT. Resolve it from the invoice the
    // payment is for — NOT the process-global startup entity, which would
    // mis-post in multi-tenant deployments.
    let entity_id = match sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT entity_id FROM invoices WHERE id = $1",
    )
    .bind(req.invoice_id)
    .fetch_optional(state.engine.pool())
    .await
    {
        Ok(Some(eid)) => eid,
        Ok(None) => {
            return Err(err_response(zavora_erp_core::ErpError::NotFound {
                entity_type: "Invoice".to_string(),
                id: req.invoice_id,
            }))
        }
        Err(e) => return Err(err_response(zavora_erp_core::ErpError::Database(e))),
    };

    match svc::record_mpesa_payment(&state.engine, entity_id, req.invoice_id, req.callback).await {
        Ok(payment) => Ok(Json(serde_json::to_value(payment).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

#[derive(serde::Deserialize)]
pub struct MpesaStkPushBody {
    pub invoice_id: uuid::Uuid,
    /// Optional override; falls back to the customer's primary phone.
    #[serde(default)]
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

    // Build the Daraja client from deployment credentials. Absent → clear error.
    let client = match zavora_erp_core::payments::daraja::DarajaClient::from_env() {
        Some(c) => c,
        None => {
            return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
                message: "M-Pesa STK Push is not configured on this deployment (set MPESA_CONSUMER_KEY/SECRET, MPESA_SHORTCODE, MPESA_PASSKEY, MPESA_CALLBACK_URL).".to_string(),
            }));
        }
    };

    // Resolve the amount due and a phone number for the prompt.
    let (number, balance_due, customer_phone) = sqlx::query_as::<_, (String, rust_decimal::Decimal, serde_json::Value)>(
        "SELECT i.number, i.balance_due, COALESCE(c.phone, '[]'::jsonb)
         FROM invoices i LEFT JOIN customers c ON c.id = i.customer_id
         WHERE i.id = $1 AND i.entity_id = $2",
    )
    .bind(req.invoice_id)
    .bind(ctx.entity_id)
    .fetch_one(state.engine.pool())
    .await
    .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)))?;

    // Prefer the explicit phone in the request, else the customer's primary phone.
    let phone = req.phone.clone().or_else(|| {
        serde_json::from_value::<Vec<zavora_erp_core::types::ContactPhone>>(customer_phone)
            .ok()
            .and_then(|ps| ps.into_iter().find(|p| !p.number.is_empty()).map(|p| p.number))
    });
    let phone = match phone {
        Some(p) => p,
        None => {
            return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
                message: "No phone number for the STK Push. Provide one or set the customer's phone.".to_string(),
            }))
        }
    };

    if balance_due <= rust_decimal::Decimal::ZERO {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: "Invoice has no outstanding balance to collect.".to_string(),
        }));
    }

    match client
        .stk_push(&phone, balance_due, &number, &format!("Payment for {number}"))
        .await
    {
        Ok(r) => Ok(Json(serde_json::json!({
            "checkout_request_id": r.checkout_request_id,
            "merchant_request_id": r.merchant_request_id,
            "response_code": r.response_code,
            "customer_message": r.customer_message,
        }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /payments/apply — Apply unapplied payment funds to a target document.
pub async fn apply_unapplied(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApplyPaymentRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::apply_unapplied_payment(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(payment) => Ok(Json(serde_json::to_value(payment).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
