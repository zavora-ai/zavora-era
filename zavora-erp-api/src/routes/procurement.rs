//! Staff-side procurement endpoints (buyer): vendor-application review, tenders,
//! bids, award → LPO, purchase-order views. All gated by `AuthContext` +
//! `require_role`; a Vendor token can never reach these (its role is unknown to
//! the staff auth layer).

use axum::extract::{Path, State};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use super::err_response;
use crate::middleware::auth::{AuthContext};
use crate::AppState;
use zavora_erp_core::procurement::*;
use zavora_erp_core::services::procurement as svc;
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;

fn ok<T: serde::Serialize>(v: T) -> ApiResult {
    Ok(Json(serde_json::to_value(v).unwrap_or_default()))
}

// ── Vendor applications ─────────────────────────────────────────────────────

/// GET /api/v1/vendor-applications — pending (and recent) registrations.
pub async fn list_applications(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query_as::<_, VendorUserRow>(
        "SELECT id, entity_id, email, display_name, company_name, kra_pin, phone, status, vendor_id, last_login, created_at \
         FROM vendor_users WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    ok(rows)
}

/// POST /api/v1/vendor-applications/{id}/approve — activate the login and link
/// (or create) the `vendors` master it will transact under.
pub async fn approve_application(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveVendorRequest>,
) -> ApiResult {
    let pool = state.engine.pool();

    let app = sqlx::query_as::<_, VendorUserRow>(
        "SELECT id, entity_id, email, display_name, company_name, kra_pin, phone, status, vendor_id, last_login, created_at \
         FROM vendor_users WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id).fetch_optional(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?
    .ok_or_else(|| err_response_boxed(ErpError::NotFound { entity_type: "vendor application".into(), id }))?;

    // Link the caller-supplied master, else create one from the registration.
    let vendor_id = if let Some(v) = req.vendor_id {
        v
    } else {
        let vid = Uuid::new_v4();
        // `vendors.email`/`.phone` are JSONB arrays of contact strings.
        let emails = serde_json::json!([app.email]);
        let phones = app.phone.as_ref().map(|p| serde_json::json!([p])).unwrap_or_else(|| serde_json::json!([]));
        sqlx::query(
            "INSERT INTO vendors (id, entity_id, name, email, phone, kra_pin, is_active) \
             VALUES ($1,$2,$3,$4,$5,$6,true)",
        )
        .bind(vid).bind(ctx.entity_id).bind(&app.company_name).bind(emails).bind(phones).bind(&app.kra_pin)
        .execute(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
        vid
    };

    sqlx::query("UPDATE vendor_users SET status='active', vendor_id=$1 WHERE id=$2 AND entity_id=$3")
        .bind(vendor_id).bind(id).bind(ctx.entity_id)
        .execute(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;

    ok(serde_json::json!({ "id": id, "status": "active", "vendor_id": vendor_id }))
}

/// POST /api/v1/vendor-applications/{id}/reject
pub async fn reject_application(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult {
    let n = sqlx::query("UPDATE vendor_users SET status='rejected' WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(ctx.entity_id).execute(state.engine.pool()).await
        .map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    if n.rows_affected() == 0 {
        return Err(err_response_boxed(ErpError::NotFound { entity_type: "vendor application".into(), id }));
    }
    ok(serde_json::json!({ "id": id, "status": "rejected" }))
}

// ── Tenders ─────────────────────────────────────────────────────────────────

pub async fn list_tenders(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query_as::<_, TenderRow>("SELECT * FROM tenders WHERE entity_id=$1 ORDER BY created_at DESC")
        .bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    ok(rows)
}

pub async fn get_tender(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let pool = state.engine.pool();
    let tender = sqlx::query_as::<_, TenderRow>("SELECT * FROM tenders WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(ctx.entity_id).fetch_optional(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?
        .ok_or_else(|| err_response_boxed(ErpError::NotFound { entity_type: "tender".into(), id }))?;
    let lines = sqlx::query_as::<_, TenderLineRow>("SELECT * FROM tender_lines WHERE tender_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(pool).await.unwrap_or_default();
    ok(serde_json::json!({ "tender": tender, "lines": lines }))
}

pub async fn create_tender(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateTenderRequest>) -> ApiResult {
    let row = svc::create_tender(&state.engine, ctx.entity_id, req, ctx.user_id).await.map_err(err_response_boxed)?;
    ok(row)
}

pub async fn publish_tender(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    svc::publish_tender(&state.engine, ctx.entity_id, id).await.map_err(err_response_boxed)?;
    ok(serde_json::json!({ "id": id, "status": "open" }))
}

/// GET /api/v1/tenders/{id}/bids — submitted bids for a tender (buyer view).
pub async fn list_bids(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let rows = sqlx::query_as::<_, BidRow>(
        "SELECT * FROM bids WHERE tender_id=$1 AND entity_id=$2 ORDER BY total_amount ASC",
    )
    .bind(id).bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    ok(rows)
}

/// POST /api/v1/tenders/{id}/award — award a bid, build the LPO.
pub async fn award_tender(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<AwardTenderRequest>,
) -> ApiResult {
    let po = svc::award_tender(&state.engine, ctx.entity_id, id, req, ctx.user_id).await.map_err(err_response_boxed)?;
    ok(po)
}

// ── Purchase orders ─────────────────────────────────────────────────────────

pub async fn list_purchase_orders(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query_as::<_, PurchaseOrderRow>("SELECT * FROM purchase_orders WHERE entity_id=$1 ORDER BY created_at DESC")
        .bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    ok(rows)
}

/// GET /api/v1/procurement/analytics — spend by vendor, open commitments, and
/// document counts by status.
pub async fn analytics(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let out = svc::procurement_analytics(&state.engine, ctx.entity_id).await.map_err(err_response_boxed)?;
    ok(out)
}

/// GET /api/v1/procurement/budget-control — budget vs committed vs actual by
/// account (encumbrance view).
pub async fn budget_control(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let out = svc::budget_commitments(&state.engine, ctx.entity_id).await.map_err(err_response_boxed)?;
    ok(out)
}

// ── Purchase requisitions (self-service → approval → convert) ───────────────

pub async fn list_requisitions(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query_as::<_, PurchaseRequisitionRow>("SELECT * FROM purchase_requisitions WHERE entity_id=$1 ORDER BY created_at DESC")
        .bind(ctx.entity_id).fetch_all(state.engine.pool()).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?;
    ok(rows)
}

pub async fn get_requisition(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let pool = state.engine.pool();
    let pr = sqlx::query_as::<_, PurchaseRequisitionRow>("SELECT * FROM purchase_requisitions WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(ctx.entity_id).fetch_optional(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?
        .ok_or_else(|| err_response_boxed(ErpError::NotFound { entity_type: "requisition".into(), id }))?;
    let lines = sqlx::query_as::<_, PurchaseRequisitionLineRow>("SELECT * FROM purchase_requisition_lines WHERE pr_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(pool).await.unwrap_or_default();
    ok(serde_json::json!({ "requisition": pr, "lines": lines }))
}

/// POST /api/v1/requisitions — any staff member can raise one (self-service).
pub async fn create_requisition(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateRequisitionRequest>) -> ApiResult {
    let pr = svc::create_requisition(&state.engine, ctx.entity_id, req, ctx.user_id).await.map_err(err_response_boxed)?;
    ok(pr)
}

pub async fn submit_requisition(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let pr = svc::submit_requisition(&state.engine, ctx.entity_id, id).await.map_err(err_response_boxed)?;
    ok(pr)
}

pub async fn approve_requisition(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let pr = svc::approve_requisition(&state.engine, ctx.entity_id, id, ctx.user_id).await.map_err(err_response_boxed)?;
    ok(pr)
}

pub async fn reject_requisition(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<RejectRequisitionRequest>) -> ApiResult {
    let pr = svc::reject_requisition(&state.engine, ctx.entity_id, id, ctx.user_id, req.reason).await.map_err(err_response_boxed)?;
    ok(pr)
}

/// POST /api/v1/requisitions/{id}/convert — turn an approved requisition into a
/// tender or a direct PO.
pub async fn convert_requisition(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<ConvertRequisitionRequest>) -> ApiResult {
    let out = svc::convert_requisition(&state.engine, ctx.entity_id, id, req, ctx.user_id).await.map_err(err_response_boxed)?;
    ok(out)
}

/// POST /api/v1/purchase-orders — direct procurement: raise an LPO straight
/// against a vendor master (no tender needed, vendor need not be on the portal).
pub async fn create_purchase_order(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreatePurchaseOrderRequest>) -> ApiResult {
    let po = svc::create_purchase_order(&state.engine, ctx.entity_id, req, ctx.user_id).await.map_err(err_response_boxed)?;
    ok(po)
}

pub async fn get_purchase_order(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let pool = state.engine.pool();
    let po = sqlx::query_as::<_, PurchaseOrderRow>("SELECT * FROM purchase_orders WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(ctx.entity_id).fetch_optional(pool).await.map_err(|e| err_response_boxed(ErpError::Database(e)))?
        .ok_or_else(|| err_response_boxed(ErpError::NotFound { entity_type: "purchase order".into(), id }))?;
    let lines = sqlx::query_as::<_, PurchaseOrderLineRow>("SELECT * FROM purchase_order_lines WHERE po_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(pool).await.unwrap_or_default();
    ok(serde_json::json!({ "purchase_order": po, "lines": lines }))
}

// ── Goods receipts + 3-way match ────────────────────────────────────────────

/// POST /api/v1/purchase-orders/{id}/receipts — record a goods receipt (GRN).
pub async fn create_goods_receipt(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<CreateGrnRequest>) -> ApiResult {
    let grn = svc::create_goods_receipt(&state.engine, ctx.entity_id, id, req, ctx.user_id).await.map_err(err_response_boxed)?;
    ok(grn)
}

#[derive(serde::Deserialize)]
pub struct SendPoRequest {
    pub recipient_email: Option<String>,
    pub message: Option<String>,
}

/// POST /api/v1/purchase-orders/{id}/send — email the LPO PDF to the vendor.
pub async fn send_purchase_order(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<SendPoRequest>) -> ApiResult {
    let sent_to = svc::send_purchase_order(&state.engine, ctx.entity_id, id, req.recipient_email, req.message).await.map_err(err_response_boxed)?;
    let _ = zavora_erp_core::services::audit::record_event(&state.engine, ctx.entity_id, "Sent", "purchase_order", id,
        &zavora_erp_core::AgentOrUserId::User(ctx.user_id), Some(serde_json::json!({ "to": sent_to }))).await;
    ok(serde_json::json!({ "sent_to": sent_to }))
}

/// GET /api/v1/purchase-orders/{id}/receipts — GRNs recorded against this PO.
pub async fn list_goods_receipts(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let rows = svc::list_goods_receipts(&state.engine, ctx.entity_id, id).await.map_err(err_response_boxed)?;
    ok(rows)
}

/// GET /api/v1/purchase-orders/{id}/match — the 3-way match report.
pub async fn purchase_order_match(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let m = svc::three_way_match(&state.engine, ctx.entity_id, id).await.map_err(err_response_boxed)?;
    ok(m)
}

/// GET /api/v1/purchase-orders/{id}/document?format=html|pdf — the legal LPO
/// document. `html` is the on-screen preview; `pdf` is that exact sheet printed
/// to PDF (bank-ready). Same renderer as the vendor-portal copy, so both match.
pub async fn purchase_order_document(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<DocumentQuery>,
) -> axum::response::Response {
    render_po_document(&state, ctx.entity_id, id, q.format.as_deref() == Some("pdf")).await
}

/// Shared by the staff + portal PO-document routes: builds the response from the
/// entity + PO id once both surfaces have authorised the request.
pub async fn render_po_document(
    state: &Arc<AppState>,
    entity_id: Uuid,
    id: Uuid,
    want_pdf: bool,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if want_pdf {
        match svc::po_document_pdf(&state.engine, entity_id, id).await {
            Ok((bytes, number)) => {
                let safe: String = number
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
                    .collect();
                let filename = if safe.is_empty() { format!("LPO-{id}") } else { safe };
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
            Err(e) => err_response_boxed(e),
        }
    } else {
        match svc::po_document_html(&state.engine, entity_id, id).await {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(e) => err_response_boxed(e),
        }
    }
}

#[derive(serde::Deserialize)]
pub struct DocumentQuery {
    pub format: Option<String>,
}

/// `err_response` returns `impl IntoResponse`; box it to a concrete `Response`
/// so it can be used with the `?` operator in these handlers.
fn err_response_boxed(e: ErpError) -> axum::response::Response {
    use axum::response::IntoResponse;
    err_response(e).into_response()
}

// ── Purchase debit notes (supplier returns) ─────────────────────────────────
use zavora_erp_core::services::debit_notes as dn_svc;
use zavora_erp_core::services::expense_claims as ec_svc;

pub async fn list_debit_notes(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    ok(dn_svc::list_debit_notes(&state.engine, ctx.entity_id).await.map_err(err_response_boxed)?)
}
pub async fn get_debit_note(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    ok(dn_svc::get_debit_note(&state.engine, ctx.entity_id, id).await.map_err(err_response_boxed)?)
}
pub async fn create_debit_note(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<dn_svc::CreateDebitNoteRequest>) -> ApiResult {
    ok(dn_svc::create_debit_note(&state.engine, ctx.entity_id, req, ctx.user_id).await.map_err(err_response_boxed)?)
}

// ── Expense claims ──────────────────────────────────────────────────────────

pub async fn list_expense_claims(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    ok(ec_svc::list_claims(&state.engine, ctx.entity_id).await.map_err(err_response_boxed)?)
}
pub async fn get_expense_claim(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    ok(ec_svc::get_claim(&state.engine, ctx.entity_id, id).await.map_err(err_response_boxed)?)
}
pub async fn create_expense_claim(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<ec_svc::CreateClaimRequest>) -> ApiResult {
    ok(ec_svc::create_claim(&state.engine, ctx.entity_id, req, ctx.user_id).await.map_err(err_response_boxed)?)
}
pub async fn submit_expense_claim(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    ok(ec_svc::submit_claim(&state.engine, ctx.entity_id, id, ctx.user_id).await.map_err(err_response_boxed)?)
}
pub async fn approve_expense_claim(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    ok(ec_svc::approve_claim(&state.engine, ctx.entity_id, id, ctx.user_id).await.map_err(err_response_boxed)?)
}
pub async fn reject_expense_claim(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<RejectRequisitionRequest>) -> ApiResult {
    ok(ec_svc::reject_claim(&state.engine, ctx.entity_id, id, ctx.user_id, req.reason).await.map_err(err_response_boxed)?)
}
