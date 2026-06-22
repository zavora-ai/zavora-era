use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_CREATE};
use zavora_erp_core::parties::*;
use zavora_erp_core::services::parties as svc;
use zavora_erp_core::AgentOrUserId;

// === Customers ===

pub async fn list_customers(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, CustomerRow>(
        "SELECT * FROM customers WHERE entity_id = $1 AND is_active = true ORDER BY name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_customer(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, CustomerRow>(
        "SELECT * FROM customers WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(ctx.entity_id)
    .fetch_optional(state.engine.pool())
    .await;
    match row {
        Ok(Some(r)) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Customer".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_customer(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCustomerRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create customer").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_customer(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update_customer(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(patch): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    if let Err(e) = require_role(ROLES_CREATE, &ctx, "update customer") { return Err(err_response(e)); }
    // Update all provided fields
    if let Some(name) = patch.get("name").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE customers SET name = $1 WHERE id = $2 AND entity_id = $3")
            .bind(name).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(kra_pin) = patch.get("kra_pin").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE customers SET kra_pin = $1 WHERE id = $2 AND entity_id = $3")
            .bind(kra_pin).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(vat_number) = patch.get("vat_number").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE customers SET vat_number = $1 WHERE id = $2 AND entity_id = $3")
            .bind(vat_number).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(currency) = patch.get("currency").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE customers SET currency = $1 WHERE id = $2 AND entity_id = $3")
            .bind(currency).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(payment_terms) = patch.get("payment_terms").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE customers SET payment_terms = $1 WHERE id = $2 AND entity_id = $3")
            .bind(payment_terms).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(email) = patch.get("email") {
        let email_json = serde_json::to_string(email).unwrap_or_default();
        sqlx::query("UPDATE customers SET email = $1::jsonb WHERE id = $2 AND entity_id = $3")
            .bind(&email_json).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(phone) = patch.get("phone") {
        let phone_json = serde_json::to_string(phone).unwrap_or_default();
        sqlx::query("UPDATE customers SET phone = $1::jsonb WHERE id = $2 AND entity_id = $3")
            .bind(&phone_json).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(address) = patch.get("address") {
        let address_json = serde_json::to_string(address).unwrap_or_default();
        sqlx::query("UPDATE customers SET address = $1::jsonb WHERE id = $2 AND entity_id = $3")
            .bind(&address_json).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(credit_limit) = patch.get("credit_limit").and_then(|v| v.as_f64()) {
        sqlx::query("UPDATE customers SET credit_limit = $1 WHERE id = $2 AND entity_id = $3")
            .bind(rust_decimal::Decimal::from_f64_retain(credit_limit).unwrap_or_default())
            .bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(ar_account) = patch.get("ar_account").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE customers SET ar_account = $1 WHERE id = $2 AND entity_id = $3")
            .bind(ar_account).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(reminder_policy) = patch.get("reminder_policy").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE customers SET reminder_policy = $1 WHERE id = $2 AND entity_id = $3")
            .bind(reminder_policy).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(portal_enabled) = patch.get("portal_enabled").and_then(|v| v.as_bool()) {
        sqlx::query("UPDATE customers SET portal_enabled = $1 WHERE id = $2 AND entity_id = $3")
            .bind(portal_enabled).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(notes) = patch.get("notes").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE customers SET notes = $1 WHERE id = $2 AND entity_id = $3")
            .bind(notes).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(is_active) = patch.get("is_active").and_then(|v| v.as_bool()) {
        sqlx::query("UPDATE customers SET is_active = $1 WHERE id = $2 AND entity_id = $3")
            .bind(is_active).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    Ok(Json(serde_json::json!({ "id": id, "updated": true })))
}

pub async fn customer_statement(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Json<serde_json::Value> {
    let invoices = sqlx::query_as::<_, zavora_erp_core::invoicing::InvoiceRow>(
        "SELECT * FROM invoices WHERE customer_id = $1 AND entity_id = $2 ORDER BY issue_date",
    )
    .bind(id).bind(ctx.entity_id)
    .fetch_all(state.engine.pool()).await.unwrap_or_default();

    Json(serde_json::json!({
        "customer_id": id,
        "invoices": serde_json::to_value(&invoices).unwrap_or_default(),
        "total_invoiced": invoices.iter().map(|i| i.gross_total).sum::<rust_decimal::Decimal>(),
        "total_paid": invoices.iter().map(|i| i.amount_paid).sum::<rust_decimal::Decimal>(),
        "balance_due": invoices.iter().map(|i| i.balance_due).sum::<rust_decimal::Decimal>(),
    }))
}

// === Vendors ===

pub async fn list_vendors(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, VendorRow>(
        "SELECT * FROM vendors WHERE entity_id = $1 AND is_active = true ORDER BY name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// GET /vendors/{id} — vendor record enriched with AP summary statistics.
pub async fn get_vendor(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, VendorRow>(
        "SELECT * FROM vendors WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await;

    let vendor = match row {
        Ok(Some(r)) => r,
        Ok(None) => return Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Vendor".into(), id })),
        Err(e) => return Err(err_response(zavora_erp_core::ErpError::Database(e))),
    };

    // Aggregate AP activity. payment_type is stored JSON-encoded (e.g. "vendor_payment"),
    // so strip the quotes before comparing. Draft/void documents are excluded from money totals.
    use rust_decimal::Decimal;
    let agg = sqlx::query_as::<_, (Decimal, i64, Decimal, Decimal, i64, Decimal, i64)>(
        r#"SELECT
            (SELECT COALESCE(SUM(gross_total),0) FROM bills WHERE vendor_id=$1 AND entity_id=$2 AND status NOT IN ('draft','cancelled','void')) AS total_billed,
            (SELECT COUNT(*) FROM bills WHERE vendor_id=$1 AND entity_id=$2) AS bill_count,
            (SELECT COALESCE(SUM(balance_due),0) FROM bills WHERE vendor_id=$1 AND entity_id=$2 AND status NOT IN ('draft','cancelled','void')) AS outstanding_bills,
            (SELECT COALESCE(SUM(amount),0) FROM payments WHERE party_id=$1 AND entity_id=$2 AND trim(both '"' from payment_type)='vendor_payment' AND status NOT IN ('cancelled','voided')) AS total_paid,
            (SELECT COUNT(*) FROM payments WHERE party_id=$1 AND entity_id=$2 AND trim(both '"' from payment_type)='vendor_payment') AS payment_count,
            (SELECT COALESCE(SUM(gross_total),0) FROM supplier_credit_notes WHERE vendor_id=$1 AND entity_id=$2 AND status NOT IN ('draft','cancelled','void')) AS total_credit_notes,
            (SELECT COUNT(*) FROM supplier_credit_notes WHERE vendor_id=$1 AND entity_id=$2) AS credit_note_count
        "#,
    )
    .bind(id)
    .bind(ctx.entity_id)
    .fetch_one(state.engine.pool())
    .await;

    let (total_billed, bill_count, outstanding_bills, total_paid, payment_count, total_credit_notes, credit_note_count) =
        match agg {
            Ok(t) => t,
            Err(e) => return Err(err_response(zavora_erp_core::ErpError::Database(e))),
        };

    let outstanding_balance = outstanding_bills - total_credit_notes;

    let mut out = serde_json::to_value(&vendor).unwrap_or_default();
    if let Some(obj) = out.as_object_mut() {
        obj.insert("total_billed".into(), serde_json::json!(total_billed));
        obj.insert("total_paid".into(), serde_json::json!(total_paid));
        obj.insert("total_credit_notes".into(), serde_json::json!(total_credit_notes));
        obj.insert("outstanding_balance".into(), serde_json::json!(outstanding_balance));
        obj.insert("bill_count".into(), serde_json::json!(bill_count));
        obj.insert("payment_count".into(), serde_json::json!(payment_count));
        obj.insert("credit_note_count".into(), serde_json::json!(credit_note_count));
    }
    Ok(Json(out))
}

pub async fn create_vendor(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateVendorRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create vendor").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_vendor(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update_vendor(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(patch): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    if let Err(e) = require_role(ROLES_CREATE, &ctx, "update vendor") { return Err(err_response(e)); }
    if let Some(name) = patch.get("name").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE vendors SET name = $1 WHERE id = $2 AND entity_id = $3")
            .bind(name).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(kra_pin) = patch.get("kra_pin").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE vendors SET kra_pin = $1 WHERE id = $2 AND entity_id = $3")
            .bind(kra_pin).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(vat_number) = patch.get("vat_number").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE vendors SET vat_number = $1 WHERE id = $2 AND entity_id = $3")
            .bind(vat_number).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(currency) = patch.get("currency").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE vendors SET currency = $1 WHERE id = $2 AND entity_id = $3")
            .bind(currency).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(payment_terms) = patch.get("payment_terms").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE vendors SET payment_terms = $1 WHERE id = $2 AND entity_id = $3")
            .bind(payment_terms).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(wht_category) = patch.get("wht_category").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE vendors SET wht_category = $1 WHERE id = $2 AND entity_id = $3")
            .bind(wht_category).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(resident) = patch.get("resident").and_then(|v| v.as_bool()) {
        sqlx::query("UPDATE vendors SET resident = $1 WHERE id = $2 AND entity_id = $3")
            .bind(resident).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(email) = patch.get("email") {
        let email_json = serde_json::to_string(email).unwrap_or_default();
        sqlx::query("UPDATE vendors SET email = $1::jsonb WHERE id = $2 AND entity_id = $3")
            .bind(&email_json).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(phone) = patch.get("phone") {
        let phone_json = serde_json::to_string(phone).unwrap_or_default();
        sqlx::query("UPDATE vendors SET phone = $1::jsonb WHERE id = $2 AND entity_id = $3")
            .bind(&phone_json).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(address) = patch.get("address") {
        let address_json = serde_json::to_string(address).unwrap_or_default();
        sqlx::query("UPDATE vendors SET address = $1::jsonb WHERE id = $2 AND entity_id = $3")
            .bind(&address_json).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(ap_account) = patch.get("ap_account").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE vendors SET ap_account = $1 WHERE id = $2 AND entity_id = $3")
            .bind(ap_account).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(default_expense_account) = patch.get("default_expense_account").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE vendors SET default_expense_account = $1 WHERE id = $2 AND entity_id = $3")
            .bind(default_expense_account).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(bank_details) = patch.get("bank_details") {
        let bank_json = serde_json::to_string(bank_details).unwrap_or_default();
        sqlx::query("UPDATE vendors SET bank_details = $1::jsonb WHERE id = $2 AND entity_id = $3")
            .bind(&bank_json).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(notes) = patch.get("notes").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE vendors SET notes = $1 WHERE id = $2 AND entity_id = $3")
            .bind(notes).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(is_active) = patch.get("is_active").and_then(|v| v.as_bool()) {
        sqlx::query("UPDATE vendors SET is_active = $1 WHERE id = $2 AND entity_id = $3")
            .bind(is_active).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    Ok(Json(serde_json::json!({ "id": id, "updated": true })))
}

// === Employees ===

pub async fn list_employees(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE entity_id = $1 AND is_active = true ORDER BY full_name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_employee(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await;
    match row {
        Ok(Some(r)) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Employee".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_employee(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateEmployeeRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create employee").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_employee(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update_employee(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(patch): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    if let Err(e) = require_role(ROLES_CREATE, &ctx, "update employee") { return Err(err_response(e)); }
    if let Some(full_name) = patch.get("full_name").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE employees SET full_name = $1 WHERE id = $2 AND entity_id = $3")
            .bind(full_name).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(salary) = patch.get("basic_salary").and_then(|v| v.as_f64()) {
        sqlx::query("UPDATE employees SET basic_salary = $1 WHERE id = $2 AND entity_id = $3")
            .bind(rust_decimal::Decimal::from_f64_retain(salary).unwrap_or_default())
            .bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(kra_pin) = patch.get("kra_pin").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE employees SET kra_pin = $1 WHERE id = $2 AND entity_id = $3")
            .bind(kra_pin).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(nssf_number) = patch.get("nssf_number").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE employees SET nssf_number = $1 WHERE id = $2 AND entity_id = $3")
            .bind(nssf_number).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(nhif_number) = patch.get("nhif_number").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE employees SET nhif_number = $1 WHERE id = $2 AND entity_id = $3")
            .bind(nhif_number).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(helb) = patch.get("helb_deduction").and_then(|v| v.as_f64()) {
        sqlx::query("UPDATE employees SET helb_deduction = $1 WHERE id = $2 AND entity_id = $3")
            .bind(rust_decimal::Decimal::from_f64_retain(helb).unwrap_or_default())
            .bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(employment_type) = patch.get("employment_type").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE employees SET employment_type = $1 WHERE id = $2 AND entity_id = $3")
            .bind(employment_type).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(allowances) = patch.get("allowances") {
        let allowances_json = serde_json::to_string(allowances).unwrap_or_default();
        sqlx::query("UPDATE employees SET allowances = $1::jsonb WHERE id = $2 AND entity_id = $3")
            .bind(&allowances_json).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(bank_account) = patch.get("bank_account") {
        let bank_json = serde_json::to_string(bank_account).unwrap_or_default();
        sqlx::query("UPDATE employees SET bank_account = $1::jsonb WHERE id = $2 AND entity_id = $3")
            .bind(&bank_json).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(tax_relief) = patch.get("tax_relief").and_then(|v| v.as_f64()) {
        sqlx::query("UPDATE employees SET tax_relief = $1 WHERE id = $2 AND entity_id = $3")
            .bind(rust_decimal::Decimal::from_f64_retain(tax_relief).unwrap_or_default())
            .bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(disability_exemption) = patch.get("disability_exemption").and_then(|v| v.as_bool()) {
        sqlx::query("UPDATE employees SET disability_exemption = $1 WHERE id = $2 AND entity_id = $3")
            .bind(disability_exemption).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    if let Some(start_date) = patch.get("start_date").and_then(|v| v.as_str()) {
        if let Ok(date) = start_date.parse::<chrono::NaiveDate>() {
            sqlx::query("UPDATE employees SET start_date = $1 WHERE id = $2 AND entity_id = $3")
                .bind(date).bind(id).bind(ctx.entity_id)
                .execute(state.engine.pool()).await.ok();
        }
    }
    if let Some(end_date) = patch.get("end_date").and_then(|v| v.as_str()) {
        if let Ok(date) = end_date.parse::<chrono::NaiveDate>() {
            sqlx::query("UPDATE employees SET end_date = $1 WHERE id = $2 AND entity_id = $3")
                .bind(date).bind(id).bind(ctx.entity_id)
                .execute(state.engine.pool()).await.ok();
        }
    }
    if let Some(is_active) = patch.get("is_active").and_then(|v| v.as_bool()) {
        sqlx::query("UPDATE employees SET is_active = $1 WHERE id = $2 AND entity_id = $3")
            .bind(is_active).bind(id).bind(ctx.entity_id)
            .execute(state.engine.pool()).await.ok();
    }
    Ok(Json(serde_json::json!({ "id": id, "updated": true })))
}
