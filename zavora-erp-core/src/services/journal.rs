use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::*;
use crate::types::AgentOrUserId;

/// Validate a journal entry request without posting.
pub async fn validate_entry(
    engine: &ErpEngine,
    req: &CreateJournalEntryRequest,
) -> ErpResult<ValidationReport> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Rule 1: Must have at least 2 lines
    if req.lines.len() < 2 {
        errors.push("Journal entry must have at least 2 lines".to_string());
    }

    // Rule 2: Each line must have either debit or credit (not both, not neither)
    for (i, line) in req.lines.iter().enumerate() {
        match (line.debit, line.credit) {
            (Some(d), Some(c)) if d > Decimal::ZERO && c > Decimal::ZERO => {
                errors.push(format!(
                    "Line {} has both debit and credit; use separate lines",
                    i + 1
                ));
            }
            (None, None) => {
                errors.push(format!("Line {} has neither debit nor credit", i + 1));
            }
            (Some(d), _) if d < Decimal::ZERO => {
                errors.push(format!("Line {} has negative debit", i + 1));
            }
            (_, Some(c)) if c < Decimal::ZERO => {
                errors.push(format!("Line {} has negative credit", i + 1));
            }
            _ => {}
        }
    }

    // Rule 3: Sum of debits must equal sum of credits (in functional currency)
    let base_ccy = &engine.config().base_currency;
    let mut total_func_debits = Decimal::ZERO;
    let mut total_func_credits = Decimal::ZERO;

    for line in &req.lines {
        let fx_rate = line.fx_rate.unwrap_or(Decimal::ONE);
        if let Some(d) = line.debit {
            total_func_debits += d * fx_rate;
        }
        if let Some(c) = line.credit {
            total_func_credits += c * fx_rate;
        }

        // Rule 4: FX rate required for non-base currency
        if line.currency != *base_ccy && line.fx_rate.is_none() {
            warnings.push(format!(
                "Line with account {} in {} has no FX rate; will use rate of 1.0",
                line.account_code, line.currency
            ));
        }
    }

    if total_func_debits != total_func_credits {
        errors.push(format!(
            "Entry is unbalanced: functional debits={}, credits={}",
            total_func_debits, total_func_credits
        ));
    }

    // Rule 5: Validate account codes exist and are active
    for line in &req.lines {
        let account = sqlx::query_scalar::<_, bool>(
            "SELECT is_active FROM accounts WHERE entity_id = $1 AND code = $2",
        )
        .bind(engine.entity_id())
        .bind(&line.account_code)
        .fetch_optional(engine.pool())
        .await?;

        match account {
            None => {
                errors.push(format!("Account {} not found", line.account_code));
            }
            Some(false) => {
                errors.push(format!("Account {} is inactive", line.account_code));
            }
            Some(true) => {}
        }

        // Rule 6: Control accounts cannot be posted directly
        let is_control = sqlx::query_scalar::<_, bool>(
            "SELECT is_control FROM accounts WHERE entity_id = $1 AND code = $2",
        )
        .bind(engine.entity_id())
        .bind(&line.account_code)
        .fetch_optional(engine.pool())
        .await?;

        if is_control == Some(true) {
            errors.push(format!(
                "Account {} is a control account and cannot be posted directly",
                line.account_code
            ));
        }
    }

    // Rule 7: Reference uniqueness
    if !req.reference.is_empty() {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM journal_entries WHERE entity_id = $1 AND reference = $2)",
        )
        .bind(engine.entity_id())
        .bind(&req.reference)
        .fetch_one(engine.pool())
        .await?;

        if exists {
            errors.push(format!("Reference '{}' already exists", req.reference));
        }
    }

    Ok(ValidationReport {
        is_valid: errors.is_empty(),
        errors,
        warnings,
    })
}

/// Create and immediately post a journal entry.
pub async fn create_and_post(
    engine: &ErpEngine,
    req: CreateJournalEntryRequest,
    period_id: Uuid,
    posted_by: AgentOrUserId,
) -> ErpResult<JournalEntry> {
    let now = Utc::now();
    let entry_id = Uuid::new_v4();

    // Generate entry number
    let number = generate_journal_number(engine).await?;

    // Build journal lines with functional amounts
    let base_ccy = &engine.config().base_currency;
    let lines: Vec<JournalLine> = req
        .lines
        .iter()
        .map(|l| {
            let fx_rate = l.fx_rate.unwrap_or(Decimal::ONE);
            JournalLine {
                id: Uuid::new_v4(),
                account_code: l.account_code.clone(),
                debit: l.debit,
                credit: l.credit,
                currency: l.currency.clone(),
                fx_rate,
                functional_debit: l.debit.map(|d| d * fx_rate),
                functional_credit: l.credit.map(|c| c * fx_rate),
                description: l.description.clone(),
                dimensions: l.dimensions.clone().unwrap_or_default(),
            }
        })
        .collect();

    // Final balance check
    let total_debits: Decimal = lines.iter().filter_map(|l| l.functional_debit).sum();
    let total_credits: Decimal = lines.iter().filter_map(|l| l.functional_credit).sum();
    if total_debits != total_credits {
        return Err(ErpError::Unbalanced {
            debits: total_debits,
            credits: total_credits,
        });
    }

    // Insert into database within a transaction
    let mut tx = engine.pool().begin().await?;

    // Insert journal entry header
    sqlx::query(
        r#"INSERT INTO journal_entries 
           (id, entity_id, number, date, period_id, source, reference, description, status, created_by, created_at, posted_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
    )
    .bind(entry_id)
    .bind(engine.entity_id())
    .bind(&number)
    .bind(req.date)
    .bind(period_id)
    .bind(serde_json::to_string(&req.source).unwrap_or_default())
    .bind(&req.reference)
    .bind(&req.description)
    .bind("posted")
    .bind(serde_json::to_value(&posted_by).unwrap_or_default())
    .bind(now)
    .bind(now) // posted_at = now since post_immediately
    .execute(&mut *tx)
    .await?;

    // Insert journal lines
    for line in &lines {
        sqlx::query(
            r#"INSERT INTO journal_lines 
               (id, entry_id, account_code, debit, credit, currency, fx_rate, functional_debit, functional_credit, description, dimensions)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
        )
        .bind(line.id)
        .bind(entry_id)
        .bind(&line.account_code)
        .bind(line.debit)
        .bind(line.credit)
        .bind(&line.currency)
        .bind(line.fx_rate)
        .bind(line.functional_debit)
        .bind(line.functional_credit)
        .bind(&line.description)
        .bind(serde_json::to_value(&line.dimensions).unwrap_or_default())
        .execute(&mut *tx)
        .await?;
    }

    // Emit audit event to Redis stream
    let audit_event = serde_json::json!({
        "event_type": "posted",
        "object_type": "journal_entry",
        "object_id": entry_id,
        "actor": posted_by,
        "timestamp": now,
    });

    // Redis audit emission (best-effort within transaction)
    let stream_key = format!("erp:audit:{}", engine.entity_id());
    let mut redis_conn = engine.redis_conn().await;
    let _: Result<(), _> = redis::cmd("XADD")
        .arg(&stream_key)
        .arg("*")
        .arg("data")
        .arg(audit_event.to_string())
        .query_async(&mut redis_conn)
        .await;

    tx.commit().await?;

    Ok(JournalEntry {
        id: entry_id,
        entity_id: engine.entity_id(),
        number,
        date: req.date,
        period_id,
        source: req.source,
        reference: req.reference,
        description: req.description,
        lines,
        status: EntryStatus::Posted,
        created_by: posted_by,
        created_at: now,
        posted_at: Some(now),
    })
}

/// Generate the next journal entry number.
async fn generate_journal_number(engine: &ErpEngine) -> ErpResult<String> {
    // Atomic increment using Postgres advisory lock + sequence
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(
               sequences, 
               '{journal_next}', 
               to_jsonb((sequences->>'journal_next')::bigint + 1)
           )
           WHERE entity_id = $1
           RETURNING (sequences->>'journal_next')::bigint - 1"#,
    )
    .bind(engine.entity_id())
    .fetch_one(engine.pool())
    .await?;

    let prefix = &engine.config().sequences.journal_prefix;
    let fiscal_year = chrono::Utc::now().format("%Y").to_string();

    if engine.config().sequences.year_reset {
        Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
    } else {
        Ok(format!("{}-{:06}", prefix, row))
    }
}
