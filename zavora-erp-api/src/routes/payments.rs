use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use zavora_erp_core::payments::*;
use zavora_erp_core::services::payments as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn record(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RecordPaymentRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::record_payment(&state.engine, req, &actor).await {
        Ok(payment) => Ok(Json(serde_json::to_value(payment).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// M-Pesa Daraja callback endpoint.
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
