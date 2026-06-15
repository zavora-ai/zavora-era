use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_CREATE, ROLES_SEND, ROLES_POST_JOURNAL};
use super::err_response;
use zavora_erp_core::invoicing::*;
use zavora_erp_core::services::invoicing as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_one(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let invoice = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(state.engine.entity_id())
    .fetch_optional(state.engine.pool()).await;

    let lines = sqlx::query_as::<_, InvoiceLineRow>(
        "SELECT * FROM invoice_lines WHERE invoice_id = $1",
    )
    .bind(id)
    .fetch_all(state.engine.pool()).await.unwrap_or_default();

    match invoice {
        Ok(Some(inv)) => Ok(Json(serde_json::json!({
            "invoice": serde_json::to_value(inv).unwrap_or_default(),
            "lines": serde_json::to_value(lines).unwrap_or_default(),
        }))),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Invoice".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateInvoiceRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create invoice").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_invoice(&state.engine, req, &actor).await {
        Ok(invoice) => Ok(Json(serde_json::to_value(invoice).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn post_invoice(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_POST_JOURNAL, &ctx, "post invoice").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::post_invoice(&state.engine, id, &actor).await {
        Ok(je_id) => Ok(Json(serde_json::json!({ "journal_entry_id": je_id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn send(
    ctx: AuthContext,
    State(_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_req): Json<SendInvoiceRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    require_role(ROLES_SEND, &ctx, "send invoice").map_err(|e| {
        let (status, msg) = match &e {
            zavora_erp_core::ErpError::PermissionDenied { .. } => (axum::http::StatusCode::FORBIDDEN, e.to_string()),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, Json(serde_json::json!({ "error": msg })))
    })?;
    Ok(Json(serde_json::json!({ "status": "queued", "invoice_id": id })))
}

pub async fn create_credit_note(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(mut req): Json<zavora_erp_core::invoicing::CreateCreditNoteRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    require_role(ROLES_CREATE, &ctx, "create credit note").map_err(|e| {
        let (status, msg) = match &e {
            zavora_erp_core::ErpError::PermissionDenied { .. } => (axum::http::StatusCode::FORBIDDEN, e.to_string()),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, Json(serde_json::json!({ "error": msg })))
    })?;

    // Ensure the request's invoice_id matches the path parameter
    req.invoice_id = id;

    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_credit_note(&state.engine, req, &actor).await {
        Ok(result) => {
            // Record audit event linking credit note to original invoice
            let audit_event = serde_json::json!({
                "event_type": "credit_note_created",
                "object_type": "invoice",
                "object_id": result.credit_note_id,
                "actor": actor,
                "metadata": {
                    "original_invoice_id": id,
                    "credit_note_number": result.credit_note_number,
                    "amount": result.amount.to_string(),
                    "journal_entry_id": result.journal_entry_id,
                    "original_new_balance": result.original_new_balance.to_string(),
                },
                "timestamp": chrono::Utc::now(),
            });

            let stream_key = format!("erp:audit:{}", state.engine.entity_id());
            let mut redis_conn = state.engine.redis_conn().await;
            let _: Result<(), _> = redis::cmd("XADD")
                .arg(&stream_key)
                .arg("*")
                .arg("data")
                .arg(audit_event.to_string())
                .query_async(&mut redis_conn)
                .await;

            Ok(Json(serde_json::json!({
                "credit_note_id": result.credit_note_id,
                "credit_note_number": result.credit_note_number,
                "amount": result.amount,
                "journal_entry_id": result.journal_entry_id,
                "original_new_balance": result.original_new_balance,
            })))
        }
        Err(e) => {
            let (status, msg) = match &e {
                zavora_erp_core::ErpError::ValidationFailed { .. } => (axum::http::StatusCode::BAD_REQUEST, e.to_string()),
                zavora_erp_core::ErpError::NotFound { .. } => (axum::http::StatusCode::NOT_FOUND, e.to_string()),
                zavora_erp_core::ErpError::PeriodClosed { .. } => (axum::http::StatusCode::CONFLICT, e.to_string()),
                _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            };
            Err((status, Json(serde_json::json!({ "error": msg }))))
        }
    }
}

pub async fn list_recurring(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, RecurringInvoiceRow>(
        "SELECT * FROM recurring_invoices WHERE entity_id = $1 ORDER BY next_run",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_recurring(
    ctx: AuthContext,
    State(_state): State<Arc<AppState>>,
    Json(req): Json<CreateRecurringInvoiceRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    require_role(ROLES_CREATE, &ctx, "create recurring invoice").map_err(|e| {
        let (status, msg) = match &e {
            zavora_erp_core::ErpError::PermissionDenied { .. } => (axum::http::StatusCode::FORBIDDEN, e.to_string()),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, Json(serde_json::json!({ "error": msg })))
    })?;
    Ok(Json(serde_json::json!({ "status": "created", "customer_id": req.customer_id })))
}
