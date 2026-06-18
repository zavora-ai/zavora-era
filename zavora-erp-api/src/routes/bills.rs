use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_CREATE, ROLES_APPROVE, ROLES_POST_JOURNAL};
use super::err_response;
use zavora_erp_core::ap::*;
use zavora_erp_core::services::bills as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, BillRow>(
        "SELECT * FROM bills WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_one(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, BillRow>(
        "SELECT * FROM bills WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await;

    let lines = sqlx::query_as::<_, zavora_erp_core::invoicing::InvoiceLineRow>(
        "SELECT id, bill_id AS invoice_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount FROM bill_lines WHERE bill_id = $1",
    )
    .bind(id)
    .fetch_all(state.engine.pool()).await.unwrap_or_default();

    match row {
        Ok(Some(bill)) => Ok(Json(serde_json::json!({
            "bill": serde_json::to_value(bill).unwrap_or_default(),
            "lines": serde_json::to_value(lines).unwrap_or_default(),
        }))),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Bill".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateBillRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "create bill").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_bill(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(bill) => Ok(Json(serde_json::to_value(bill).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn approve(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_APPROVE, &ctx, "approve bill").map_err(err_response)?;
    let req = ApproveBillRequest {
        bill_id: id,
        approved_by: ctx.user_id,
    };
    match svc::approve_bill(&state.engine, ctx.entity_id, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "approved" }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn post_bill(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_POST_JOURNAL, &ctx, "post bill").map_err(err_response)?;
    // Post bill to GL: DR Expense / CR AP / DR VAT Input / CR WHT Payable
    let bill = sqlx::query_as::<_, BillRow>(
        "SELECT * FROM bills WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await;

    match bill {
        Ok(Some(b)) => {
            if b.status != "approved" {
                return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
                    message: format!("Bill must be approved before posting (status: {})", b.status),
                }));
            }
            // Build journal lines
            let base_ccy = state.engine.config().base_currency.clone();
            let mut lines = vec![
                zavora_erp_core::ledger::journal::CreateJournalLineRequest {
                    account_code: "7900".to_string(), // default expense
                    debit: Some(b.subtotal),
                    credit: None,
                    currency: base_ccy.clone(),
                    fx_rate: Some(b.fx_rate),
                    description: Some(format!("Bill {}", b.number)),
                    dimensions: None,
                },
                zavora_erp_core::ledger::journal::CreateJournalLineRequest {
                    account_code: "3010".to_string(), // AP
                    debit: None,
                    credit: Some(b.gross_total),
                    currency: base_ccy.clone(),
                    fx_rate: Some(b.fx_rate),
                    description: Some(format!("Bill {} - AP", b.number)),
                    dimensions: None,
                },
            ];
            if b.tax_total > rust_decimal::Decimal::ZERO {
                lines.push(zavora_erp_core::ledger::journal::CreateJournalLineRequest {
                    account_code: "1300".to_string(), // VAT Input
                    debit: Some(b.tax_total),
                    credit: None,
                    currency: base_ccy.clone(),
                    fx_rate: Some(b.fx_rate),
                    description: Some("VAT Input".to_string()),
                    dimensions: None,
                });
            }
            if b.wht_amount > rust_decimal::Decimal::ZERO {
                lines.push(zavora_erp_core::ledger::journal::CreateJournalLineRequest {
                    account_code: "3210".to_string(), // WHT Payable
                    debit: None,
                    credit: Some(b.wht_amount),
                    currency: base_ccy.clone(),
                    fx_rate: Some(b.fx_rate),
                    description: Some("WHT deducted".to_string()),
                    dimensions: None,
                });
            }

            let entry_req = zavora_erp_core::ledger::journal::CreateJournalEntryRequest {
                date: b.issue_date,
                source: zavora_erp_core::ledger::journal::JournalSource::Bill,
                source_id: Some(b.id),
                reference: b.number.clone(),
                description: format!("Bill {} posted", b.number),
                lines,
                post_immediately: true,
            };

            let actor = AgentOrUserId::User(ctx.user_id);
            let period = zavora_erp_core::services::periods::period_for_date(&state.engine, ctx.entity_id, b.issue_date).await;
            match period {
                Ok(p) => {
                    match zavora_erp_core::services::journal::create_and_post(&state.engine, ctx.entity_id, entry_req, p.id, actor).await {
                        Ok(entry) => {
                            sqlx::query("UPDATE bills SET status = 'posted', journal_entry_id = $1 WHERE id = $2")
                                .bind(entry.id).bind(id)
                                .execute(state.engine.pool()).await.ok();
                            Ok(Json(serde_json::json!({ "journal_entry_id": entry.id })))
                        }
                        Err(e) => Err(err_response(e)),
                    }
                }
                Err(e) => Err(err_response(e)),
            }
        }
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Bill".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

/// PUT /bills/{id} — edit a draft bill (replaces lines, recomputes totals).
pub async fn update(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateBillRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "edit bill").map_err(err_response)?;
    match svc::update_bill_draft(&state.engine, ctx.entity_id, id, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "id": id, "updated": true }))),
        Err(e) => Err(err_response(e)),
    }
}

/// DELETE /bills/{id} — delete a draft bill and its lines.
pub async fn delete(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "delete bill").map_err(err_response)?;
    match svc::delete_bill_draft(&state.engine, ctx.entity_id, id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "id": id, "deleted": true }))),
        Err(e) => Err(err_response(e)),
    }
}
