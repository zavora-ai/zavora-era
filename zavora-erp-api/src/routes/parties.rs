use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use zavora_erp_core::parties::*;
use zavora_erp_core::services::parties as svc;
use zavora_erp_core::AgentOrUserId;

// === Customers ===

pub async fn list_customers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, CustomerRow>(
        "SELECT * FROM customers WHERE entity_id = $1 AND is_active = true ORDER BY name",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_customer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, CustomerRow>(
        "SELECT * FROM customers WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(state.engine.entity_id())
    .fetch_optional(state.engine.pool())
    .await;
    match row {
        Ok(Some(r)) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Customer".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_customer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCustomerRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::create_customer(&state.engine, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update_customer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(patch): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let name = patch.get("name").and_then(|v| v.as_str());
    if let Some(name) = name {
        sqlx::query("UPDATE customers SET name = $1 WHERE id = $2 AND entity_id = $3")
            .bind(name).bind(id).bind(state.engine.entity_id())
            .execute(state.engine.pool()).await.ok();
    }
    Json(serde_json::json!({ "id": id, "updated": true }))
}

pub async fn customer_statement(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Json<serde_json::Value> {
    let invoices = sqlx::query_as::<_, zavora_erp_core::invoicing::InvoiceRow>(
        "SELECT * FROM invoices WHERE customer_id = $1 AND entity_id = $2 ORDER BY issue_date",
    )
    .bind(id).bind(state.engine.entity_id())
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
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, VendorRow>(
        "SELECT * FROM vendors WHERE entity_id = $1 AND is_active = true ORDER BY name",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_vendor(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, VendorRow>(
        "SELECT * FROM vendors WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(state.engine.entity_id())
    .fetch_optional(state.engine.pool()).await;
    match row {
        Ok(Some(r)) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Vendor".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_vendor(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateVendorRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::create_vendor(&state.engine, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update_vendor(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(patch): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let name = patch.get("name").and_then(|v| v.as_str());
    if let Some(name) = name {
        sqlx::query("UPDATE vendors SET name = $1 WHERE id = $2 AND entity_id = $3")
            .bind(name).bind(id).bind(state.engine.entity_id())
            .execute(state.engine.pool()).await.ok();
    }
    Json(serde_json::json!({ "id": id, "updated": true }))
}

// === Employees ===

pub async fn list_employees(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE entity_id = $1 AND is_active = true ORDER BY full_name",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_employee(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(state.engine.entity_id())
    .fetch_optional(state.engine.pool()).await;
    match row {
        Ok(Some(r)) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Employee".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_employee(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateEmployeeRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::create_employee(&state.engine, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update_employee(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(patch): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    if let Some(salary) = patch.get("basic_salary").and_then(|v| v.as_f64()) {
        sqlx::query("UPDATE employees SET basic_salary = $1 WHERE id = $2 AND entity_id = $3")
            .bind(rust_decimal::Decimal::from_f64_retain(salary).unwrap_or_default())
            .bind(id).bind(state.engine.entity_id())
            .execute(state.engine.pool()).await.ok();
    }
    Json(serde_json::json!({ "id": id, "updated": true }))
}
