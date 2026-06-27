use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_CREATE};
use zavora_erp_core::invoicing::{CreateTemplateRequest, InvoiceTemplateRow};

/// GET /invoice-templates — list the entity's invoice templates (for the send
/// dialog's template picker and template management).
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, InvoiceTemplateRow>(
        "SELECT * FROM invoice_templates WHERE entity_id = $1 ORDER BY is_default DESC, name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// POST /invoice-templates — create a template.
pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTemplateRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create invoice template").map_err(err_response)?;
    let id = uuid::Uuid::new_v4();
    let layout = req
        .layout
        .map(|l| serde_json::to_string(&l).unwrap_or_default().trim_matches('"').to_string())
        .unwrap_or_else(|| "Modern".to_string());
    let result = sqlx::query(
        r#"INSERT INTO invoice_templates
           (id, entity_id, name, logo_url, primary_color, secondary_color, font, footer_text,
            show_bank_details, show_mpesa_paybill, layout, is_default)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
    )
    .bind(id)
    .bind(ctx.entity_id)
    .bind(&req.name)
    .bind(&req.logo_url)
    .bind(req.primary_color.clone().unwrap_or_else(|| "#1a56db".to_string()))
    .bind(&req.secondary_color)
    .bind(req.font.clone().unwrap_or_else(|| "Inter".to_string()))
    .bind(&req.footer_text)
    .bind(req.show_bank_details.unwrap_or(true))
    .bind(req.show_mpesa_paybill.unwrap_or(true))
    .bind(&layout)
    .bind(req.is_default.unwrap_or(false))
    .execute(state.engine.pool())
    .await;
    match result {
        Ok(_) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}
