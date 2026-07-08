use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{AuthContext};
use zavora_erp_core::bank::*;
use zavora_erp_core::services::bank as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list_accounts(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, BankAccountRow>(
        "SELECT * FROM bank_accounts WHERE entity_id = $1 AND is_active = true ORDER BY name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => {
            // Enrich each account with its balance in the account's OWN currency.
            // The trial balance/GL detail report only in functional (base)
            // currency, so a USD account would otherwise show its KES-equivalent.
            // Here we sum the native transaction-currency amounts on the linked
            // GL account (posted entries only) so the displayed balance matches
            // the bank's own currency.
            let mut out = Vec::with_capacity(r.len());
            for acct in &r {
                let bal = sqlx::query_scalar::<_, rust_decimal::Decimal>(
                    r#"SELECT COALESCE(SUM(COALESCE(jl.debit, 0) - COALESCE(jl.credit, 0)), 0)
                       FROM journal_lines jl
                       JOIN journal_entries je ON je.id = jl.entry_id
                       WHERE jl.entity_id = $1 AND jl.account_code = $2
                         AND je.status = 'posted'"#,
                )
                .bind(ctx.entity_id)
                .bind(&acct.gl_account)
                .fetch_one(state.engine.pool())
                .await
                .unwrap_or_default();
                let mut v = serde_json::to_value(acct).unwrap_or_default();
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("balance".to_string(), serde_json::json!(bal.to_string()));
                }
                out.push(v);
            }
            Ok(Json(serde_json::Value::Array(out)))
        }
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_account(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateBankAccountRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let id = uuid::Uuid::new_v4();
    let currency = req.currency.unwrap_or_else(|| "KES".to_string());
    let gl = req.gl_account.unwrap_or_else(|| "1020".to_string());
    let result = sqlx::query(
        "INSERT INTO bank_accounts (id, entity_id, name, bank_name, account_number, currency, gl_account, feed_provider, feed_enabled) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(id).bind(ctx.entity_id)
    .bind(&req.name).bind(&req.bank_name).bind(&req.account_number)
    .bind(&currency).bind(&gl)
    .bind(req.feed_provider.as_ref().map(|f| serde_json::to_string(f).unwrap_or_default()))
    .bind(req.feed_provider.is_some())
    .execute(state.engine.pool()).await;
    match result {
        Ok(_) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

#[derive(serde::Deserialize)]
pub struct ImportStatementBody {
    pub bank_account_id: Uuid,
    pub filename: String,
    pub content: String,
}

/// POST /bank/import — import a bank statement (CSV/MT940/OFX) into the
/// categorisation queue. Idempotent: re-importing the same file for a bank
/// account is rejected, and individual duplicate lines are skipped.
pub async fn import_statement(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ImportStatementBody>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let req = ImportStatementRequest {
        entity_id: ctx.entity_id,
        bank_account_id: body.bank_account_id,
        filename: body.filename,
        content: body.content,
        imported_by: AgentOrUserId::User(ctx.user_id),
    };
    match svc::import_statement(&state.engine, req).await {
        Ok(result) => Ok(Json(serde_json::to_value(result).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /bank/import/extract — extract candidate transaction rows from a **PDF**
/// bank statement for review. Sends the file to the configured OCR/extraction
/// provider (xberg sidecar), parses the recovered text into rows, and returns
/// them **without writing anything**. The user reviews/edits the rows in the UI
/// and confirms via the normal `POST /bank/import` (CSV) path, so the
/// deterministic importer + idempotency + categorisation queue remain the single
/// source of truth. OCR'd financial rows are never auto-committed.
pub async fn extract_statement(
    _ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    use axum::response::IntoResponse;
    let er = |e: zavora_erp_core::ErpError| err_response(e).into_response();

    const MAX_BYTES: usize = 8 * 1024 * 1024;
    let mut bytes: Vec<u8> = Vec::new();
    let mut filename = "statement.pdf".to_string();
    let mut mime_type = "application/pdf".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| er(zavora_erp_core::ErpError::ValidationFailed { message: format!("invalid upload: {e}") }))?
    {
        if field.name() == Some("file") {
            if let Some(f) = field.file_name() { filename = f.to_string(); }
            if let Some(ct) = field.content_type() { mime_type = ct.to_string(); }
            let data = field.bytes().await.map_err(|e| {
                er(zavora_erp_core::ErpError::ValidationFailed { message: format!("could not read file: {e}") })
            })?;
            bytes = data.to_vec();
        }
    }

    if bytes.is_empty() {
        return Err(er(zavora_erp_core::ErpError::ValidationFailed {
            message: "no file provided (expected a 'file' part)".to_string(),
        }));
    }
    if bytes.len() > MAX_BYTES {
        return Err(er(zavora_erp_core::ErpError::ValidationFailed {
            message: format!("file too large (max {} MiB)", MAX_BYTES / (1024 * 1024)),
        }));
    }

    let lower = filename.to_lowercase();
    let is_spreadsheet = lower.ends_with(".xlsx") || lower.ends_with(".xls") || lower.ends_with(".ods")
        || mime_type.contains("spreadsheet") || mime_type.contains("excel");

    // ── Spreadsheet statements (M-Pesa full statement, bank .xlsx exports) ──
    // Columns are explicit (Paid in / Withdrawn / Balance), so we map them
    // directly — no OCR and no balance reconciliation needed.
    if is_spreadsheet {
        let (rows, recon) = zavora_erp_core::services::statement_xlsx::parse_statement_xlsx_checked(&bytes);
        if rows.is_empty() {
            return Err(er(zavora_erp_core::ErpError::ValidationFailed {
                message: "No transaction rows found in the spreadsheet. Expected columns like Date/Completion Time, Description/Details, and Paid in/Withdrawn (or Debit/Credit) and Balance.".to_string(),
            }));
        }
        return Ok(Json(serde_json::json!({
            "provider": "xlsx",
            "row_count": rows.len(),
            "rows": rows,
            "reconciliation": recon,
        })));
    }

    // ── PDF statements ──
    // Try the local Pdfium text layer first (digital PDFs — the common case in
    // Kenya). Only when there is no text layer (a scanned PDF) do we fall back to
    // the OCR sidecar. This makes the common case work offline with no sidecar.
    let mut provider = "pdfium-local";
    let local = crate::routes::pdf_text::extract_pdf_text(&bytes);
    let text = match local {
        Some(t) => t,
        None => {
            provider = state.ocr.name();
            let input = zavora_erp_core::services::ocr_provider::OcrInput { bytes, mime_type, filename };
            let result = state.ocr.extract(&input).await.map_err(er)?;
            result.raw_text.unwrap_or_default()
        }
    };

    if text.trim().is_empty() {
        return Err(er(zavora_erp_core::ErpError::ValidationFailed {
            message: "Could not read this PDF. It looks like a scanned image with no text layer — enable the OCR sidecar (OCR_PROVIDER=xberg, XBERG_URL) for scanned statements, or export CSV/OFX/XLSX from your bank.".to_string(),
        }));
    }

    let rows = zavora_erp_core::services::statement_pdf::parse_statement_text(&text);

    Ok(Json(serde_json::json!({
        "provider": provider,
        "row_count": rows.len(),
        "rows": rows,
        // Returned so an advanced user can sanity-check / re-map if parsing missed rows.
        "raw_text": text,
    })))
}

/// DELETE /bank-accounts/{id} — soft-delete a bank account (sets is_active = false).
pub async fn delete_account(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let result = sqlx::query(
        "UPDATE bank_accounts SET is_active = false WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(ctx.entity_id)
    .execute(state.engine.pool())
    .await;
    match result {
        Ok(_) => Ok(Json(serde_json::json!({ "status": "deleted", "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn reconcile(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(statement_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::match_bank_lines(&state.engine, ctx.entity_id, statement_id).await {
        Ok(report) => Ok(Json(serde_json::to_value(report).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn confirm_match(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConfirmMatchRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::confirm_match(&state.engine, ctx.entity_id, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "confirmed" }))),
        Err(e) => Err(err_response(e)),
    }
}
