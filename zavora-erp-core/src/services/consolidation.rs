//! Group consolidation with intercompany elimination.
//!
//! Combines the trial balances of a set of group companies (translated to a
//! presentation currency) and **eliminates intercompany balances precisely**:
//! because an intercompany charge posts equal, mirrored amounts to dedicated IC
//! control accounts in both companies (see `services::intercompany`), removing
//! those accounts' balances from the combined TB nets the group's internal
//! dealings to zero while keeping the TB balanced.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;

#[derive(Debug, Clone, Serialize)]
pub struct ConsolLine {
    pub account_code: String,
    pub account_name: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct EliminationLine {
    pub account_code: String,
    pub account_name: String,
    pub debit_removed: Decimal,
    pub credit_removed: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberSummary {
    pub entity_id: Uuid,
    pub name: String,
    pub base_currency: String,
    pub translation_rate: Decimal,
    pub translated: bool,
    pub ownership_pct: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsolidatedTrialBalance {
    pub as_at: NaiveDate,
    pub presentation_currency: String,
    pub members: Vec<MemberSummary>,
    pub lines: Vec<ConsolLine>,
    pub eliminations: Vec<EliminationLine>,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub elimination_total: Decimal,
    pub balanced: bool,
    /// Non-controlling-interest memo: for members owned < 100%, the minority's
    /// share of that member's net (post-translation) movement. A memo — a full
    /// NCI equity split on the face of the report is a follow-up.
    pub nci_memo: Decimal,
}

/// Latest exchange rate on/before `as_at` translating `from`→`to`, scoped to the
/// entity. `None` when no rate is on file (caller falls back to 1:1 + flags it).
async fn translation_rate(
    engine: &ErpEngine,
    entity_id: Uuid,
    from_ccy: &str,
    to_ccy: &str,
    as_at: NaiveDate,
) -> Option<Decimal> {
    if from_ccy.eq_ignore_ascii_case(to_ccy) {
        return Some(Decimal::ONE);
    }
    sqlx::query_scalar::<_, Decimal>(
        "SELECT rate FROM exchange_rates \
         WHERE entity_id = $1 AND from_ccy = $2 AND to_ccy = $3 AND rate_date <= $4 \
         ORDER BY rate_date DESC LIMIT 1",
    )
    .bind(entity_id)
    .bind(from_ccy)
    .bind(to_ccy)
    .bind(as_at)
    .fetch_optional(engine.pool())
    .await
    .ok()
    .flatten()
}

/// A group member for consolidation.
pub struct ConsolMember {
    pub entity_id: Uuid,
    pub name: String,
    pub base_currency: String,
    pub ownership_pct: Decimal,
}

/// Consolidate the given members' trial balances as at a date, translated to
/// `presentation_ccy`, with intercompany control accounts eliminated.
pub async fn consolidate(
    engine: &ErpEngine,
    members: &[ConsolMember],
    as_at: NaiveDate,
    presentation_ccy: &str,
) -> ErpResult<ConsolidatedTrialBalance> {
    let presentation_ccy = presentation_ccy.to_uppercase();

    // The set of IC control account codes across members' posting setups.
    let mut ic_codes: std::collections::HashSet<String> = std::collections::HashSet::new();
    for m in members {
        if let Ok(cfg) = engine.config_for(m.entity_id).await {
            let p = &cfg.posting;
            ic_codes.insert(p.intercompany_receivable.clone());
            ic_codes.insert(p.intercompany_payable.clone());
            ic_codes.insert(p.intercompany_income.clone());
            ic_codes.insert(p.intercompany_expense.clone());
        }
    }

    // account_code -> (debit, credit) combined across members.
    let mut combined: HashMap<String, (Decimal, Decimal)> = HashMap::new();
    let mut member_summaries: Vec<MemberSummary> = Vec::new();
    let mut nci_memo = Decimal::ZERO;

    for m in members {
        let rate_opt = translation_rate(engine, m.entity_id, &m.base_currency, &presentation_ccy, as_at).await;
        let translated = rate_opt.is_some();
        let rate = rate_opt.unwrap_or(Decimal::ONE);

        let movements = sqlx::query_as::<_, (String, Decimal, Decimal)>(
            "SELECT account_code,
                    COALESCE(SUM(functional_debit), 0)  AS debit,
                    COALESCE(SUM(functional_credit), 0) AS credit
             FROM journal_lines
             WHERE entity_id = $1 AND entry_date <= $2
             GROUP BY account_code",
        )
        .bind(m.entity_id)
        .bind(as_at)
        .fetch_all(engine.pool())
        .await
        .unwrap_or_default();

        let mut member_net = Decimal::ZERO; // credit - debit
        for (code, d, c) in movements {
            let td = (d * rate).round_dp(2);
            let tc = (c * rate).round_dp(2);
            member_net += tc - td;
            let e = combined.entry(code).or_insert((Decimal::ZERO, Decimal::ZERO));
            e.0 += td;
            e.1 += tc;
        }

        if m.ownership_pct < Decimal::new(100, 0) {
            let minority = (Decimal::new(100, 0) - m.ownership_pct) / Decimal::new(100, 0);
            nci_memo += (member_net * minority).round_dp(2);
        }

        member_summaries.push(MemberSummary {
            entity_id: m.entity_id,
            name: m.name.clone(),
            base_currency: m.base_currency.clone(),
            translation_rate: rate,
            translated,
            ownership_pct: m.ownership_pct,
        });
    }

    // Account names (code -> name) across the members' charts.
    let entity_ids: Vec<Uuid> = members.iter().map(|m| m.entity_id).collect();
    let name_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT DISTINCT ON (code) code, name FROM accounts WHERE entity_id = ANY($1) ORDER BY code",
    )
    .bind(&entity_ids)
    .fetch_all(engine.pool())
    .await
    .unwrap_or_default();
    let names: HashMap<String, String> = name_rows.into_iter().collect();

    // Eliminate IC control accounts: pull them out of `combined` into eliminations.
    let mut eliminations: Vec<EliminationLine> = Vec::new();
    let mut elimination_total = Decimal::ZERO;
    for code in &ic_codes {
        if let Some((d, c)) = combined.remove(code) {
            if d != Decimal::ZERO || c != Decimal::ZERO {
                elimination_total += d.max(c);
                eliminations.push(EliminationLine {
                    account_code: code.clone(),
                    account_name: names.get(code).cloned().unwrap_or_else(|| code.clone()),
                    debit_removed: d,
                    credit_removed: c,
                });
            }
        }
    }
    eliminations.sort_by(|a, b| a.account_code.cmp(&b.account_code));

    // Build the consolidated lines (net each account to a single side).
    let mut lines: Vec<ConsolLine> = combined
        .into_iter()
        .map(|(code, (d, c))| {
            let net = d - c;
            let (debit, credit) = if net >= Decimal::ZERO { (net, Decimal::ZERO) } else { (Decimal::ZERO, -net) };
            ConsolLine {
                account_name: names.get(&code).cloned().unwrap_or_else(|| code.clone()),
                account_code: code,
                debit,
                credit,
            }
        })
        .filter(|l| l.debit != Decimal::ZERO || l.credit != Decimal::ZERO)
        .collect();
    lines.sort_by(|a, b| a.account_code.cmp(&b.account_code));

    let total_debit: Decimal = lines.iter().map(|l| l.debit).sum();
    let total_credit: Decimal = lines.iter().map(|l| l.credit).sum();
    let balanced = (total_debit - total_credit).abs() < Decimal::new(1, 2);

    Ok(ConsolidatedTrialBalance {
        as_at,
        presentation_currency: presentation_ccy,
        members: member_summaries,
        lines,
        eliminations,
        total_debit,
        total_credit,
        elimination_total,
        balanced,
        nci_memo,
    })
}
