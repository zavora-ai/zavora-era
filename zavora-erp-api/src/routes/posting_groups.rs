use axum::{extract::State, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;
use axum::response::IntoResponse;

/// GET /posting-groups — the full posting-group configuration for the tenant:
/// VAT & General business/product groups and both posting matrices.
pub async fn get_all(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let pool = state.engine.pool();
    let e = ctx.entity_id;
    // Make sure defaults exist so the editor is never empty.
    let _ = zavora_erp_core::posting::groups::ensure_default_posting_groups(&state.engine, e).await;

    async fn rows(pool: &sqlx::PgPool, sql: &str, e: Uuid) -> Vec<serde_json::Value> {
        sqlx::query_as::<_, (serde_json::Value,)>(sql)
            .bind(e).fetch_all(pool).await.unwrap_or_default()
            .into_iter().map(|(j,)| j).collect()
    }
    let vat_business = rows(pool, "SELECT to_jsonb(t) FROM (SELECT id, code, name FROM vat_business_groups WHERE entity_id=$1 ORDER BY code) t", e).await;
    let vat_product = rows(pool, "SELECT to_jsonb(t) FROM (SELECT id, code, name FROM vat_product_groups WHERE entity_id=$1 ORDER BY code) t", e).await;
    let vat_matrix = rows(pool, "SELECT to_jsonb(t) FROM (SELECT id, vat_biz_group_id, vat_prod_group_id, vat_rate, vat_output_account, vat_input_account FROM vat_posting_matrix WHERE entity_id=$1) t", e).await;
    let general_business = rows(pool, "SELECT to_jsonb(t) FROM (SELECT id, code, name, receivables_account, payables_account FROM general_business_groups WHERE entity_id=$1 ORDER BY code) t", e).await;
    let general_product = rows(pool, "SELECT to_jsonb(t) FROM (SELECT id, code, name FROM general_product_groups WHERE entity_id=$1 ORDER BY code) t", e).await;
    let general_matrix = rows(pool, "SELECT to_jsonb(t) FROM (SELECT id, gen_biz_group_id, gen_prod_group_id, sales_account, purchase_account, cogs_account FROM general_posting_matrix WHERE entity_id=$1) t", e).await;

    Ok(Json(serde_json::json!({
        "vat_business": vat_business, "vat_product": vat_product, "vat_matrix": vat_matrix,
        "general_business": general_business, "general_product": general_product, "general_matrix": general_matrix,
    })))
}

#[derive(serde::Deserialize)]
pub struct GroupReq { pub kind: String, pub code: String, pub name: String }

/// POST /posting-groups/group — create a group. `kind` is one of
/// vat_business | vat_product | general_business | general_product.
pub async fn create_group(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<GroupReq>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let table = match req.kind.as_str() {
        "vat_business" => "vat_business_groups",
        "vat_product" => "vat_product_groups",
        "general_business" => "general_business_groups",
        "general_product" => "general_product_groups",
        _ => return Err(err_response(zavora_erp_core::ErpError::ValidationFailed { message: "invalid group kind".into() }).into_response()),
    };
    let id = Uuid::new_v4();
    let sql = format!("INSERT INTO {table} (id, entity_id, code, name) VALUES ($1,$2,$3,$4)");
    sqlx::query(&sql).bind(id).bind(ctx.entity_id).bind(&req.code).bind(&req.name)
        .execute(state.engine.pool()).await
        .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)).into_response())?;
    Ok(Json(serde_json::json!({ "id": id })))
}

#[derive(serde::Deserialize)]
pub struct AssignReq {
    /// "customer" | "vendor" | "product".
    pub kind: String,
    pub id: Uuid,
    pub general_group_id: Option<Uuid>,
    pub vat_group_id: Option<Uuid>,
}

/// POST /posting-groups/assign — set a master's posting groups.
pub async fn assign(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(r): Json<AssignReq>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let (table, gen_col, vat_col) = match r.kind.as_str() {
        "customer" => ("customers", "general_business_group_id", "vat_business_group_id"),
        "vendor" => ("vendors", "general_business_group_id", "vat_business_group_id"),
        "product" => ("products", "general_product_group_id", "vat_product_group_id"),
        _ => return Err(err_response(zavora_erp_core::ErpError::ValidationFailed { message: "invalid kind".into() }).into_response()),
    };
    let sql = format!("UPDATE {table} SET {gen_col}=$1, {vat_col}=$2 WHERE id=$3 AND entity_id=$4");
    sqlx::query(&sql).bind(r.general_group_id).bind(r.vat_group_id).bind(r.id).bind(ctx.entity_id)
        .execute(state.engine.pool()).await
        .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)).into_response())?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
pub struct BizControlReq {
    pub gen_biz_group_id: Uuid,
    pub receivables_account: Option<String>,
    pub payables_account: Option<String>,
}

/// POST /posting-groups/business-control — set the A/R and A/P control accounts
/// for a general business posting group (BC "specific posting groups"). Empty
/// string clears the account so posting falls back to the flat setup.
pub async fn upsert_business_control(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(r): Json<BizControlReq>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let norm = |s: Option<String>| s.filter(|v| !v.trim().is_empty());
    sqlx::query(
        "UPDATE general_business_groups SET receivables_account=$1, payables_account=$2 WHERE id=$3 AND entity_id=$4",
    ).bind(norm(r.receivables_account)).bind(norm(r.payables_account)).bind(r.gen_biz_group_id).bind(ctx.entity_id)
        .execute(state.engine.pool()).await
        .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)).into_response())?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
pub struct GeneralCellReq {
    pub gen_biz_group_id: Uuid,
    pub gen_prod_group_id: Uuid,
    pub sales_account: Option<String>,
    pub purchase_account: Option<String>,
    pub cogs_account: Option<String>,
}

/// POST /posting-groups/general-matrix — upsert a general matrix cell.
pub async fn upsert_general(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(r): Json<GeneralCellReq>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    // sales_account / purchase_account are NOT NULL in the table; the editor may
    // send only the field that changed (e.g. just sales for a royalty row), so
    // coerce any missing account to "" — the resolver treats empty as
    // "fall through to the product / flat setup", which is the intended meaning.
    let sales = r.sales_account.clone().unwrap_or_default();
    let purchase = r.purchase_account.clone().unwrap_or_default();
    sqlx::query(
        "DELETE FROM general_posting_matrix WHERE entity_id=$1 AND gen_biz_group_id=$2 AND gen_prod_group_id=$3",
    ).bind(ctx.entity_id).bind(r.gen_biz_group_id).bind(r.gen_prod_group_id)
        .execute(state.engine.pool()).await.ok();
    sqlx::query(
        "INSERT INTO general_posting_matrix (id, entity_id, gen_biz_group_id, gen_prod_group_id, sales_account, purchase_account, cogs_account)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    ).bind(Uuid::new_v4()).bind(ctx.entity_id).bind(r.gen_biz_group_id).bind(r.gen_prod_group_id)
        .bind(&sales).bind(&purchase).bind(&r.cogs_account)
        .execute(state.engine.pool()).await
        .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)).into_response())?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
pub struct VatCellReq {
    pub vat_biz_group_id: Uuid,
    pub vat_prod_group_id: Uuid,
    pub vat_rate: rust_decimal::Decimal,
    pub vat_output_account: Option<String>,
    pub vat_input_account: Option<String>,
}

/// POST /posting-groups/vat-matrix — upsert a VAT matrix cell.
pub async fn upsert_vat(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(r): Json<VatCellReq>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    // vat_output_account / vat_input_account are NOT NULL; coerce missing to ""
    // (resolver treats empty as fall-through), so a partial cell edit still saves.
    let out = r.vat_output_account.clone().unwrap_or_default();
    let inp = r.vat_input_account.clone().unwrap_or_default();
    sqlx::query(
        "DELETE FROM vat_posting_matrix WHERE entity_id=$1 AND vat_biz_group_id=$2 AND vat_prod_group_id=$3",
    ).bind(ctx.entity_id).bind(r.vat_biz_group_id).bind(r.vat_prod_group_id)
        .execute(state.engine.pool()).await.ok();
    sqlx::query(
        "INSERT INTO vat_posting_matrix (id, entity_id, vat_biz_group_id, vat_prod_group_id, vat_rate, vat_output_account, vat_input_account)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    ).bind(Uuid::new_v4()).bind(ctx.entity_id).bind(r.vat_biz_group_id).bind(r.vat_prod_group_id)
        .bind(r.vat_rate).bind(&out).bind(&inp)
        .execute(state.engine.pool()).await
        .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)).into_response())?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
