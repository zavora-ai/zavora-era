use axum::{extract::{Path, Query, State}, Json};
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;
use zavora_erp_core::AgentOrUserId;
use zavora_erp_core::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};

/// GET /tax-filings
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let rows = sqlx::query_as::<_, (Uuid, String, chrono::NaiveDate, chrono::NaiveDate, Decimal, String, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, tax_type, period_from, period_to, amount, status, remitted_at, filed_at
         FROM tax_filings WHERE entity_id = $1 ORDER BY period_to DESC, filed_at DESC",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();
    let items: Vec<_> = rows.into_iter().map(|(id, tax_type, period_from, period_to, amount, status, remitted_at, filed_at)| {
        serde_json::json!({ "id": id, "tax_type": tax_type, "period_from": period_from, "period_to": period_to,
            "amount": amount, "status": status, "remitted_at": remitted_at, "filed_at": filed_at })
    }).collect();
    Json(serde_json::to_value(items).unwrap_or_default())
}

#[derive(serde::Deserialize)]
pub struct FileRequest {
    pub tax_type: String,
    pub period_from: chrono::NaiveDate,
    pub period_to: chrono::NaiveDate,
    pub amount: Decimal,
}

/// POST /tax-filings — record a return as filed for a period.
pub async fn file(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<FileRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO tax_filings (entity_id, tax_type, period_from, period_to, amount, filed_by)
         VALUES ($1,$2,$3,$4,$5,$6) RETURNING id",
    )
    .bind(ctx.entity_id).bind(req.tax_type.trim()).bind(req.period_from).bind(req.period_to).bind(req.amount).bind(ctx.user_id)
    .fetch_one(state.engine.pool())
    .await;
    match id {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

#[derive(serde::Deserialize)]
pub struct RemitRequest {
    pub liability_account: String,
    pub bank_account_code: String,
    pub payment_date: chrono::NaiveDate,
}

/// POST /tax-filings/{id}/remit — record the payment to KRA: DR the tax-liability
/// account / CR the bank, clearing the liability.
pub async fn remit(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<RemitRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {

    let filing = sqlx::query_as::<_, (String, Decimal, String)>(
        "SELECT tax_type, amount, status FROM tax_filings WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await;
    let (tax_type, amount, status) = match filing {
        Ok(Some(f)) => f,
        Ok(None) => return Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "TaxFiling".into(), id })),
        Err(e) => return Err(err_response(zavora_erp_core::ErpError::Database(e))),
    };
    if status == "remitted" {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed { message: "This filing has already been remitted".into() }));
    }

    let base_ccy = state.engine.config().base_currency.clone();
    let lines = vec![
        CreateJournalLineRequest { account_code: req.liability_account.clone(), debit: Some(amount), credit: None, currency: base_ccy.clone(), fx_rate: Some(Decimal::ONE), description: Some(format!("{} remittance", tax_type)), dimensions: None },
        CreateJournalLineRequest { account_code: req.bank_account_code.clone(), debit: None, credit: Some(amount), currency: base_ccy.clone(), fx_rate: Some(Decimal::ONE), description: Some(format!("{} remittance to KRA", tax_type)), dimensions: None },
    ];
    let entry_req = CreateJournalEntryRequest {
        date: req.payment_date,
        source: JournalSource::Payment,
        source_id: Some(id),
        reference: format!("{}-REMIT", tax_type),
        description: format!("{} remittance to KRA", tax_type),
        lines,
        post_immediately: true,
    };
    let period = match zavora_erp_core::services::periods::period_for_date(&state.engine, ctx.entity_id, req.payment_date).await {
        Ok(p) => p,
        Err(e) => return Err(err_response(e)),
    };
    let actor = AgentOrUserId::User(ctx.user_id);
    let entry = match zavora_erp_core::services::journal::create_and_post(&state.engine, ctx.entity_id, entry_req, period.id, actor).await {
        Ok(e) => e,
        Err(e) => return Err(err_response(e)),
    };

    sqlx::query("UPDATE tax_filings SET status = 'remitted', remittance_journal_id = $1, remitted_at = NOW() WHERE id = $2 AND entity_id = $3")
        .bind(entry.id).bind(id).bind(ctx.entity_id)
        .execute(state.engine.pool()).await.ok();

    Ok(Json(serde_json::json!({ "journal_entry_id": entry.id })))
}

#[derive(serde::Deserialize)]
pub struct CitParams {
    pub fiscal_year: Option<i32>,
    /// Manual taxable-profit adjustment (± — disallowables, investment
    /// deductions, loss carry-forwards the ledger can't see).
    pub adjustments: Option<Decimal>,
}

/// GET /tax/cit/estimate — corporation-tax estimate + the installment-tax
/// calendar for a fiscal year (decision support; iTax is the filing of record).
pub async fn cit_estimate(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Query(params): Query<CitParams>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match zavora_erp_core::services::cit::estimate(
        &state.engine,
        ctx.entity_id,
        params.fiscal_year,
        params.adjustments.unwrap_or_default(),
    )
    .await
    {
        Ok(est) => Ok(Json(serde_json::to_value(est).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

#[derive(serde::Deserialize)]
pub struct CitProvisionBody {
    pub fiscal_year: Option<i32>,
    #[serde(default)]
    pub adjustments: Option<Decimal>,
    /// Book this exact amount instead of the incremental estimate true-up
    /// (e.g. the tax agent's final computation).
    #[serde(default)]
    pub amount: Option<Decimal>,
}

/// POST /tax/cit/provision — book the corporation-tax provision
/// (DR 8500 Corporate Income Tax / CR 3510 Corporation Tax Payable).
pub async fn cit_provision(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CitProvisionBody>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::User(ctx.user_id);
    match zavora_erp_core::services::cit::post_provision(
        &state.engine,
        ctx.entity_id,
        body.fiscal_year,
        body.adjustments.unwrap_or_default(),
        body.amount,
        &actor,
    )
    .await
    {
        Ok(res) => Ok(Json(serde_json::to_value(res).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
