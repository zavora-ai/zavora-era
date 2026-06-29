use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::transactions::*;

/// Categorise a transaction: assign the contra GL account, **post the
/// double-entry journal** (bank vs contra), link it, and mark the line posted.
///
/// The categorisation queue exists to turn an imported bank line into a ledger
/// entry. Tagging an account without posting would leave the bank GL and the
/// contra account untouched — the books wouldn't move — so categorising posts
/// the journal immediately:
///   * money out (debit line): DR contra account / CR bank GL
///   * money in  (credit line): DR bank GL / CR contra account
/// Idempotent-ish: a line already linked to a journal entry is not re-posted.
pub async fn categorise(engine: &ErpEngine, entity_id: Uuid, req: CategoriseRequest) -> ErpResult<()> {
    // Load the transaction.
    let txn = sqlx::query_as::<_, ImportedTransactionRow>(
        "SELECT * FROM imported_transactions WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.transaction_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "ImportedTransaction".to_string(),
        id: req.transaction_id,
    })?;

    // Don't double-post a line that was already posted to the ledger.
    if txn.journal_entry_id.is_some() {
        return Err(ErpError::ValidationFailed {
            message: "Transaction is already posted to the ledger.".to_string(),
        });
    }

    let amount = txn.debit.or(txn.credit).unwrap_or(Decimal::ZERO);
    if amount == Decimal::ZERO {
        return Err(ErpError::ValidationFailed {
            message: "Transaction has no debit or credit amount to post.".to_string(),
        });
    }

    // Resolve the bank's GL account (per bank account), else the tenant default.
    let bank_gl = match sqlx::query_scalar::<_, String>(
        "SELECT gl_account FROM bank_accounts WHERE id = $1 AND entity_id = $2",
    )
    .bind(txn.bank_account)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    {
        Some(a) => a,
        None => engine.posting_for(entity_id).await?.default_bank.clone(),
    };

    let base_currency = engine.config_for(entity_id).await?.base_currency.clone();
    let description = req
        .description
        .clone()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| txn.description.clone());

    let contra = CreateJournalLineRequest {
        account_code: req.account_code.clone(),
        debit: if txn.debit.is_some() { Some(amount) } else { None },
        credit: if txn.debit.is_some() { None } else { Some(amount) },
        currency: base_currency.clone(),
        fx_rate: None,
        description: Some(description.clone()),
        dimensions: None,
    };
    let bank = CreateJournalLineRequest {
        account_code: bank_gl,
        debit: if txn.debit.is_some() { None } else { Some(amount) },
        credit: if txn.debit.is_some() { Some(amount) } else { None },
        currency: base_currency,
        fx_rate: None,
        description: Some(description.clone()),
        dimensions: None,
    };
    // Money out → contra first (DR), bank (CR). Money in → bank first (DR), contra (CR).
    let lines = if txn.debit.is_some() { vec![contra, bank] } else { vec![bank, contra] };

    let period = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM fiscal_periods WHERE entity_id = $1 AND start_date <= $2 AND end_date >= $2",
    )
    .bind(entity_id)
    .bind(txn.value_date)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::ValidationFailed {
        message: format!("No fiscal period found for date {}", txn.value_date),
    })?;

    let je = crate::services::journal::create_and_post(
        engine,
        entity_id,
        CreateJournalEntryRequest {
            date: txn.value_date,
            source: JournalSource::Payment,
            source_id: None,
            reference: txn.reference.clone(),
            description,
            lines,
            post_immediately: true,
        },
        period,
        req.categorised_by.clone(),
    )
    .await?;

    sqlx::query(
        "UPDATE imported_transactions SET assigned_account = $1, category_status = 'posted', journal_entry_id = $2 WHERE id = $3 AND entity_id = $4",
    )
    .bind(&req.account_code)
    .bind(je.id)
    .bind(req.transaction_id)
    .bind(entity_id)
    .execute(engine.pool())
    .await?;
    Ok(())
}

/// Split a transaction into multiple GL parts.
pub async fn split_transaction(engine: &ErpEngine, entity_id: Uuid, req: SplitRequest) -> ErpResult<Vec<Uuid>> {
    // Validate parts sum to original amount
    let original = sqlx::query_as::<_, ImportedTransactionRow>(
        "SELECT * FROM imported_transactions WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.transaction_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "ImportedTransaction".to_string(),
        id: req.transaction_id,
    })?;

    let original_amount = original.debit.or(original.credit).unwrap_or(Decimal::ZERO);
    let parts_total: Decimal = req.parts.iter().map(|p| p.amount).sum();

    if parts_total != original_amount {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Split parts total ({}) does not match transaction amount ({})",
                parts_total, original_amount
            ),
        });
    }

    // Create split child transactions
    let mut child_ids = Vec::new();
    let is_debit = original.debit.is_some();

    for part in &req.parts {
        let child_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO imported_transactions 
               (id, entity_id, bank_account, value_date, description, reference, debit, credit, running_bal, category_status, assigned_account, import_batch_id, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'categorised', $10, $11, $12)"#,
        )
        .bind(child_id)
        .bind(entity_id)
        .bind(original.bank_account)
        .bind(original.value_date)
        .bind(&part.description)
        .bind(&original.reference)
        .bind(if is_debit { Some(part.amount) } else { None::<Decimal> })
        .bind(if !is_debit { Some(part.amount) } else { None::<Decimal> })
        .bind(original.running_bal)
        .bind(&part.account_code)
        .bind(original.import_batch_id)
        .bind(Utc::now())
        .execute(engine.pool())
        .await?;

        child_ids.push(child_id);
    }

    // Mark original as excluded (replaced by splits)
    sqlx::query(
        "UPDATE imported_transactions SET category_status = 'excluded' WHERE id = $1",
    )
    .bind(req.transaction_id)
    .execute(engine.pool())
    .await?;

    Ok(child_ids)
}

/// Merge duplicate transactions.
pub async fn merge_transactions(engine: &ErpEngine, entity_id: Uuid, req: MergeRequest) -> ErpResult<()> {
    // Mark duplicates as merged into primary
    for dup_id in &req.duplicate_ids {
        sqlx::query(
            "UPDATE imported_transactions SET merged_into = $1, category_status = 'excluded' WHERE id = $2 AND entity_id = $3",
        )
        .bind(req.primary_id)
        .bind(dup_id)
        .bind(entity_id)
        .execute(engine.pool())
        .await?;
    }
    Ok(())
}

/// Get the categorisation queue.
pub async fn get_queue(engine: &ErpEngine, query: TransactionQueueQuery) -> ErpResult<Vec<ImportedTransactionRow>> {
    let limit = query.limit.unwrap_or(50) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let rows = sqlx::query_as::<_, ImportedTransactionRow>(
        r#"SELECT * FROM imported_transactions 
           WHERE entity_id = $1 AND category_status = COALESCE($2, category_status) AND merged_into IS NULL
           ORDER BY value_date DESC
           LIMIT $3 OFFSET $4"#,
    )
    .bind(query.entity_id)
    .bind(query.status.as_ref().map(|s| s.as_db_str()))
    .bind(limit)
    .bind(offset)
    .fetch_all(engine.pool())
    .await?;

    Ok(rows)
}
