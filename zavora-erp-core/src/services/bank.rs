use uuid::Uuid;

use crate::bank::*;
use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::types::AgentOrUserId;

/// Run the three-pass bank reconciliation matching algorithm.
pub async fn match_bank_lines(engine: &ErpEngine, statement_id: Uuid) -> ErpResult<MatchReport> {
    // Pass 1: Exact match — amount + date + reference
    let exact_matches = sqlx::query_as::<_, ExactMatchRow>(
        r#"SELECT it.id as stmt_line_id, je.id as journal_entry_id, 
               COALESCE(it.debit, it.credit) as amount, it.value_date as date
           FROM imported_transactions it
           JOIN journal_entries je ON je.entity_id = it.entity_id AND je.status = 'posted'
           JOIN journal_lines jl ON jl.entry_id = je.id
           WHERE it.import_batch_id = $1 AND it.category_status = 'uncategorised'
           AND je.date = it.value_date
           AND (jl.functional_debit = it.credit OR jl.functional_credit = it.debit)
           AND je.reference = it.reference"#,
    )
    .bind(statement_id)
    .fetch_all(engine.pool())
    .await?;

    let exact: Vec<MatchPair> = exact_matches
        .iter()
        .map(|r| MatchPair {
            statement_line_id: r.stmt_line_id,
            journal_entry_id: r.journal_entry_id,
            amount: r.amount,
            date: r.date,
        })
        .collect();

    // Pass 2 & 3 would involve fuzzy matching and AI — stub for now
    Ok(MatchReport {
        statement_id,
        exact_matches: exact,
        near_matches: Vec::new(),
        ai_suggestions: Vec::new(),
        unmatched: Vec::new(),
    })
}

/// Confirm a reconciliation match.
pub async fn confirm_match(engine: &ErpEngine, req: ConfirmMatchRequest) -> ErpResult<()> {
    sqlx::query(
        "UPDATE imported_transactions SET journal_entry_id = $1, category_status = 'posted' WHERE id = $2",
    )
    .bind(req.journal_entry_id)
    .bind(req.statement_line_id)
    .execute(engine.pool())
    .await?;
    Ok(())
}

/// Post an unmatched bank line as a new journal entry.
pub async fn post_unmatched(engine: &ErpEngine, req: PostUnmatchedRequest) -> ErpResult<Uuid> {
    // Create a journal entry for the unmatched transaction
    let txn = sqlx::query_as::<_, crate::transactions::ImportedTransactionRow>(
        "SELECT * FROM imported_transactions WHERE id = $1",
    )
    .bind(req.statement_line_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "ImportedTransaction".to_string(),
        id: req.statement_line_id,
    })?;

    let _amount = txn.debit.or(txn.credit).unwrap_or_default();

    // Journal entry creation delegated to journal service
    // Mark as posted
    sqlx::query(
        "UPDATE imported_transactions SET assigned_account = $1, category_status = 'posted' WHERE id = $2",
    )
    .bind(&req.account_code)
    .bind(req.statement_line_id)
    .execute(engine.pool())
    .await?;

    Ok(req.statement_line_id) // placeholder — would return journal entry ID
}

#[derive(Debug, sqlx::FromRow)]
struct ExactMatchRow {
    stmt_line_id: Uuid,
    journal_entry_id: Uuid,
    amount: rust_decimal::Decimal,
    date: chrono::NaiveDate,
}
