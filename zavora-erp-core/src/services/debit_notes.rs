//! Purchase debit notes — the buyer's document for a supplier return or
//! overcharge claim. Issuing one reduces the payable to the vendor (DR AP) and
//! reverses the original charge (CR the expense/inventory account per line).

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
pub struct DebitNoteRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub vendor_id: Uuid,
    pub applies_to_bill: Option<Uuid>,
    pub po_id: Option<Uuid>,
    pub debit_note_date: NaiveDate,
    pub reason: Option<String>,
    pub currency: String,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub gross_total: Decimal,
    pub status: String,
    pub created_by: Option<Uuid>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DebitNoteLineRow {
    pub id: Uuid,
    pub debit_note_id: Uuid,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub account_code: Option<String>,
    pub line_total: Decimal,
    pub line_no: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDebitNoteRequest {
    pub vendor_id: Uuid,
    pub applies_to_bill: Option<Uuid>,
    pub po_id: Option<Uuid>,
    pub reason: Option<String>,
    pub debit_note_date: Option<NaiveDate>,
    pub currency: Option<String>,
    pub lines: Vec<CreateDebitNoteLineRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDebitNoteLineRequest {
    pub description: String,
    #[serde(default = "one")]
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub account_code: Option<String>,
}
fn one() -> Decimal { Decimal::ONE }

pub async fn list_debit_notes(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<DebitNoteRow>> {
    Ok(sqlx::query_as::<_, DebitNoteRow>("SELECT * FROM purchase_debit_notes WHERE entity_id=$1 ORDER BY created_at DESC")
        .bind(entity_id).fetch_all(engine.pool()).await?)
}

pub async fn get_debit_note(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<serde_json::Value> {
    let dn = sqlx::query_as::<_, DebitNoteRow>("SELECT * FROM purchase_debit_notes WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "debit note".into(), id })?;
    let lines = sqlx::query_as::<_, DebitNoteLineRow>("SELECT * FROM purchase_debit_note_lines WHERE debit_note_id=$1 ORDER BY line_no")
        .bind(id).fetch_all(engine.pool()).await.unwrap_or_default();
    Ok(serde_json::json!({ "debit_note": dn, "lines": lines }))
}

/// Issue a debit note: persist it and post the AP-reducing journal.
pub async fn create_debit_note(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateDebitNoteRequest,
    created_by: Uuid,
) -> ErpResult<DebitNoteRow> {
    if req.lines.is_empty() {
        return Err(ErpError::ValidationFailed { message: "a debit note needs at least one line".into() });
    }
    let date = req.debit_note_date.unwrap_or_else(|| Utc::now().date_naive());
    let currency = req.currency.filter(|c| !c.trim().is_empty()).unwrap_or_else(|| "KES".to_string());
    let number = crate::services::procurement::next_number(engine, entity_id, "debit_note_next", "DN", date).await?;
    let id = Uuid::new_v4();

    let line_totals: Vec<Decimal> = req.lines.iter().map(|l| (l.quantity * l.unit_price).round_dp(2)).collect();
    let subtotal: Decimal = line_totals.iter().copied().sum();

    let dn = sqlx::query_as::<_, DebitNoteRow>(
        r#"INSERT INTO purchase_debit_notes
           (id, entity_id, number, vendor_id, applies_to_bill, po_id, debit_note_date, reason, currency,
            subtotal, tax_total, gross_total, status, created_by)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,0,$10,'issued',$11) RETURNING *"#,
    )
    .bind(id).bind(entity_id).bind(&number).bind(req.vendor_id).bind(req.applies_to_bill).bind(req.po_id)
    .bind(date).bind(&req.reason).bind(&currency).bind(subtotal).bind(created_by)
    .fetch_one(engine.pool()).await?;

    for (i, (l, total)) in req.lines.iter().zip(line_totals.iter()).enumerate() {
        sqlx::query(
            "INSERT INTO purchase_debit_note_lines (debit_note_id, description, quantity, unit_price, account_code, line_total, line_no)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(id).bind(&l.description).bind(l.quantity).bind(l.unit_price).bind(&l.account_code).bind(*total).bind(i as i32)
        .execute(engine.pool()).await?;
    }

    // Journal: DR Accounts Payable (reduce what we owe), CR each line's account.
    let posting = engine.posting_for(entity_id).await?;
    let ap_account = crate::posting::groups::resolve_payables(engine, entity_id, req.vendor_id)
        .await.unwrap_or_else(|| posting.accounts_payable.clone());
    let mut je_lines: Vec<CreateJournalLineRequest> = vec![CreateJournalLineRequest {
        account_code: ap_account,
        debit: Some(subtotal),
        credit: None,
        currency: currency.clone(),
        fx_rate: Some(Decimal::ONE),
        description: Some(format!("Debit note {number} - AP reduction")),
        dimensions: None,
    }];
    for (l, total) in req.lines.iter().zip(line_totals.iter()) {
        let acct = l.account_code.clone().filter(|a| !a.is_empty()).unwrap_or_else(|| posting.default_expense.clone());
        je_lines.push(CreateJournalLineRequest {
            account_code: acct,
            debit: None,
            credit: Some(*total),
            currency: currency.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!("Debit note {number}: {}", l.description)),
            dimensions: None,
        });
    }

    let period = crate::services::periods::period_for_date(engine, entity_id, date).await?;
    crate::services::journal::create_and_post(
        engine,
        entity_id,
        CreateJournalEntryRequest {
            date,
            source: JournalSource::Agent("DebitNote".to_string()),
            source_id: Some(id),
            reference: number.clone(),
            description: format!("Purchase debit note {number}"),
            lines: je_lines,
            post_immediately: true,
        },
        period.id,
        AgentOrUserId::User(created_by),
    ).await?;

    let _ = crate::services::audit::record_event(engine, entity_id, "Created", "debit_note", id,
        &AgentOrUserId::User(created_by), Some(serde_json::json!({ "number": number, "gross": subtotal }))).await;

    Ok(dn)
}
