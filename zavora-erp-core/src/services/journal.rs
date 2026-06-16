use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::*;
use crate::money::{round_money, rounding_outcome, RoundingOutcome};
use crate::period::{FiscalPeriod, PeriodStatus};
use crate::types::AgentOrUserId;

/// Convenience alias for a Postgres transaction handle.
pub type PgTx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// Validate a journal entry request without posting.
pub async fn validate_entry(
    engine: &ErpEngine,
    entity_id: Uuid,
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
        .bind(entity_id)
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
        .bind(entity_id)
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
        .bind(entity_id)
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

/// Enforce period status rules for journal entry insertion.
///
/// - If the period is **SoftClosed**, only entries with source `Manual` are allowed
///   (prior-period adjustments). All other sources are rejected.
/// - If the period is **HardClosed**, ALL entries are rejected as a defence-in-depth
///   measure alongside the database trigger.
/// - Open or Future periods allow all entries (Future is unlikely in practice).
pub async fn enforce_period_status(
    engine: &ErpEngine,
    entity_id: Uuid,
    period_id: Uuid,
    source: &JournalSource,
) -> ErpResult<()> {
    let period = sqlx::query_as::<_, FiscalPeriod>(
        "SELECT * FROM fiscal_periods WHERE id = $1 AND entity_id = $2",
    )
    .bind(period_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "fiscal_period".to_string(),
        id: period_id,
    })?;

    let status = period.parsed_status();

    match status {
        PeriodStatus::HardClosed => {
            return Err(ErpError::PeriodClosedDetailed {
                period_name: period.name.clone(),
                status: "HardClosed".to_string(),
                period_id: period.id,
            });
        }
        PeriodStatus::SoftClosed => {
            if *source != JournalSource::Manual {
                return Err(ErpError::PeriodClosedDetailed {
                    period_name: period.name.clone(),
                    status: "SoftClosed".to_string(),
                    period_id: period.id,
                });
            }
            // Manual entries (prior-period adjustments) are allowed in SoftClosed
        }
        PeriodStatus::Open | PeriodStatus::Future => {
            // All entries allowed
        }
    }

    Ok(())
}

/// Create and immediately post a journal entry in its own transaction.
///
/// Thin wrapper around [`create_and_post_in_tx`] for callers that have no
/// surrounding transaction to thread through.
pub async fn create_and_post(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateJournalEntryRequest,
    period_id: Uuid,
    posted_by: AgentOrUserId,
) -> ErpResult<JournalEntry> {
    let mut tx = engine.pool().begin().await?;
    let entry = create_and_post_in_tx(&mut tx, engine, entity_id, req, period_id, posted_by).await?;
    tx.commit().await?;
    emit_journal_audit(engine, entity_id, &entry).await;
    Ok(entry)
}

/// Create and immediately post a journal entry **within a caller-provided
/// transaction** (Requirement 2).
///
/// The caller owns the transaction lifecycle, so balance updates, document
/// status changes, and the journal entry all commit or roll back together.
///
/// Monetary amounts are rounded to 2 decimal places (banker's rounding). If the
/// entry is left imbalanced by <= 0.01 due to VAT line-level rounding, a
/// rounding-adjustment line is inserted to the configured account (Req 5.3);
/// a larger imbalance is rejected as genuinely unbalanced (Req 2.6).
pub async fn create_and_post_in_tx(
    tx: &mut PgTx<'_>,
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateJournalEntryRequest,
    period_id: Uuid,
    posted_by: AgentOrUserId,
) -> ErpResult<JournalEntry> {
    enforce_period_status_in_tx(tx, entity_id, period_id, &req.source).await?;

    let now = Utc::now();
    let entry_id = Uuid::new_v4();
    let number = generate_journal_number_in_tx(tx, engine, entity_id).await?;

    // Build journal lines with rounded transaction and functional amounts.
    let mut lines: Vec<JournalLine> = req
        .lines
        .iter()
        .map(|l| {
            let fx_rate = l.fx_rate.unwrap_or(Decimal::ONE);
            JournalLine {
                id: Uuid::new_v4(),
                account_code: l.account_code.clone(),
                debit: l.debit.map(round_money),
                credit: l.credit.map(round_money),
                currency: l.currency.clone(),
                fx_rate,
                functional_debit: l.debit.map(|d| round_money(d * fx_rate)),
                functional_credit: l.credit.map(|c| round_money(c * fx_rate)),
                description: l.description.clone(),
                dimensions: l.dimensions.clone().unwrap_or_default(),
            }
        })
        .collect();

    // Balance check with sub-cent rounding tolerance.
    let total_debits: Decimal = lines.iter().filter_map(|l| l.functional_debit).sum();
    let total_credits: Decimal = lines.iter().filter_map(|l| l.functional_credit).sum();

    match rounding_outcome(total_debits, total_credits) {
        RoundingOutcome::Balanced => {}
        RoundingOutcome::Adjust { debit, amount } => {
            // Absorb the sub-cent residue into the rounding-adjustment account so
            // VAT accumulation cannot block posting.
            let (debit_amt, credit_amt) = if debit {
                (Some(amount), None)
            } else {
                (None, Some(amount))
            };
            lines.push(JournalLine {
                id: Uuid::new_v4(),
                account_code: engine.posting().rounding_adjustment.clone(),
                debit: debit_amt,
                credit: credit_amt,
                currency: engine.config().base_currency.clone(),
                fx_rate: Decimal::ONE,
                functional_debit: debit_amt,
                functional_credit: credit_amt,
                description: Some("Rounding adjustment".to_string()),
                dimensions: Default::default(),
            });
        }
        RoundingOutcome::Unbalanced => {
            return Err(ErpError::Unbalanced {
                debits: total_debits,
                credits: total_credits,
            });
        }
    }

    // Insert journal entry header
    sqlx::query(
        r#"INSERT INTO journal_entries
           (id, entity_id, number, date, period_id, source, reference, description, status, created_by, created_at, posted_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
    )
    .bind(entry_id)
    .bind(entity_id)
    .bind(&number)
    .bind(req.date)
    .bind(period_id)
    .bind(serde_json::to_string(&req.source).unwrap_or_default())
    .bind(&req.reference)
    .bind(&req.description)
    .bind("posted")
    .bind(serde_json::to_value(&posted_by).unwrap_or_default())
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
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
        .execute(&mut **tx)
        .await?;
    }

    Ok(JournalEntry {
        id: entry_id,
        entity_id,
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

/// Emit a best-effort audit event for a posted journal entry to the Redis stream.
/// Runs after the database transaction commits, so a Redis hiccup never rolls
/// back accounting data.
async fn emit_journal_audit(engine: &ErpEngine, entity_id: Uuid, entry: &JournalEntry) {
    let audit_event = serde_json::json!({
        "event_type": "posted",
        "object_type": "journal_entry",
        "object_id": entry.id,
        "actor": entry.created_by,
        "timestamp": entry.created_at,
    });
    let stream_key = format!("erp:audit:{}", entity_id);
    let mut redis_conn = engine.redis_conn().await;
    let _: Result<(), _> = redis::cmd("XADD")
        .arg(&stream_key)
        .arg("*")
        .arg("data")
        .arg(audit_event.to_string())
        .query_async(&mut redis_conn)
        .await;
}

/// Generate the next journal entry number within a transaction.
async fn generate_journal_number_in_tx(
    tx: &mut PgTx<'_>,
    engine: &ErpEngine,
    entity_id: Uuid,
) -> ErpResult<String> {
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
    .bind(entity_id)
    .fetch_one(&mut **tx)
    .await?;

    let prefix = &engine.config().sequences.journal_prefix;
    let fiscal_year = chrono::Utc::now().format("%Y").to_string();

    if engine.config().sequences.year_reset {
        Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
    } else {
        Ok(format!("{}-{:06}", prefix, row))
    }
}

/// Period-status enforcement against a transaction (see [`enforce_period_status`]).
pub async fn enforce_period_status_in_tx(
    tx: &mut PgTx<'_>,
    entity_id: Uuid,
    period_id: Uuid,
    source: &JournalSource,
) -> ErpResult<()> {
    let period = sqlx::query_as::<_, FiscalPeriod>(
        "SELECT * FROM fiscal_periods WHERE id = $1 AND entity_id = $2",
    )
    .bind(period_id)
    .bind(entity_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "fiscal_period".to_string(),
        id: period_id,
    })?;

    match period.parsed_status() {
        PeriodStatus::HardClosed => Err(ErpError::PeriodClosedDetailed {
            period_name: period.name.clone(),
            status: "HardClosed".to_string(),
            period_id: period.id,
        }),
        PeriodStatus::SoftClosed if *source != JournalSource::Manual => {
            Err(ErpError::PeriodClosedDetailed {
                period_name: period.name.clone(),
                status: "SoftClosed".to_string(),
                period_id: period.id,
            })
        }
        _ => Ok(()),
    }
}
