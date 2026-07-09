use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;
use zavora_erp_core::ap::*;
use zavora_erp_core::services::bills as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    axum::extract::Query(page): axum::extract::Query<crate::routes::pagination::PaginationParams>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bills WHERE entity_id = $1")
        .bind(ctx.entity_id).fetch_one(state.engine.pool()).await.unwrap_or(0);
    let rows = sqlx::query_as::<_, BillRow>(
        "SELECT * FROM bills WHERE entity_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
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
    let row = sqlx::query_as::<_, BillRow>(
        "SELECT * FROM bills WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await;

    let lines = sqlx::query_as::<_, zavora_erp_core::invoicing::InvoiceLineRow>(
        "SELECT id, bill_id AS invoice_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount, dimensions FROM bill_lines WHERE bill_id = $1",
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
            // GL determination from the entity's posting config (not hardcoded).
            let posting = state.engine.posting_for(ctx.entity_id).await.map_err(err_response)?;
            // Lines post in the BILL's currency at the bill's fx_rate — a USD
            // bill previously posted with currency=KES + fx_rate, so functional
            // amounts were rate-multiplied while the ledger claimed base
            // currency (mirrors post_invoice, which got this right).
            use zavora_erp_core::ledger::journal::CreateJournalLineRequest as JLine;
            let zero = rust_decimal::Decimal::ZERO;

            // Debit lines: one DR per bill line on its own account, carrying the
            // line's analytical dimensions. Lines for inventory-tracked products
            // debit the GRNI clearing account instead of expense — the goods
            // receipt already booked DR Inventory / CR GRNI, so the bill clears
            // GRNI rather than double-counting stock as an expense (COGS books
            // at issue). Falls back to a single default-expense line if the bill
            // has no captured lines.
            let bill_lines = sqlx::query_as::<_, (String, rust_decimal::Decimal, serde_json::Value, bool)>(
                r#"SELECT bl.account_code, bl.line_total, bl.dimensions,
                          COALESCE(p.track_inventory, false)
                   FROM bill_lines bl
                   LEFT JOIN products p ON p.id = bl.product_id AND p.entity_id = $2
                   WHERE bl.bill_id = $1 ORDER BY bl.id"#,
            )
            .bind(id)
            .bind(ctx.entity_id)
            .fetch_all(state.engine.pool())
            .await
            .unwrap_or_default();

            let mut lines: Vec<JLine> = Vec::new();
            if bill_lines.is_empty() {
                lines.push(JLine {
                    account_code: posting.default_expense.clone(),
                    debit: Some(b.subtotal),
                    credit: None,
                    currency: b.currency.clone(),
                    fx_rate: Some(b.fx_rate),
                    description: Some(format!("Bill {}", b.number)),
                    dimensions: None,
                });
            } else {
                for (account_code, line_total, dims, tracks_inventory) in &bill_lines {
                    let (account, desc) = if *tracks_inventory {
                        (posting.inventory_clearing.clone(), format!("Bill {} — GRNI cleared", b.number))
                    } else {
                        (account_code.clone(), format!("Bill {}", b.number))
                    };
                    lines.push(JLine {
                        account_code: account,
                        debit: Some(*line_total),
                        credit: None,
                        currency: b.currency.clone(),
                        fx_rate: Some(b.fx_rate),
                        description: Some(desc),
                        dimensions: serde_json::from_value(dims.clone()).ok(),
                    });
                }
            }

            // DR VAT Input (recoverable input tax). Input-VAT account is routed by
            // the vendor's VAT posting group, falling back to the flat setup.
            if b.tax_total > zero {
                let vat_account = zavora_erp_core::posting::groups::resolve_vat_input(&state.engine, ctx.entity_id, b.vendor_id, None)
                    .await
                    .unwrap_or_else(|| posting.vat_input.clone());
                lines.push(JLine {
                    account_code: vat_account,
                    debit: Some(b.tax_total),
                    credit: None,
                    currency: b.currency.clone(),
                    fx_rate: Some(b.fx_rate),
                    description: Some("VAT Input".to_string()),
                    dimensions: None,
                });
            }

            // CR Accounts Payable for the net owed to the vendor and CR WHT
            // Payable for the amount withheld. gross_total is already net of WHT
            // (subtotal + tax - wht), so AP + WHT == subtotal + tax == debits.
            let ap_account = zavora_erp_core::posting::groups::resolve_payables(&state.engine, ctx.entity_id, b.vendor_id)
                .await
                .unwrap_or_else(|| posting.accounts_payable.clone());
            lines.push(JLine {
                account_code: ap_account,
                debit: None,
                credit: Some(b.gross_total),
                currency: b.currency.clone(),
                fx_rate: Some(b.fx_rate),
                description: Some(format!("Bill {} - AP", b.number)),
                dimensions: None,
            });
            if b.wht_amount > zero {
                lines.push(JLine {
                    account_code: posting.wht_payable.clone(),
                    debit: None,
                    credit: Some(b.wht_amount),
                    currency: b.currency.clone(),
                    fx_rate: Some(b.fx_rate),
                    description: Some("WHT withheld".to_string()),
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
                    // JE + bill status flip commit together — the old two-step
                    // (post, then UPDATE … .ok()) could leave a posted JE with
                    // the bill still 'approved', silently double-postable.
                    let post_atomic = async {
                        let mut tx = state.engine.pool().begin().await?;
                        let entry = zavora_erp_core::services::journal::create_and_post_in_tx(
                            &mut tx, &state.engine, ctx.entity_id, entry_req, p.id, actor,
                        )
                        .await?;
                        sqlx::query("UPDATE bills SET status = 'posted', journal_entry_id = $1 WHERE id = $2 AND entity_id = $3")
                            .bind(entry.id).bind(id).bind(ctx.entity_id)
                            .execute(&mut *tx)
                            .await?;
                        tx.commit().await?;
                        Ok::<_, zavora_erp_core::ErpError>(entry)
                    };
                    match post_atomic.await {
                        Ok(entry) => Ok(Json(serde_json::json!({ "journal_entry_id": entry.id }))),
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
    match svc::delete_bill_draft(&state.engine, ctx.entity_id, id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "id": id, "deleted": true }))),
        Err(e) => Err(err_response(e)),
    }
}
