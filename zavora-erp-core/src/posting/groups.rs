//! Posting-group resolution (BC/NetSuite-style matrices).
//!
//! Two matrices derive GL accounts from master-data groups instead of forcing a
//! hardcoded account on every line:
//!   * **General**: (business group × product group) → sales / purchase / COGS.
//!   * **VAT**: (VAT business group × VAT product group) → rate + output/input.
//!
//! Resolution is always a *fallback chain*: an explicit per-line account still
//! wins (override), then the matrix, then the flat [`PostingSetup`] defaults — so
//! turning groups on never breaks an existing tenant.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;

/// General posting accounts for a (business, product) combination.
#[derive(Debug, Clone)]
pub struct GeneralPosting {
    pub sales_account: String,
    pub purchase_account: String,
    pub cogs_account: String,
}

/// VAT posting (rate + accounts) for a (business, product) combination.
#[derive(Debug, Clone)]
pub struct VatPosting {
    pub vat_rate: Decimal,
    pub vat_output_account: String,
    pub vat_input_account: String,
}

/// Look up the general posting matrix for a (biz, prod) group pair. Returns
/// `None` when either group is unset or the combination isn't configured (the
/// caller then falls back to the product account / `PostingSetup`).
pub async fn resolve_general(
    engine: &ErpEngine,
    entity_id: Uuid,
    biz_group: Option<Uuid>,
    prod_group: Option<Uuid>,
) -> ErpResult<Option<GeneralPosting>> {
    let (Some(b), Some(p)) = (biz_group, prod_group) else { return Ok(None) };
    let row = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
        "SELECT sales_account, purchase_account, cogs_account
         FROM general_posting_matrix
         WHERE entity_id = $1 AND gen_biz_group_id = $2 AND gen_prod_group_id = $3",
    )
    .bind(entity_id)
    .bind(b)
    .bind(p)
    .fetch_optional(engine.pool())
    .await?;
    Ok(row.map(|(s, pu, c)| GeneralPosting {
        sales_account: s.unwrap_or_default(),
        purchase_account: pu.unwrap_or_default(),
        cogs_account: c.unwrap_or_default(),
    }))
}

/// Look up the VAT posting matrix for a (biz, prod) group pair.
pub async fn resolve_vat(
    engine: &ErpEngine,
    entity_id: Uuid,
    biz_group: Option<Uuid>,
    prod_group: Option<Uuid>,
) -> ErpResult<Option<VatPosting>> {
    let (Some(b), Some(p)) = (biz_group, prod_group) else { return Ok(None) };
    let row = sqlx::query_as::<_, (Decimal, Option<String>, Option<String>)>(
        "SELECT vat_rate, vat_output_account, vat_input_account
         FROM vat_posting_matrix
         WHERE entity_id = $1 AND vat_biz_group_id = $2 AND vat_prod_group_id = $3",
    )
    .bind(entity_id)
    .bind(b)
    .bind(p)
    .fetch_optional(engine.pool())
    .await?;
    Ok(row.map(|(rate, out, inp)| VatPosting {
        vat_rate: rate,
        vat_output_account: out.unwrap_or_default(),
        vat_input_account: inp.unwrap_or_default(),
    }))
}

/// The general business group assigned to a customer/vendor (if any).
pub async fn customer_general_biz(engine: &ErpEngine, entity_id: Uuid, customer_id: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT general_business_group_id FROM customers WHERE id = $1 AND entity_id = $2")
        .bind(customer_id).bind(entity_id).fetch_optional(engine.pool()).await.ok().flatten()
}
pub async fn vendor_general_biz(engine: &ErpEngine, entity_id: Uuid, vendor_id: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT general_business_group_id FROM vendors WHERE id = $1 AND entity_id = $2")
        .bind(vendor_id).bind(entity_id).fetch_optional(engine.pool()).await.ok().flatten()
}
/// The general product group assigned to a product (if any).
pub async fn product_general_group(engine: &ErpEngine, entity_id: Uuid, product_id: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT general_product_group_id FROM products WHERE id = $1 AND entity_id = $2")
        .bind(product_id).bind(entity_id).fetch_optional(engine.pool()).await.ok().flatten()
}

/// Idempotently seed a tenant's default posting groups + matrices from its flat
/// `PostingSetup`, and assign sane defaults to any unassigned masters. A no-op
/// once groups exist, so it is safe to call on every startup and after signup.
pub async fn ensure_default_posting_groups(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<()> {
    let already: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM general_business_groups WHERE entity_id = $1",
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(0);

    if already == 0 {
        seed_groups(engine, entity_id).await?;
    }
    // Always assign defaults to any master missing a group (covers masters
    // created after the initial seed). Idempotent: only touches NULL rows.
    assign_default_groups(engine, entity_id).await?;
    Ok(())
}

async fn group_id(engine: &ErpEngine, table: &str, entity_id: Uuid, code: &str) -> Option<Uuid> {
    sqlx::query_scalar(&format!("SELECT id FROM {table} WHERE entity_id=$1 AND code=$2"))
        .bind(entity_id).bind(code).fetch_optional(engine.pool()).await.ok().flatten()
}

/// Assign the default groups (by code) to any customer/vendor/product that has none.
async fn assign_default_groups(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<()> {
    let (Some(gen_biz), Some(vat_biz), Some(gen_goods), Some(gen_services), Some(vat_std)) = (
        group_id(engine, "general_business_groups", entity_id, "DOMESTIC").await,
        group_id(engine, "vat_business_groups", entity_id, "STD").await,
        group_id(engine, "general_product_groups", entity_id, "GOODS").await,
        group_id(engine, "general_product_groups", entity_id, "SERVICES").await,
        group_id(engine, "vat_product_groups", entity_id, "STD16").await,
    ) else { return Ok(()) };

    sqlx::query("UPDATE customers SET general_business_group_id=$1, vat_business_group_id=$2 WHERE entity_id=$3 AND general_business_group_id IS NULL")
        .bind(gen_biz).bind(vat_biz).bind(entity_id).execute(engine.pool()).await?;
    sqlx::query("UPDATE vendors SET general_business_group_id=$1, vat_business_group_id=$2 WHERE entity_id=$3 AND general_business_group_id IS NULL")
        .bind(gen_biz).bind(vat_biz).bind(entity_id).execute(engine.pool()).await?;
    sqlx::query("UPDATE products SET general_product_group_id=$1, vat_product_group_id=$2 WHERE entity_id=$3 AND general_product_group_id IS NULL AND product_type <> 'Service'")
        .bind(gen_goods).bind(vat_std).bind(entity_id).execute(engine.pool()).await?;
    sqlx::query("UPDATE products SET general_product_group_id=$1, vat_product_group_id=$2 WHERE entity_id=$3 AND general_product_group_id IS NULL AND product_type = 'Service'")
        .bind(gen_services).bind(vat_std).bind(entity_id).execute(engine.pool()).await?;
    Ok(())
}

async fn seed_groups(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<()> {
    let ps = engine.posting_for(entity_id).await?;

    // --- General groups ---
    let gen_biz = Uuid::new_v4();
    let gen_goods = Uuid::new_v4();
    let gen_services = Uuid::new_v4();
    sqlx::query("INSERT INTO general_business_groups (id, entity_id, code, name) VALUES ($1,$2,'DOMESTIC','Domestic')")
        .bind(gen_biz).bind(entity_id).execute(engine.pool()).await?;
    sqlx::query("INSERT INTO general_product_groups (id, entity_id, code, name) VALUES ($1,$2,'GOODS','Goods'),($3,$2,'SERVICES','Services')")
        .bind(gen_goods).bind(entity_id).bind(gen_services).execute(engine.pool()).await?;
    for prod in [gen_goods, gen_services] {
        sqlx::query(
            "INSERT INTO general_posting_matrix (id, entity_id, gen_biz_group_id, gen_prod_group_id, sales_account, purchase_account, cogs_account)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(Uuid::new_v4()).bind(entity_id).bind(gen_biz).bind(prod)
        .bind(&ps.default_sales).bind(&ps.default_purchase).bind(&ps.default_purchase)
        .execute(engine.pool()).await?;
    }

    // --- VAT groups ---
    let vat_biz = Uuid::new_v4();
    let vat_std = Uuid::new_v4();
    let vat_zero = Uuid::new_v4();
    let vat_exempt = Uuid::new_v4();
    sqlx::query("INSERT INTO vat_business_groups (id, entity_id, code, name) VALUES ($1,$2,'STD','Standard (VAT registered)')")
        .bind(vat_biz).bind(entity_id).execute(engine.pool()).await?;
    sqlx::query(
        "INSERT INTO vat_product_groups (id, entity_id, code, name) VALUES ($1,$2,'STD16','Standard 16%'),($3,$2,'ZERO','Zero-rated'),($4,$2,'EXEMPT','Exempt')",
    )
    .bind(vat_std).bind(entity_id).bind(vat_zero).bind(vat_exempt).execute(engine.pool()).await?;
    for (prod, rate) in [(vat_std, dec!(0.16)), (vat_zero, dec!(0)), (vat_exempt, dec!(0))] {
        sqlx::query(
            "INSERT INTO vat_posting_matrix (id, entity_id, vat_biz_group_id, vat_prod_group_id, vat_rate, vat_output_account, vat_input_account)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(Uuid::new_v4()).bind(entity_id).bind(vat_biz).bind(prod).bind(rate)
        .bind(&ps.vat_output).bind(&ps.vat_input)
        .execute(engine.pool()).await?;
    }

    // --- Assign defaults to existing masters that have none ---
    sqlx::query("UPDATE customers SET general_business_group_id = $1, vat_business_group_id = $2 WHERE entity_id = $3 AND general_business_group_id IS NULL")
        .bind(gen_biz).bind(vat_biz).bind(entity_id).execute(engine.pool()).await?;
    sqlx::query("UPDATE vendors SET general_business_group_id = $1, vat_business_group_id = $2 WHERE entity_id = $3 AND general_business_group_id IS NULL")
        .bind(gen_biz).bind(vat_biz).bind(entity_id).execute(engine.pool()).await?;
    // Goods vs Services by product_type; default to Goods.
    sqlx::query("UPDATE products SET general_product_group_id = $1, vat_product_group_id = $2 WHERE entity_id = $3 AND general_product_group_id IS NULL AND product_type <> 'Service'")
        .bind(gen_goods).bind(vat_std).bind(entity_id).execute(engine.pool()).await?;
    sqlx::query("UPDATE products SET general_product_group_id = $1, vat_product_group_id = $2 WHERE entity_id = $3 AND general_product_group_id IS NULL AND product_type = 'Service'")
        .bind(gen_services).bind(vat_std).bind(entity_id).execute(engine.pool()).await?;

    Ok(())
}
