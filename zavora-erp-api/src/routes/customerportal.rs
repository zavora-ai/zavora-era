//! Customer-portal self-service (CRM add-in). Everything is row-scoped to the
//! authenticated `customer_users` principal (`ctx.customer_id`) — a customer only
//! ever sees their own account, invoices, statement and support tickets.

use std::sync::Arc;
use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::customer_auth::CustomerContext;
use zavora_erp_core::crm::{CreateTicketRequest, TicketReplyRequest};
use zavora_erp_core::services::crm as svc;
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;
fn er(e: ErpError) -> axum::response::Response { use axum::response::IntoResponse; err_response(e).into_response() }
fn db(e: sqlx::Error) -> axum::response::Response { er(ErpError::Database(e)) }

/// GET /me/profile — the customer's portal profile + linked account name.
pub async fn profile(ctx: CustomerContext, State(state): State<Arc<AppState>>) -> ApiResult {
    use sqlx::Row;
    let row = sqlx::query(
        "SELECT cu.email, cu.display_name, cu.status, cu.customer_id, c.name AS customer_name \
         FROM customer_users cu LEFT JOIN customers c ON c.id = cu.customer_id \
         WHERE cu.id = $1 AND cu.entity_id = $2",
    )
    .bind(ctx.customer_user_id).bind(ctx.entity_id).fetch_optional(state.engine.pool()).await.map_err(db)?
    .ok_or_else(|| er(ErpError::NotFound { entity_type: "Customer".into(), id: ctx.customer_user_id }))?;
    Ok(Json(serde_json::json!({
        "email": row.get::<String,_>("email"),
        "display_name": row.get::<String,_>("display_name"),
        "status": row.get::<String,_>("status"),
        "customer_id": row.get::<Option<Uuid>,_>("customer_id"),
        "customer_name": row.get::<Option<String>,_>("customer_name"),
        "linked": ctx.customer_id.is_some(),
    })))
}

#[derive(Deserialize)]
pub struct ProfilePatch { pub display_name: Option<String> }

/// PUT /me/profile — update the customer's own display name.
pub async fn update_profile(ctx: CustomerContext, State(state): State<Arc<AppState>>, Json(p): Json<ProfilePatch>) -> ApiResult {
    if let Some(name) = p.display_name {
        sqlx::query("UPDATE customer_users SET display_name = $1 WHERE id = $2 AND entity_id = $3")
            .bind(name).bind(ctx.customer_user_id).bind(ctx.entity_id).execute(state.engine.pool()).await.map_err(db)?;
    }
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

/// GET /me/invoices — the linked account's invoices.
pub async fn invoices(ctx: CustomerContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let Some(customer_id) = ctx.customer_id else {
        return Ok(Json(serde_json::json!({ "linked": false, "invoices": [] })));
    };
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT id, number AS invoice_number, status, issue_date, due_date, gross_total, balance_due, currency \
         FROM invoices WHERE entity_id = $1 AND customer_id = $2 ORDER BY issue_date DESC LIMIT 200",
    )
    .bind(ctx.entity_id).bind(customer_id).fetch_all(state.engine.pool()).await.map_err(db)?;
    let list: Vec<serde_json::Value> = rows.iter().map(|r| serde_json::json!({
        "id": r.get::<Uuid,_>("id"),
        "invoice_number": r.get::<Option<String>,_>("invoice_number"),
        "status": r.get::<String,_>("status"),
        "issue_date": r.get::<Option<chrono::NaiveDate>,_>("issue_date").map(|d| d.to_string()),
        "due_date": r.get::<Option<chrono::NaiveDate>,_>("due_date").map(|d| d.to_string()),
        "gross_total": r.get::<rust_decimal::Decimal,_>("gross_total"),
        "balance_due": r.get::<Option<rust_decimal::Decimal>,_>("balance_due"),
        "currency": r.get::<Option<String>,_>("currency"),
    })).collect();
    Ok(Json(serde_json::json!({ "linked": true, "invoices": list })))
}

/// GET /me/statement — outstanding balance + open invoices summary.
pub async fn statement(ctx: CustomerContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let Some(customer_id) = ctx.customer_id else {
        return Ok(Json(serde_json::json!({ "linked": false, "outstanding": 0, "open_invoices": 0 })));
    };
    let (outstanding, open): (rust_decimal::Decimal, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(balance_due),0), COUNT(*) FROM invoices \
         WHERE entity_id = $1 AND customer_id = $2 AND status NOT IN ('paid','voided')",
    )
    .bind(ctx.entity_id).bind(customer_id).fetch_one(state.engine.pool()).await.map_err(db)?;
    Ok(Json(serde_json::json!({ "linked": true, "outstanding": outstanding, "open_invoices": open })))
}

// ─── Support tickets (own only) ──────────────────────────────────────────────

pub async fn list_tickets(ctx: CustomerContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query_as::<_, zavora_erp_core::crm::TicketRow>(
        "SELECT * FROM crm_tickets WHERE entity_id = $1 AND created_by_customer_user_id = $2 ORDER BY updated_at DESC",
    )
    .bind(ctx.entity_id).bind(ctx.customer_user_id).fetch_all(state.engine.pool()).await.map_err(db)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
pub struct PortalTicketRequest { pub subject: String, #[serde(default)] pub description: Option<String>, #[serde(default = "d_normal")] pub priority: String }
fn d_normal() -> String { "Normal".into() }

pub async fn create_ticket(ctx: CustomerContext, State(state): State<Arc<AppState>>, Json(req): Json<PortalTicketRequest>) -> ApiResult {
    let create = CreateTicketRequest { customer_id: ctx.customer_id, subject: req.subject, description: req.description, priority: req.priority };
    let id = svc::create_ticket(&state.engine, ctx.entity_id, &create, Some(ctx.customer_user_id)).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

/// Verify the ticket was raised by this customer before exposing it.
async fn owns_ticket(state: &Arc<AppState>, ctx: &CustomerContext, ticket_id: Uuid) -> Result<(), axum::response::Response> {
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT created_by_customer_user_id FROM crm_tickets WHERE id = $1 AND entity_id = $2",
    )
    .bind(ticket_id).bind(ctx.entity_id).fetch_optional(state.engine.pool()).await.map_err(db)?.flatten();
    if owner == Some(ctx.customer_user_id) { Ok(()) }
    else { Err(er(ErpError::PermissionDenied { action: "view ticket".into(), required_role: "ticket owner".into() })) }
}

pub async fn get_ticket(ctx: CustomerContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    owns_ticket(&state, &ctx, id).await?;
    let v = svc::get_ticket(&state.engine, ctx.entity_id, id).await.map_err(er)?;
    Ok(Json(v))
}

pub async fn reply_ticket(ctx: CustomerContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<TicketReplyRequest>) -> ApiResult {
    owns_ticket(&state, &ctx, id).await?;
    svc::reply_ticket(&state.engine, ctx.entity_id, id, "customer", Some(ctx.customer_user_id), &req.body).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}
