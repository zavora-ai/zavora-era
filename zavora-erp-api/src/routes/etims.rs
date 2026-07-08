//! KRA eTIMS OSCU/VSCU API — device config, initialisation, and invoice
//! transmission (real-time tax-invoice sign-off).

use axum::extract::{Path, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use super::err_response;
use crate::middleware::auth::{AuthContext};
use crate::AppState;
use zavora_erp_core::services::etims as svc;
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;

fn boxed(e: ErpError) -> axum::response::Response {
    use axum::response::IntoResponse;
    err_response(e).into_response()
}
fn ok<T: serde::Serialize>(v: T) -> ApiResult {
    Ok(Json(serde_json::to_value(v).unwrap_or_default()))
}

/// GET /api/v1/etims/config — the entity's eTIMS device config + status.
pub async fn get_config(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    ok(svc::get_device(&state.engine, ctx.entity_id).await.map_err(boxed)?)
}

/// PUT /api/v1/etims/config — update credentials / environment / enabled.
pub async fn save_config(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(patch): Json<svc::EtimsConfigPatch>) -> ApiResult {
    ok(svc::save_config(&state.engine, ctx.entity_id, patch).await.map_err(boxed)?)
}

/// POST /api/v1/etims/initialize — register the device with KRA (OSCU/VSCU init).
pub async fn initialize(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    ok(svc::initialize_device(&state.engine, ctx.entity_id).await.map_err(boxed)?)
}

/// POST /api/v1/etims/invoices/{id}/transmit — (re)transmit an invoice to KRA.
pub async fn transmit(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    ok(svc::transmit_invoice(&state.engine, ctx.entity_id, id).await.map_err(boxed)?)
}

/// POST /api/v1/etims/products/{id}/register — register a product with KRA.
pub async fn register_product(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    svc::register_item(&state.engine, ctx.entity_id, id).await.map_err(boxed)?;
    ok(serde_json::json!({ "status": "registered", "product_id": id }))
}
