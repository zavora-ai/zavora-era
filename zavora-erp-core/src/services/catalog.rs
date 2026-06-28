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
    let currency = match req.currency.clone() {
        Some(c) => c,
        None => engine.config_for(entity_id).await?.base_currency.clone(),
    };
    let uom = req.uom.unwrap_or(crate::types::UnitOfMeasure::Each);
    let posting = engine.posting_for(entity_id).await?;
    let sales_account = req.sales_account.unwrap_or_else(|| posting.default_sales.clone());
    let purchase_account = req.purchase_account.unwrap_or_else(|| posting.default_purchase.clone());
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
    .bind(serde_json::to_string(&req.product_type).unwrap_or_default().trim_matches('"').to_string())
    .bind(req.unit_price)
    .bind(&currency)
    .bind(serde_json::to_string(&uom).unwrap_or_default().trim_matches('"').to_string())
    .bind(&sales_account)
    .bind(&purchase_account)
    .bind(serde_json::to_string(&vat_treatment).unwrap_or_default().trim_matches('"').to_string())
    .bind(req.track_inventory.unwrap_or(false))
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(id)
}
