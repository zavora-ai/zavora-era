use chrono::Utc;
use uuid::Uuid;

use crate::catalog::*;
use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::types::AgentOrUserId;

/// Create a product/service.
///
/// When `track_inventory` is true a linked inventory item is created under the
/// provided SKU (required) and `products.inventory_item_id` is set — without
/// that link, posting an invoice for the product fails at stock-issue time.
/// Optional opening stock (quantity + unit cost) posts DR inventory /
/// CR opening-balance equity.
pub async fn create_product(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateProductRequest,
    created_by: &AgentOrUserId,
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
    let track_inventory = req.track_inventory.unwrap_or(false);

    // Validate the stock-master side BEFORE inserting the product, so a bad
    // request doesn't leave an orphan product with a broken inventory link.
    let sku = req.sku.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if track_inventory && sku.is_none() {
        return Err(crate::error::ErpError::ValidationFailed {
            message: "SKU is required for an inventory-tracked product".to_string(),
        });
    }
    let opening_qty = req.opening_stock.unwrap_or_default();
    let opening_cost = req.opening_unit_cost.unwrap_or_default();
    if opening_qty < rust_decimal::Decimal::ZERO {
        return Err(crate::error::ErpError::ValidationFailed {
            message: "Opening stock cannot be negative".to_string(),
        });
    }
    if opening_qty > rust_decimal::Decimal::ZERO && opening_cost <= rust_decimal::Decimal::ZERO {
        return Err(crate::error::ErpError::ValidationFailed {
            message: "Opening stock needs a unit cost — stock without a cost corrupts weighted-average costing"
                .to_string(),
        });
    }

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
    .bind(track_inventory)
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    if track_inventory {
        let sku = sku.expect("validated above");
        let item_id = crate::services::inventory::create_item(
            engine,
            entity_id,
            crate::inventory::CreateInventoryItemRequest {
                sku,
                description: req.name.clone(),
                uom: Some(serde_json::to_string(&uom).unwrap_or_default().trim_matches('"').to_string()),
                costing_method: None,
                gl_inventory: None,
                gl_cogs: None,
                reorder_point: None,
                reorder_quantity: None,
                product_id: Some(id),
                warehouse_id: None,
            },
        )
        .await?;

        sqlx::query("UPDATE products SET inventory_item_id = $1 WHERE id = $2 AND entity_id = $3")
            .bind(item_id)
            .bind(id)
            .bind(entity_id)
            .execute(engine.pool())
            .await?;

        if opening_qty > rust_decimal::Decimal::ZERO {
            post_opening_stock(engine, entity_id, item_id, opening_qty, opening_cost, created_by).await?;
        }
    }

    Ok(id)
}

/// Create and link a stock item for a product that is (now) inventory-tracked
/// but has no `inventory_item_id` yet — the missing half of enabling
/// `track_inventory` after creation. No opening stock here; existing products
/// take stock through receive/adjust so quantities stay auditable.
pub async fn link_inventory_item(
    engine: &ErpEngine,
    entity_id: Uuid,
    product_id: Uuid,
    sku: &str,
) -> ErpResult<Uuid> {
    let (name, uom) = sqlx::query_as::<_, (String, String)>(
        "SELECT name, uom FROM products WHERE id = $1 AND entity_id = $2",
    )
    .bind(product_id)
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?;

    let item_id = crate::services::inventory::create_item(
        engine,
        entity_id,
        crate::inventory::CreateInventoryItemRequest {
            sku: sku.to_string(),
            description: name,
            uom: Some(uom),
            costing_method: None,
            gl_inventory: None,
            gl_cogs: None,
            reorder_point: None,
            reorder_quantity: None,
            product_id: Some(product_id),
            warehouse_id: None,
        },
    )
    .await?;

    sqlx::query("UPDATE products SET inventory_item_id = $1 WHERE id = $2 AND entity_id = $3")
        .bind(item_id)
        .bind(product_id)
        .bind(entity_id)
        .execute(engine.pool())
        .await?;

    Ok(item_id)
}

/// Book opening stock for a newly created item: stock quantities, a movement
/// row, and an opening-balance JE (DR inventory / CR opening-balance equity —
/// the seeded 9300, same account onboarding opening balances land in).
async fn post_opening_stock(
    engine: &ErpEngine,
    entity_id: Uuid,
    item_id: Uuid,
    quantity: rust_decimal::Decimal,
    unit_cost: rust_decimal::Decimal,
    actor: &AgentOrUserId,
) -> ErpResult<()> {
    use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
    use rust_decimal::Decimal;

    let value = (quantity * unit_cost).round_dp(2);
    let today = Utc::now().date_naive();

    let mut tx = engine.pool().begin().await?;

    let (sku, gl_inventory) = sqlx::query_as::<_, (String, String)>(
        "SELECT sku, gl_inventory FROM inventory_items WHERE id = $1 AND entity_id = $2",
    )
    .bind(item_id)
    .bind(entity_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE inventory_items SET on_hand = $1, available = $1, unit_cost = $2, total_value = $3 WHERE id = $4 AND entity_id = $5",
    )
    .bind(quantity)
    .bind(unit_cost)
    .bind(value)
    .bind(item_id)
    .bind(entity_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"INSERT INTO stock_movements (id, entity_id, item_id, movement_type, date, quantity, unit_cost, total_cost, notes, created_by, created_at)
           VALUES ($1, $2, $3, 'adjustment', $4, $5, $6, $7, 'Opening stock', $8, $9)"#,
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .bind(item_id)
    .bind(today)
    .bind(quantity)
    .bind(unit_cost)
    .bind(value)
    .bind(serde_json::to_value(actor).unwrap_or_default())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    if value >= Decimal::new(1, 2) {
        let base_ccy = engine.config_for(entity_id).await?.base_currency.clone();
        let entry_req = CreateJournalEntryRequest {
            date: today,
            source: JournalSource::OpeningBalance,
            source_id: Some(item_id),
            reference: format!("OPENSTOCK-{sku}"),
            description: format!("Opening stock {sku}: {quantity} @ {unit_cost}"),
            lines: vec![
                CreateJournalLineRequest {
                    account_code: gl_inventory,
                    debit: Some(value),
                    credit: None,
                    currency: base_ccy.clone(),
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!("Opening stock {sku}")),
                    dimensions: None,
                },
                CreateJournalLineRequest {
                    // Opening Balance Equity — seeded by the COA template.
                    account_code: "9300".to_string(),
                    debit: None,
                    credit: Some(value),
                    currency: base_ccy,
                    fx_rate: Some(Decimal::ONE),
                    description: Some(format!("Opening stock {sku}")),
                    dimensions: None,
                },
            ],
            post_immediately: true,
        };
        let period = crate::services::periods::period_for_date(engine, entity_id, today).await?;
        crate::services::journal::create_and_post_in_tx(&mut tx, engine, entity_id, entry_req, period.id, actor.clone())
            .await?;
    }

    tx.commit().await?;
    Ok(())
}
