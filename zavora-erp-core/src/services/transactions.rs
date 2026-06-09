use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::transactions::*;

/// Categorise a transaction (assign a GL account).
pub async fn categorise(engine: &ErpEngine, req: CategoriseRequest) -> ErpResult<()> {
    sqlx::query(
        "UPDATE imported_transactions SET assigned_account = $1, category_status = 'categorised' WHERE id = $2 AND entity_id = $3",
    )
    .bind(&req.account_code)
    .bind(req.transaction_id)
    .bind(engine.entity_id())
    .execute(engine.pool())
    .await?;
    Ok(())
}

/// Split a transaction into multiple GL parts.
pub async fn split_transaction(engine: &ErpEngine, req: SplitRequest) -> ErpResult<Vec<Uuid>> {
    // Validate parts sum to original amount
    let original = sqlx::query_as::<_, ImportedTransactionRow>(
        "SELECT * FROM imported_transactions WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.transaction_id)
    .bind(engine.entity_id())
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
        .bind(engine.entity_id())
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
pub async fn merge_transactions(engine: &ErpEngine, req: MergeRequest) -> ErpResult<()> {
    // Mark duplicates as merged into primary
    for dup_id in &req.duplicate_ids {
        sqlx::query(
            "UPDATE imported_transactions SET merged_into = $1, category_status = 'excluded' WHERE id = $2 AND entity_id = $3",
        )
        .bind(req.primary_id)
        .bind(dup_id)
        .bind(engine.entity_id())
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
    .bind(query.status.map(|s| serde_json::to_string(&s).unwrap_or_default()))
    .bind(limit)
    .bind(offset)
    .fetch_all(engine.pool())
    .await?;

    Ok(rows)
}
