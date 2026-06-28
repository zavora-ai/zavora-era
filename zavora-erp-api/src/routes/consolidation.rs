use axum::{extract::State, Json};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::AuthContext;
use super::err_response;

/// Entities the current user (by email) is an active member of — the only ones
/// they may consolidate, so a consolidation can never read foreign tenants.
async fn authorized_entities(state: &AppState, user_id: Uuid) -> Vec<(Uuid, String, String, String)> {
    let email: Option<String> = sqlx::query_scalar("SELECT email FROM era_users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(state.engine.pool())
        .await
        .ok()
        .flatten();
    let Some(email) = email else { return vec![] };
    sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT u.entity_id,
                COALESCE(s.organization_name, '(unnamed)') AS name,
                COALESCE(s.base_currency, 'KES') AS currency,
                u.role
         FROM era_users u
         LEFT JOIN entity_settings s ON s.entity_id = u.entity_id
         WHERE u.email = $1 AND u.is_active = true
         ORDER BY name",
    )
    .bind(email)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default()
}

/// GET /consolidation/entities — entities available to consolidate.
pub async fn my_entities(ctx: AuthContext, State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let rows = authorized_entities(&state, ctx.user_id).await;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, name, currency, role)| serde_json::json!({ "entity_id": id, "name": name, "currency": currency, "role": role }))
        .collect();
    Json(serde_json::to_value(items).unwrap_or_default())
}

#[derive(serde::Deserialize)]
pub struct ConsolidatedRequest {
    pub entity_ids: Vec<Uuid>,
    pub as_at: Option<chrono::NaiveDate>,
    /// Currency to present the consolidated balances in. Defaults to the first
    /// selected entity's base currency.
    #[serde(default)]
    pub presentation_currency: Option<String>,
    /// When true (default), net out intercompany AR/AP between the consolidated
    /// entities (matched by shared KRA PIN).
    #[serde(default = "default_true")]
    pub eliminate_intercompany: bool,
}

fn default_true() -> bool {
    true
}

/// Latest exchange rate (on or before `as_at`) to translate `from_ccy` into the
/// presentation currency, looked up within `entity_id`. Returns `None` when the
/// currencies match (rate 1) is handled by the caller; `None` here means no rate
/// is on file so the caller flags the entity as untranslated.
async fn translation_rate(
    state: &AppState,
    entity_id: Uuid,
    from_ccy: &str,
    to_ccy: &str,
    as_at: chrono::NaiveDate,
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
    .fetch_optional(state.engine.pool())
    .await
    .ok()
    .flatten()
}

/// POST /consolidation/trial-balance — consolidated trial balance across the
/// selected (authorized) entities as at a date.
///
/// Each entity's functional balances are **translated** into the presentation
/// currency via its `exchange_rates` (latest on/before the date; rate 1 when an
/// entity already reports in the presentation currency). **Intercompany AR/AP**
/// between the consolidated entities — receivables from / payables to a party
/// whose KRA PIN matches a sister entity in the set — are netted into an
/// `eliminations` section so the group balance is not overstated.
pub async fn trial_balance(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConsolidatedRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let authorized = authorized_entities(&state, ctx.user_id).await;
    let auth_ids: Vec<Uuid> = authorized.iter().map(|(id, ..)| *id).collect();
    // Only keep requested entities the user is actually a member of.
    let selected: Vec<Uuid> = req.entity_ids.into_iter().filter(|e| auth_ids.contains(e)).collect();

    if selected.is_empty() {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: "Select at least one entity you have access to".to_string(),
        }));
    }

    let as_at = req.as_at.unwrap_or_else(|| chrono::Utc::now().date_naive());

    // Selected entities with their base currency (and KRA PIN for IC matching).
    let chosen: Vec<&(Uuid, String, String, String)> =
        authorized.iter().filter(|(id, ..)| selected.contains(id)).collect();

    // Presentation currency: explicit, else the first selected entity's base.
    let presentation_ccy = req
        .presentation_currency
        .clone()
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| chosen.first().map(|(_, _, cur, _)| cur.clone()).unwrap_or_else(|| "KES".to_string()))
        .to_uppercase();

    // --- Per-entity, per-account functional balances, translated to presentation ccy. ---
    let mut combined: HashMap<String, (Decimal, Decimal)> = HashMap::new(); // code -> (debit, credit)
    let mut untranslated: Vec<serde_json::Value> = Vec::new();

    for (eid, name, base_ccy, _role) in &chosen {
        let rate = translation_rate(&state, *eid, base_ccy, &presentation_ccy, as_at).await;
        let rate = match rate {
            Some(r) => r,
            None => {
                // No rate on file — include at 1:1 but flag it so the result is honest.
                untranslated.push(serde_json::json!({
                    "entity": name, "from": base_ccy, "to": presentation_ccy,
                }));
                Decimal::ONE
            }
        };

        let movements = sqlx::query_as::<_, (String, Decimal, Decimal)>(
            "SELECT account_code,
                    COALESCE(SUM(functional_debit), 0)  AS debit,
                    COALESCE(SUM(functional_credit), 0) AS credit
             FROM journal_lines
             WHERE entity_id = $1 AND entry_date <= $2
             GROUP BY account_code",
        )
        .bind(eid)
        .bind(as_at)
        .fetch_all(state.engine.pool())
        .await
        .unwrap_or_default();

        for (code, d, c) in movements {
            let e = combined.entry(code).or_insert((Decimal::ZERO, Decimal::ZERO));
            e.0 += (d * rate).round_dp(2);
            e.1 += (c * rate).round_dp(2);
        }
    }

    // --- Intercompany elimination: AR/AP against sister entities (shared KRA PIN). ---
    let mut eliminations: Vec<serde_json::Value> = Vec::new();
    let mut elim_ar = Decimal::ZERO;
    let mut elim_ap = Decimal::ZERO;
    if req.eliminate_intercompany && chosen.len() > 1 {
        // KRA PINs of the consolidated entities (the "intercompany" set).
        let group_pins: Vec<String> = sqlx::query_scalar::<_, String>(
            "SELECT kra_pin FROM entity_settings WHERE entity_id = ANY($1) AND kra_pin IS NOT NULL AND kra_pin <> ''",
        )
        .bind(&selected)
        .fetch_all(state.engine.pool())
        .await
        .unwrap_or_default();

        if !group_pins.is_empty() {
            // Outstanding receivables from intercompany customers (open invoices).
            let ic_ar: Decimal = sqlx::query_scalar::<_, Decimal>(
                "SELECT COALESCE(SUM(i.balance_due), 0)
                 FROM invoices i JOIN customers c ON c.id = i.customer_id
                 WHERE i.entity_id = ANY($1) AND c.kra_pin = ANY($2)
                   AND i.status NOT IN ('draft','voided','cancelled') AND i.issue_date <= $3",
            )
            .bind(&selected)
            .bind(&group_pins)
            .bind(as_at)
            .fetch_one(state.engine.pool())
            .await
            .unwrap_or(Decimal::ZERO);

            // Outstanding payables to intercompany vendors (open bills).
            let ic_ap: Decimal = sqlx::query_scalar::<_, Decimal>(
                "SELECT COALESCE(SUM(b.balance_due), 0)
                 FROM bills b JOIN vendors v ON v.id = b.vendor_id
                 WHERE b.entity_id = ANY($1) AND v.kra_pin = ANY($2)
                   AND b.status NOT IN ('draft','voided','cancelled') AND b.issue_date <= $3",
            )
            .bind(&selected)
            .bind(&group_pins)
            .bind(as_at)
            .fetch_one(state.engine.pool())
            .await
            .unwrap_or(Decimal::ZERO);

            elim_ar = ic_ar;
            elim_ap = ic_ap;
            if ic_ar != Decimal::ZERO {
                eliminations.push(serde_json::json!({
                    "description": "Intercompany receivables (AR)", "amount": ic_ar,
                }));
            }
            if ic_ap != Decimal::ZERO {
                eliminations.push(serde_json::json!({
                    "description": "Intercompany payables (AP)", "amount": ic_ap,
                }));
            }
        }
    }

    // Apply eliminations to the AR/AP control accounts in the combined balances.
    let posting = state.engine.posting();
    if elim_ar != Decimal::ZERO {
        if let Some(e) = combined.get_mut(&posting.accounts_receivable) {
            e.0 -= elim_ar; // reduce the debit (asset) side
        }
    }
    if elim_ap != Decimal::ZERO {
        if let Some(e) = combined.get_mut(&posting.accounts_payable) {
            e.1 -= elim_ap; // reduce the credit (liability) side
        }
    }

    // Account names (first seen per code across the selected entities).
    let names: HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT DISTINCT ON (code) code, name FROM accounts WHERE entity_id = ANY($1) ORDER BY code",
    )
    .bind(&selected)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let mut total_debit = Decimal::ZERO;
    let mut total_credit = Decimal::ZERO;
    let mut codes: Vec<&String> = combined.keys().collect();
    codes.sort();
    let lines: Vec<_> = codes
        .into_iter()
        .map(|code| {
            let (d, c) = combined[code];
            let net = d - c;
            let (closing_debit, closing_credit) = if net >= Decimal::ZERO { (net, Decimal::ZERO) } else { (Decimal::ZERO, -net) };
            total_debit += closing_debit;
            total_credit += closing_credit;
            serde_json::json!({
                "account_code": code,
                "account_name": names.get(code).cloned().unwrap_or_default(),
                "closing_debit": closing_debit,
                "closing_credit": closing_credit,
            })
        })
        .collect();

    let difference = total_debit - total_credit;

    Ok(Json(serde_json::json!({
        "as_at": as_at,
        "presentation_currency": presentation_ccy,
        "entities": chosen.iter().map(|(_, name, cur, _)| serde_json::json!({ "name": name, "currency": cur })).collect::<Vec<_>>(),
        // Retained for backward-compat with the UI; now informational since
        // balances are translated to a single presentation currency.
        "mixed_currency": chosen.iter().map(|(_, _, cur, _)| cur).collect::<std::collections::HashSet<_>>().len() > 1,
        "untranslated": untranslated,
        "eliminations": eliminations,
        "lines": lines,
        "total_debits": total_debit,
        "total_credits": total_credit,
        "is_balanced": difference.abs() < Decimal::new(1, 2),
        "difference": difference,
    })))
}
