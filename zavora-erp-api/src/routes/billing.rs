use axum::{extract::State, Json};
use std::sync::Arc;

use crate::middleware::auth::AuthContext;
use crate::AppState;
use super::err_response;

#[derive(serde::Deserialize)]
pub struct CheckoutBody {
    pub plan: String,
    /// Where Paystack returns the payer after payment (the app dashboard).
    #[serde(default)]
    pub callback_url: Option<String>,
}

/// POST /billing/checkout — start a subscription payment for the caller's plan.
///
/// Called right after signup with the new owner's access token. Free plans
/// activate immediately; paid plans return a Paystack `authorization_url` the
/// browser redirects to (card / M-Pesa / bank). The plan PRICE is resolved
/// server-side, so the amount can't be tampered with client-side.
pub async fn checkout(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CheckoutBody>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    // The payer email is the authenticated owner's — from the token's user,
    // never the request body.
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM era_users WHERE id = $1")
        .bind(ctx.user_id)
        .fetch_optional(state.engine.pool())
        .await
        .ok()
        .flatten();
    let email = email.unwrap_or_default();

    match zavora_erp_core::services::billing::start_checkout(
        &state.engine,
        ctx.entity_id,
        &email,
        &body.plan,
        body.callback_url,
    )
    .await
    {
        Ok(res) => Ok(Json(serde_json::to_value(res).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /billing/cancel — cancel the caller tenant's subscription. Access
/// continues until the paid-through date; renewal stops.
pub async fn cancel(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match zavora_erp_core::services::billing::cancel(&state.engine, ctx.entity_id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "cancelled" }))),
        Err(e) => Err(err_response(e)),
    }
}

/// GET /billing/subscription — the caller tenant's current subscription state.
pub async fn get_subscription(ctx: AuthContext, State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let sub: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT subscription FROM entity_settings WHERE entity_id = $1")
            .bind(ctx.entity_id)
            .fetch_optional(state.engine.pool())
            .await
            .ok()
            .flatten();
    Json(sub.unwrap_or_else(|| serde_json::json!({})))
}
