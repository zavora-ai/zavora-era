use chrono::Utc;
use uuid::Uuid;

use crate::catalog::*;
use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::types::AgentOrUserId;

/// Create a product/service.
pub async fn create_product(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateProductRequest,
    _created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    let currency = req.currency.unwrap_or_else(|| engine.config().base_currency.clone());
    let uom = req.uom.unwrap_or(crate::types::UnitOfMeasure::Each);
    let sales_account = req.sales_account.unwrap_or_else(|| "5000".to_string());
    let purchase_account = req.purchase_account.unwrap_or_else(|| "6000".to_string());
    let vat_treatment = req.vat_treatment.unwrap_or(crate::types::VatTreatment::Standard16);

    sqlx::query(
        r#"INSERT INTO products 
           (id, entity_id, name, description, product_type, unit_price, currency, uom,
            sales_account, purchase_account, vat_treatment, track_inventory, is_active, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, true, $13)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(serde_json::to_string(&req.product_type).unwrap_or_default())
    .bind(req.unit_price)
    .bind(&currency)
    .bind(serde_json::to_string(&uom).unwrap_or_default())
    .bind(&sales_account)
    .bind(&purchase_account)
    .bind(serde_json::to_string(&vat_treatment).unwrap_or_default())
    .bind(req.track_inventory.unwrap_or(false))
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(id)
}
