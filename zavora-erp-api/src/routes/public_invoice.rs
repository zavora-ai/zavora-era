//! Public (unauthenticated) invoice pay-link endpoints. Reached by a customer
//! via a random `public_token`; no `AuthContext` — the token is the credential.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::routes::err_response;
use crate::AppState;
use zavora_erp_core::services::public_invoice as svc;

/// GET /public/invoices/{token} — sanitized invoice summary; stamps `viewed_at`.
pub async fn get_public_invoice(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::get_public_invoice(&state.engine, &token).await {
        Ok(view) => Ok(Json(serde_json::to_value(view).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

#[derive(Debug, Deserialize)]
pub struct PublicPayRequest {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub callback_url: Option<String>,
}

/// POST /public/invoices/{token}/pay — start a Paystack card payment; returns
/// the `authorization_url` the browser redirects to.
pub async fn pay_public_invoice(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Json(req): Json<PublicPayRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::pay_public_invoice(&state.engine, &token, req.email, req.callback_url).await {
        Ok(res) => Ok(Json(serde_json::to_value(res).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
