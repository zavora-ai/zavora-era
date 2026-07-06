//! Staff expense claims — self-service reimbursement. A claimant files lines,
//! submits for approval; on approval we post the expense (DR expense accounts,
//! CR accounts payable as the reimbursement liability) and mark it reimbursable.
//! Delegation-of-Authority limits apply to the approver.

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::types::AgentOrUserId;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ExpenseClaimRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub claimant_id: Uuid,
    pub title: String,
    pub currency: String,
    pub total: Decimal,
    pub status: String,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<chrono::DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ExpenseClaimLineRow {
    pub id: Uuid,
    pub claim_id: Uuid,
    pub expense_date: Option<NaiveDate>,
    pub description: String,
    pub account_code: Option<String>,
    pub amount: Decimal,
    pub line_no: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateClaimRequest {
    pub title: String,
    pub currency: Option<String>,
    pub notes: Option<String>,
    pub lines: Vec<CreateClaimLineRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateClaimLineRequest {
    pub expense_date: Option<NaiveDate>,
    pub description: String,
    pub account_code: Option<String>,
    pub amount: Decimal,
}

pub async fn list_claims(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<ExpenseClaimRow>> {
    Ok(sqlx::query_as::<_, ExpenseClaimRow>("SELECT * FROM expense_claims WHERE entity_id=$1 ORDER BY created_at DESC")
        .bind(entity_id).fetch_all(engine.pool()).await?)
}

pub async fn get_claim(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<serde_json::Value> {
    let claim = sqlx::query_as::<_, ExpenseClaimRow>("SELECT * FROM expense_claims WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "expense claim".into(), id })?;
    let lines = sqlx::query_as::<_, ExpenseClaimLineRow>("SELECT * FROM expense_claim_lines WHERE claim_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(engine.pool()).await.unwrap_or_default();
    Ok(serde_json::json!({ "claim": claim, "lines": lines }))
}

pub async fn create_claim(engine: &ErpEngine, entity_id: Uuid, req: CreateClaimRequest, claimant: Uuid) -> ErpResult<ExpenseClaimRow> {
    if req.lines.is_empty() {
        return Err(ErpError::ValidationFailed { message: "an expense claim needs at least one line".into() });
    }
    let date = Utc::now().date_naive();
    let currency = req.currency.filter(|c| !c.trim().is_empty()).unwrap_or_else(|| "KES".to_string());
    let number = crate::services::procurement::next_number(engine, entity_id, "expense_claim_next", "EXP", date).await?;
    let id = Uuid::new_v4();
    let total: Decimal = req.lines.iter().map(|l| l.amount).sum();

    let claim = sqlx::query_as::<_, ExpenseClaimRow>(
        r#"INSERT INTO expense_claims (id, entity_id, number, claimant_id, title, currency, total, status, notes)
           VALUES ($1,$2,$3,$4,$5,$6,$7,'draft',$8) RETURNING *"#,
    )
    .bind(id).bind(entity_id).bind(&number).bind(claimant).bind(&req.title).bind(&currency).bind(total).bind(&req.notes)
    .fetch_one(engine.pool()).await?;

    for (i, l) in req.lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO expense_claim_lines (claim_id, expense_date, description, account_code, amount, line_no)
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(id).bind(l.expense_date).bind(&l.description).bind(&l.account_code).bind(l.amount).bind(i as i32)
        .execute(engine.pool()).await?;
    }
    Ok(claim)
}

pub async fn submit_claim(engine: &ErpEngine, entity_id: Uuid, id: Uuid, claimant: Uuid) -> ErpResult<ExpenseClaimRow> {
    // A claimant can only submit their own draft.
    let row = sqlx::query_as::<_, ExpenseClaimRow>(
        "UPDATE expense_claims SET status='submitted' WHERE id=$1 AND entity_id=$2 AND claimant_id=$3 AND status='draft' RETURNING *",
    )
    .bind(id).bind(entity_id).bind(claimant)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "claim not found, not yours, or not a draft".into() })?;
    Ok(row)
}

pub async fn reject_claim(engine: &ErpEngine, entity_id: Uuid, id: Uuid, approver: Uuid, reason: Option<String>) -> ErpResult<ExpenseClaimRow> {
    let row = sqlx::query_as::<_, ExpenseClaimRow>(
        "UPDATE expense_claims SET status='rejected', approved_by=$3, approved_at=now(), rejection_reason=$4 \
         WHERE id=$1 AND entity_id=$2 AND status='submitted' RETURNING *",
    )
    .bind(id).bind(entity_id).bind(approver).bind(&reason)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "claim not found or not awaiting approval".into() })?;
    Ok(row)
}

/// Approve a submitted claim: enforce the approver's DoA limit, post the expense
/// journal (DR expense / CR AP reimbursement), and mark it approved.
pub async fn approve_claim(engine: &ErpEngine, entity_id: Uuid, id: Uuid, approver: Uuid) -> ErpResult<ExpenseClaimRow> {
    let claim = sqlx::query_as::<_, ExpenseClaimRow>(
        "SELECT * FROM expense_claims WHERE id=$1 AND entity_id=$2 AND status='submitted'",
    )
    .bind(id).bind(entity_id).fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "claim not found or not awaiting approval".into() })?;

    crate::services::approval::assert_within_limit(engine, entity_id, approver, claim.total, "expense claim").await?;

    let lines = sqlx::query_as::<_, ExpenseClaimLineRow>("SELECT * FROM expense_claim_lines WHERE claim_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(engine.pool()).await?;

    // Journal: DR each expense line, CR accounts payable (reimbursement due).
    let posting = engine.posting_for(entity_id).await?;
    let mut je_lines: Vec<CreateJournalLineRequest> = Vec::new();
    for l in &lines {
        let acct = l.account_code.clone().filter(|a| !a.is_empty()).unwrap_or_else(|| posting.default_expense.clone());
        je_lines.push(CreateJournalLineRequest {
            account_code: acct,
            debit: Some(l.amount),
            credit: None,
            currency: claim.currency.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!("Expense claim {}: {}", claim.number, l.description)),
            dimensions: None,
        });
    }
    je_lines.push(CreateJournalLineRequest {
        account_code: posting.accounts_payable.clone(),
        debit: None,
        credit: Some(claim.total),
        currency: claim.currency.clone(),
        fx_rate: Some(Decimal::ONE),
        description: Some(format!("Expense claim {} - reimbursement payable", claim.number)),
        dimensions: None,
    });

    let period = crate::services::periods::period_for_date(engine, entity_id, Utc::now().date_naive()).await?;
    crate::services::journal::create_and_post(
        engine, entity_id,
        CreateJournalEntryRequest {
            date: Utc::now().date_naive(),
            source: JournalSource::Agent("ExpenseClaim".to_string()),
            source_id: Some(id),
            reference: claim.number.clone(),
            description: format!("Expense claim {}", claim.number),
            lines: je_lines,
            post_immediately: true,
        },
        period.id,
        AgentOrUserId::User(approver),
    ).await?;

    let row = sqlx::query_as::<_, ExpenseClaimRow>(
        "UPDATE expense_claims SET status='approved', approved_by=$3, approved_at=now() WHERE id=$1 AND entity_id=$2 RETURNING *",
    )
    .bind(id).bind(entity_id).bind(approver).fetch_one(engine.pool()).await?;

    let _ = crate::services::audit::record_event(engine, entity_id, "Approved", "expense_claim", id,
        &AgentOrUserId::User(approver), Some(serde_json::json!({ "number": claim.number, "total": claim.total }))).await;
    Ok(row)
}
