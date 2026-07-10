//! Intercompany accounting for multi-company groups.
//!
//! A **group** is a set of entities consolidated together, one of which is the
//! parent. An **intercompany charge** between two members posts mirrored,
//! balanced journal entries into BOTH ledgers in a single database transaction:
//!
//!   From (charging) company:  DR Intercompany Receivable  / CR Intercompany Income
//!   To   (charged)  company:  DR Intercompany Charges      / CR Intercompany Payable
//!
//! Because both sides land on dedicated IC control accounts with equal amounts,
//! consolidation can eliminate them exactly (see `services::consolidation`).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::services::{journal, periods};
use crate::types::AgentOrUserId;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CompanyGroup {
    pub id: Uuid,
    pub name: String,
    pub presentation_currency: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct GroupMember {
    pub group_id: Uuid,
    pub entity_id: Uuid,
    pub is_parent: bool,
    pub ownership_pct: Decimal,
    /// Resolved display name (from entity_settings).
    #[sqlx(default)]
    pub name: Option<String>,
    #[sqlx(default)]
    pub base_currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct IntercompanyTxn {
    pub id: Uuid,
    pub group_id: Option<Uuid>,
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub tx_date: NaiveDate,
    pub description: String,
    pub from_journal_id: Option<Uuid>,
    pub to_journal_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    #[serde(default)]
    pub presentation_currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub entity_id: Uuid,
    #[serde(default)]
    pub is_parent: bool,
    #[serde(default)]
    pub ownership_pct: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct IntercompanyChargeRequest {
    pub group_id: Option<Uuid>,
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub amount: Decimal,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub tx_date: Option<NaiveDate>,
    #[serde(default)]
    pub description: Option<String>,
}

// ── Groups ────────────────────────────────────────────────────────────────

pub async fn create_group(engine: &ErpEngine, req: CreateGroupRequest, created_by: Uuid) -> ErpResult<CompanyGroup> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ErpError::ValidationFailed { message: "Group name is required".into() });
    }
    let ccy = req.presentation_currency.filter(|c| !c.trim().is_empty()).unwrap_or_else(|| "KES".into());
    let row = sqlx::query_as::<_, CompanyGroup>(
        "INSERT INTO company_groups (name, presentation_currency, created_by)
         VALUES ($1, $2, $3) RETURNING id, name, presentation_currency, created_at",
    )
    .bind(name)
    .bind(ccy.to_uppercase())
    .bind(created_by)
    .fetch_one(engine.pool())
    .await?;
    Ok(row)
}

pub async fn list_groups(engine: &ErpEngine) -> ErpResult<Vec<CompanyGroup>> {
    Ok(sqlx::query_as::<_, CompanyGroup>(
        "SELECT id, name, presentation_currency, created_at FROM company_groups ORDER BY created_at DESC",
    )
    .fetch_all(engine.pool())
    .await?)
}

pub async fn group_members(engine: &ErpEngine, group_id: Uuid) -> ErpResult<Vec<GroupMember>> {
    Ok(sqlx::query_as::<_, GroupMember>(
        "SELECT m.group_id, m.entity_id, m.is_parent, m.ownership_pct,
                s.organization_name AS name, s.base_currency
         FROM company_group_members m
         LEFT JOIN entity_settings s ON s.entity_id = m.entity_id
         WHERE m.group_id = $1
         ORDER BY m.is_parent DESC, name",
    )
    .bind(group_id)
    .fetch_all(engine.pool())
    .await?)
}

pub async fn add_member(engine: &ErpEngine, group_id: Uuid, req: AddMemberRequest) -> ErpResult<()> {
    let ownership = req.ownership_pct.unwrap_or(Decimal::new(100, 0));
    // A group has at most one parent: demote any existing parent when adding one.
    if req.is_parent {
        sqlx::query("UPDATE company_group_members SET is_parent = false WHERE group_id = $1")
            .bind(group_id)
            .execute(engine.pool())
            .await?;
    }
    sqlx::query(
        "INSERT INTO company_group_members (group_id, entity_id, is_parent, ownership_pct)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (group_id, entity_id)
         DO UPDATE SET is_parent = $3, ownership_pct = $4",
    )
    .bind(group_id)
    .bind(req.entity_id)
    .bind(req.is_parent)
    .bind(ownership)
    .execute(engine.pool())
    .await?;
    Ok(())
}

pub async fn remove_member(engine: &ErpEngine, group_id: Uuid, entity_id: Uuid) -> ErpResult<()> {
    sqlx::query("DELETE FROM company_group_members WHERE group_id = $1 AND entity_id = $2")
        .bind(group_id)
        .bind(entity_id)
        .execute(engine.pool())
        .await?;
    Ok(())
}

pub async fn is_member(engine: &ErpEngine, group_id: Uuid, entity_id: Uuid) -> ErpResult<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM company_group_members WHERE group_id = $1 AND entity_id = $2)",
    )
    .bind(group_id)
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?)
}

// ── Intercompany charge (both-sided posting) ─────────────────────────────────

fn line(account: &str, debit: Option<Decimal>, credit: Option<Decimal>, ccy: &str, desc: &str) -> CreateJournalLineRequest {
    CreateJournalLineRequest {
        account_code: account.to_string(),
        debit,
        credit,
        currency: ccy.to_string(),
        fx_rate: None,
        description: Some(desc.to_string()),
        dimensions: None,
    }
}

/// Post a balanced intercompany charge into both companies' ledgers atomically.
pub async fn post_intercompany_charge(
    engine: &ErpEngine,
    req: IntercompanyChargeRequest,
    created_by: Uuid,
) -> ErpResult<IntercompanyTxn> {
    let amount = req.amount.round_dp(2);
    if amount <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed { message: "Charge amount must be positive".into() });
    }
    if req.from_entity_id == req.to_entity_id {
        return Err(ErpError::ValidationFailed { message: "A company cannot charge itself".into() });
    }

    let from_cfg = engine.config_for(req.from_entity_id).await?;
    let to_cfg = engine.config_for(req.to_entity_id).await?;
    let from_ccy = from_cfg.base_currency.clone();
    let to_ccy = to_cfg.base_currency.clone();
    // v1: same-currency intercompany only (both post in their own base). Cross-
    // currency IC (translation on one leg) is a documented follow-up.
    if !from_ccy.eq_ignore_ascii_case(&to_ccy) {
        return Err(ErpError::ValidationFailed {
            message: format!("Cross-currency intercompany not yet supported ({from_ccy} vs {to_ccy})"),
        });
    }
    let ccy = req.currency.filter(|c| !c.trim().is_empty()).unwrap_or_else(|| from_ccy.clone());
    let tx_date = req.tx_date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    let desc = req.description.unwrap_or_default();

    let from_period = periods::period_for_date(engine, req.from_entity_id, tx_date).await?;
    let to_period = periods::period_for_date(engine, req.to_entity_id, tx_date).await?;

    let ref_tag = format!("ICG-{}", &Uuid::new_v4().to_string()[..8]);
    let actor = AgentOrUserId::User(created_by);

    let from_ps = &from_cfg.posting;
    let to_ps = &to_cfg.posting;

    // Charging company: DR IC Receivable / CR IC Income.
    let from_req = CreateJournalEntryRequest {
        date: tx_date,
        source: JournalSource::Manual,
        reference: ref_tag.clone(),
        description: format!("Intercompany charge (out): {desc}").trim().to_string(),
        source_id: None,
        lines: vec![
            line(&from_ps.intercompany_receivable, Some(amount), None, &ccy, "IC receivable"),
            line(&from_ps.intercompany_income, None, Some(amount), &ccy, "IC income"),
        ],
        post_immediately: true,
    };
    // Charged company: DR IC Charges / CR IC Payable.
    let to_req = CreateJournalEntryRequest {
        date: tx_date,
        source: JournalSource::Manual,
        reference: ref_tag.clone(),
        description: format!("Intercompany charge (in): {desc}").trim().to_string(),
        source_id: None,
        lines: vec![
            line(&to_ps.intercompany_expense, Some(amount), None, &ccy, "IC charges"),
            line(&to_ps.intercompany_payable, None, Some(amount), &ccy, "IC payable"),
        ],
        post_immediately: true,
    };

    let mut tx = engine.pool().begin().await?;
    let from_entry =
        journal::create_and_post_in_tx(&mut tx, engine, req.from_entity_id, from_req, from_period.id, actor.clone()).await?;
    let to_entry =
        journal::create_and_post_in_tx(&mut tx, engine, req.to_entity_id, to_req, to_period.id, actor).await?;

    let row = sqlx::query_as::<_, IntercompanyTxn>(
        "INSERT INTO intercompany_transactions
            (group_id, from_entity_id, to_entity_id, amount, currency, tx_date, description,
             from_journal_id, to_journal_id, created_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         RETURNING id, group_id, from_entity_id, to_entity_id, amount, currency, tx_date,
                   description, from_journal_id, to_journal_id, created_at",
    )
    .bind(req.group_id)
    .bind(req.from_entity_id)
    .bind(req.to_entity_id)
    .bind(amount)
    .bind(ccy.to_uppercase())
    .bind(tx_date)
    .bind(&desc)
    .bind(from_entry.id)
    .bind(to_entry.id)
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(row)
}

/// List intercompany transactions touching any of the given entities.
pub async fn list_intercompany(engine: &ErpEngine, entity_ids: &[Uuid]) -> ErpResult<Vec<IntercompanyTxn>> {
    Ok(sqlx::query_as::<_, IntercompanyTxn>(
        "SELECT id, group_id, from_entity_id, to_entity_id, amount, currency, tx_date, description,
                from_journal_id, to_journal_id, created_at
         FROM intercompany_transactions
         WHERE from_entity_id = ANY($1) OR to_entity_id = ANY($1)
         ORDER BY tx_date DESC, created_at DESC
         LIMIT 200",
    )
    .bind(entity_ids)
    .fetch_all(engine.pool())
    .await?)
}
