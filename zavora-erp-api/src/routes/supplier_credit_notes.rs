use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_CREATE};
use super::err_response;
use zavora_erp_core::ap::CreateSupplierCreditNoteRequest;
use zavora_erp_core::services::supplier_credit_notes as svc;
use zavora_erp_core::AgentOrUserId;

/// GET /supplier-credit-notes — list AP credit notes for the tenant.
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::list_supplier_credit_notes(&state.engine, ctx.entity_id).await {
        Ok(rows) => Ok(Json(serde_json::to_value(rows).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// GET /supplier-credit-notes/{id} — fetch one (header + line items).
pub async fn get_one(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::get_supplier_credit_note(&state.engine, ctx.entity_id, id).await {
        Ok(Some(cn)) => {
            let lines = sqlx::query_as::<_, zavora_erp_core::invoicing::InvoiceLineRow>(
                "SELECT id, credit_note_id AS invoice_id, product_id, description, quantity, \
                        unit_price, 0::numeric AS discount_percent, gl_account_code AS account_code, \
                        vat_treatment, line_total, vat_amount \
                 FROM supplier_credit_note_lines WHERE credit_note_id = $1",
            )
            .bind(id)
            .fetch_all(state.engine.pool())
            .await
            .unwrap_or_default();
            Ok(Json(serde_json::json!({
                "credit_note": serde_json::to_value(cn).unwrap_or_default(),
                "lines": serde_json::to_value(lines).unwrap_or_default(),
            })))
        }
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "SupplierCreditNote".into(), id })),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /supplier-credit-notes — create + post a supplier credit note.
pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSupplierCreditNoteRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create supplier credit note").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_supplier_credit_note(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(cn) => Ok(Json(serde_json::to_value(cn).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
