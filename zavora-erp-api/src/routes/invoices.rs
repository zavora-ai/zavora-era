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
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    axum::extract::Query(page): axum::extract::Query<crate::routes::pagination::PaginationParams>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoices WHERE entity_id = $1")
        .bind(ctx.entity_id).fetch_one(state.engine.pool()).await.unwrap_or(0);
    let rows = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE entity_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(ctx.entity_id).bind(page.effective_limit()).bind(page.effective_offset())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(crate::routes::pagination::PaginatedResponse::new(r, total, &page)).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_one(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let invoice = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id)
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
    match svc::create_invoice(&state.engine, ctx.entity_id, req, &actor).await {
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
    match svc::post_invoice(&state.engine, ctx.entity_id, id, &actor).await {
        Ok(je_id) => Ok(Json(serde_json::json!({ "journal_entry_id": je_id }))),
        Err(e) => Err(err_response(e)),
    }
}

#[derive(serde::Deserialize)]
pub struct WriteOffRequest {
    pub expense_account: String,
    #[serde(default)]
    pub amount: Option<rust_decimal::Decimal>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// POST /invoices/{id}/write-off — write an uncollectable invoice (or part) off
/// to a bad-debt expense account.
pub async fn write_off(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<WriteOffRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_POST_JOURNAL, &ctx, "write off invoice").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::write_off_invoice(&state.engine, ctx.entity_id, id, req.expense_account, req.amount, req.reason, actor).await {
        Ok(je_id) => Ok(Json(serde_json::json!({ "journal_entry_id": je_id }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /invoices/{id}/send — mark a posted invoice as sent (records sent_at).
/// Delivery is decoupled from posting; this stamps that the invoice was sent,
/// including off-system (printed/emailed manually).
pub async fn send(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(mut req): Json<SendInvoiceRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_SEND, &ctx, "send invoice").map_err(err_response)?;
    req.invoice_id = id; // path is authoritative
    match svc::send_invoice(&state.engine, ctx.entity_id, req).await {
        Ok(Some(recipient)) => Ok(Json(serde_json::json!({ "status": "sent", "invoice_id": id, "emailed_to": recipient }))),
        Ok(None) => Ok(Json(serde_json::json!({ "status": "sent", "invoice_id": id, "emailed_to": null }))),
        Err(e) => Err(err_response(e)),
    }
}

/// PUT /invoices/{id} — edit a draft invoice (replaces lines, recomputes totals).
pub async fn update(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateInvoiceRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "edit invoice").map_err(err_response)?;
    match svc::update_invoice_draft(&state.engine, ctx.entity_id, id, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "id": id, "updated": true }))),
        Err(e) => Err(err_response(e)),
    }
}

/// DELETE /invoices/{id} — delete a draft invoice and its lines.
pub async fn delete(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "delete invoice").map_err(err_response)?;
    match svc::delete_invoice_draft(&state.engine, ctx.entity_id, id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "id": id, "deleted": true }))),
        Err(e) => Err(err_response(e)),
    }
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
    match svc::create_credit_note(&state.engine, ctx.entity_id, req, &actor).await {
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

            let stream_key = format!("erp:audit:{}", ctx.entity_id);
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

/// POST /invoices/{id}/etims-transmit — mark a posted invoice as transmitted to KRA eTIMS.
pub async fn etims_transmit(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_SEND, &ctx, "transmit invoice to eTIMS").map_err(err_response)?;
    let etims_number = req.get("etims_invoice_number").and_then(|v| v.as_str()).map(|s| s.to_string());
    match svc::mark_invoice_etims_transmitted(&state.engine, ctx.entity_id, id, etims_number).await {
        Ok(()) => Ok(Json(serde_json::json!({ "id": id, "etims_status": "transmitted" }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn list_recurring(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, RecurringInvoiceRow>(
        "SELECT * FROM recurring_invoices WHERE entity_id = $1 ORDER BY next_run",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_recurring(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRecurringInvoiceRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    require_role(ROLES_CREATE, &ctx, "create recurring invoice").map_err(|e| {
        let (status, msg) = match &e {
            zavora_erp_core::ErpError::PermissionDenied { .. } => (axum::http::StatusCode::FORBIDDEN, e.to_string()),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, Json(serde_json::json!({ "error": msg })))
    })?;

    let bad_request = |msg: String| (axum::http::StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({ "error": msg })));

    // Validate the template carries at least one line so the scheduler can produce a real invoice.
    if req.template.lines.is_empty() {
        return Err(bad_request("Recurring template must have at least one line item.".to_string()));
    }
    if let Some(end) = req.end_date {
        if end < req.start_date {
            return Err(bad_request("End date cannot be before the start date.".to_string()));
        }
    }

    // Validate the customer belongs to this entity before persisting the schedule.
    let customer_ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM customers WHERE id = $1 AND entity_id = $2)",
    )
    .bind(req.customer_id)
    .bind(ctx.entity_id)
    .fetch_one(state.engine.pool())
    .await
    .unwrap_or(false);
    if !customer_ok {
        return Err(bad_request("Customer not found for this organisation.".to_string()));
    }

    // Store frequency as its canonical enum string (e.g. "Monthly") so the scheduler round-trips it.
    let frequency = serde_json::to_value(&req.frequency)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "Monthly".to_string());

    // Persist the template with the customer_id pinned so scheduled runs target the right customer.
    let mut template = req.template.clone();
    template.customer_id = req.customer_id;
    let template_json = serde_json::to_value(&template).map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))
    })?;

    // First run is the start date.
    let next_run = req.start_date;

    let row = sqlx::query_as::<_, RecurringInvoiceRow>(
        "INSERT INTO recurring_invoices
            (entity_id, customer_id, template, frequency, start_date, end_date, next_run, auto_send, auto_charge)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING *",
    )
    .bind(ctx.entity_id)
    .bind(req.customer_id)
    .bind(template_json)
    .bind(&frequency)
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(next_run)
    .bind(req.auto_send.unwrap_or(false))
    .bind(req.auto_charge.unwrap_or(false))
    .fetch_one(state.engine.pool())
    .await
    .map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))
    })?;

    Ok(Json(serde_json::to_value(row).unwrap_or_default()))
}

/// PUT /recurring-invoices/{id} — replace the editable fields of a schedule.
pub async fn update_recurring(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateRecurringInvoiceRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    require_role(ROLES_CREATE, &ctx, "update recurring invoice").map_err(|e| {
        let (status, msg) = match &e {
            zavora_erp_core::ErpError::PermissionDenied { .. } => (axum::http::StatusCode::FORBIDDEN, e.to_string()),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, Json(serde_json::json!({ "error": msg })))
    })?;

    let unprocessable = |msg: String| (axum::http::StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({ "error": msg })));

    if req.template.lines.is_empty() {
        return Err(unprocessable("Recurring template must have at least one line item.".to_string()));
    }
    if let Some(end) = req.end_date {
        if end < req.start_date {
            return Err(unprocessable("End date cannot be before the start date.".to_string()));
        }
    }

    let frequency = serde_json::to_value(&req.frequency)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "Monthly".to_string());

    let mut template = req.template.clone();
    template.customer_id = req.customer_id;
    let template_json = serde_json::to_value(&template).map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))
    })?;

    // Preserve the schedule's place in the run cycle: only reset next_run if it has not yet run.
    let row = sqlx::query_as::<_, RecurringInvoiceRow>(
        "UPDATE recurring_invoices SET
            customer_id = $1, template = $2, frequency = $3, start_date = $4, end_date = $5,
            auto_send = $6, auto_charge = $7,
            next_run = CASE WHEN run_count = 0 THEN $4 ELSE next_run END
         WHERE id = $8 AND entity_id = $9
         RETURNING *",
    )
    .bind(req.customer_id)
    .bind(template_json)
    .bind(&frequency)
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(req.auto_send.unwrap_or(false))
    .bind(req.auto_charge.unwrap_or(false))
    .bind(id)
    .bind(ctx.entity_id)
    .fetch_optional(state.engine.pool())
    .await
    .map_err(|e| {
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))
    })?;

    match row {
        Some(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        None => Err((axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Recurring invoice not found." })))),
    }
}

/// DELETE /recurring-invoices/{id} — remove a schedule.
pub async fn delete_recurring(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    require_role(ROLES_CREATE, &ctx, "delete recurring invoice").map_err(|e| {
        let (status, msg) = match &e {
            zavora_erp_core::ErpError::PermissionDenied { .. } => (axum::http::StatusCode::FORBIDDEN, e.to_string()),
            _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        (status, Json(serde_json::json!({ "error": msg })))
    })?;

    let result = sqlx::query("DELETE FROM recurring_invoices WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(ctx.entity_id)
        .execute(state.engine.pool())
        .await
        .map_err(|e| {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))
        })?;

    if result.rows_affected() == 0 {
        return Err((axum::http::StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Recurring invoice not found." }))));
    }
    Ok(Json(serde_json::json!({ "status": "deleted", "id": id })))
}

#[derive(serde::Deserialize)]
pub struct DocumentQuery {
    /// "html" (default) for the on-screen/iframe document, or "pdf" for download.
    #[serde(default)]
    pub format: Option<String>,
}

/// GET /invoices/{id}/document?format=html|pdf
///
/// The single source of truth for the invoice document. `html` returns the same
/// markup shown on screen; `pdf` returns that exact HTML printed to PDF (headless
/// Chrome, with a hand-built fallback). The emailed attachment uses the same
/// renderer, so screen == download == email.
pub async fn document(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<DocumentQuery>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let want_pdf = q.format.as_deref() == Some("pdf");

    if want_pdf {
        match svc::invoice_document_pdf(&state.engine, ctx.entity_id, id).await {
            Ok((bytes, number)) => {
                // Name the file by the invoice number (e.g. INV-2026-0004.pdf),
                // keeping only filename-safe characters.
                let safe: String = number
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
                    .collect();
                let filename = if safe.is_empty() { format!("invoice-{id}") } else { safe };
                (
                    [
                        (axum::http::header::CONTENT_TYPE, "application/pdf".to_string()),
                        (
                            axum::http::header::CONTENT_DISPOSITION,
                            format!("inline; filename=\"{filename}.pdf\""),
                        ),
                    ],
                    bytes,
                )
                    .into_response()
            }
            Err(e) => err_response(e).into_response(),
        }
    } else {
        match svc::invoice_document_html(&state.engine, ctx.entity_id, id).await {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(e) => err_response(e).into_response(),
        }
    }
}

/// GET /recurring-invoices/{id}/document?format=html|pdf — preview of the next
/// invoice the schedule will generate (same renderer as invoices).
pub async fn recurring_document(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<DocumentQuery>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if q.format.as_deref() == Some("pdf") {
        match svc::recurring_document_pdf(&state.engine, ctx.entity_id, id).await {
            Ok((bytes, _)) => (
                [
                    (axum::http::header::CONTENT_TYPE, "application/pdf".to_string()),
                    (axum::http::header::CONTENT_DISPOSITION, "inline; filename=\"recurring-preview.pdf\"".to_string()),
                ],
                bytes,
            )
                .into_response(),
            Err(e) => err_response(e).into_response(),
        }
    } else {
        match svc::recurring_document_html(&state.engine, ctx.entity_id, id).await {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(e) => err_response(e).into_response(),
        }
    }
}

/// GET /recurring-invoices/{id}/invoices — invoices generated by this template.
pub async fn recurring_history(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::recurring_invoice_history(&state.engine, ctx.entity_id, id).await {
        Ok(items) => Ok(Json(serde_json::to_value(items).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
