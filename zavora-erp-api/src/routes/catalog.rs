use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_CREATE};
use zavora_erp_core::catalog::*;
use zavora_erp_core::services::catalog as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list_products(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, ProductRow>(
        "SELECT * FROM products WHERE entity_id = $1 AND is_active = true ORDER BY name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => {
            let r: Vec<ProductRow> = r.into_iter().map(|p| p.normalized()).collect();
            Ok(Json(serde_json::to_value(r).unwrap_or_default()))
        }
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_product(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, ProductRow>(
        "SELECT * FROM products WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await;
    match row {
        Ok(Some(r)) => Ok(Json(serde_json::to_value(r.normalized()).unwrap_or_default())),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Product".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_product(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateProductRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create product").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_product(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update_product(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProductRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    if let Err(e) = require_role(ROLES_CREATE, &ctx, "update product") { return Err(err_response(e)); }

    // Confirm the product belongs to the tenant before mutating.
    let exists = sqlx::query_scalar::<_, Uuid>("SELECT id FROM products WHERE id = $1 AND entity_id = $2")
        .bind(id).bind(ctx.entity_id)
        .fetch_optional(state.engine.pool()).await
        .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)))?;
    if exists.is_none() {
        return Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Product".into(), id }));
    }

    // COALESCE keeps any field the caller omitted. Enum columns are stored as
    // BARE strings (e.g. Service, Exempt) — matching create_product and what the
    // invoicing/grouping readers expect — so trim the quotes serde adds.
    let product_type = req.product_type.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default().trim_matches('"').to_string());
    let vat_treatment = req.vat_treatment.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default().trim_matches('"').to_string());
    let uom = req.uom.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default().trim_matches('"').to_string());

    let result = sqlx::query(
        "UPDATE products SET
            name             = COALESCE($1, name),
            description      = CASE WHEN $2::bool THEN $3 ELSE description END,
            product_type     = COALESCE($4, product_type),
            unit_price       = CASE WHEN $5::bool THEN $6 ELSE unit_price END,
            currency         = COALESCE($7, currency),
            uom              = COALESCE($8, uom),
            sales_account    = COALESCE($9, sales_account),
            purchase_account = COALESCE($10, purchase_account),
            vat_treatment    = COALESCE($11, vat_treatment),
            track_inventory  = COALESCE($12, track_inventory),
            is_active        = COALESCE($13, is_active)
         WHERE id = $14 AND entity_id = $15",
    )
    .bind(req.name)
    .bind(req.description.is_some()).bind(req.description.flatten())
    .bind(product_type)
    .bind(req.unit_price.is_some()).bind(req.unit_price.flatten())
    .bind(req.currency)
    .bind(uom)
    .bind(req.sales_account)
    .bind(req.purchase_account)
    .bind(vat_treatment)
    .bind(req.track_inventory)
    .bind(req.is_active)
    .bind(id).bind(ctx.entity_id)
    .execute(state.engine.pool()).await;

    match result {
        Ok(_) => Ok(Json(serde_json::json!({ "id": id, "updated": true }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// DELETE /products/{id} — remove a product, but only when it has never been
/// used on a transaction (invoice / bill / estimate / supplier credit note).
/// Otherwise we refuse, so historical documents keep their product link and the
/// books stay intact; the caller can deactivate instead.
pub async fn delete_product(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    if let Err(e) = require_role(ROLES_CREATE, &ctx, "delete product") { return Err(err_response(e)); }

    let exists = sqlx::query_scalar::<_, Uuid>("SELECT id FROM products WHERE id = $1 AND entity_id = $2")
        .bind(id).bind(ctx.entity_id)
        .fetch_optional(state.engine.pool()).await
        .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)))?;
    if exists.is_none() {
        return Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Product".into(), id }));
    }

    // Block deletion if the product is referenced by any transaction line.
    let used: i64 = sqlx::query_scalar(
        "SELECT
            (SELECT COUNT(*) FROM invoice_lines WHERE product_id = $1)
          + (SELECT COUNT(*) FROM bill_lines WHERE product_id = $1)
          + (SELECT COUNT(*) FROM estimate_lines WHERE product_id = $1)
          + (SELECT COUNT(*) FROM supplier_credit_note_lines WHERE product_id = $1)",
    )
    .bind(id)
    .fetch_one(state.engine.pool()).await
    .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)))?;

    if used > 0 {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: format!(
                "This product is used on {used} transaction line(s) and cannot be deleted. Deactivate it instead to hide it from new documents."
            ),
        }));
    }

    sqlx::query("DELETE FROM products WHERE id = $1 AND entity_id = $2")
        .bind(id).bind(ctx.entity_id)
        .execute(state.engine.pool()).await
        .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)))?;
    Ok(Json(serde_json::json!({ "id": id, "deleted": true })))
}
