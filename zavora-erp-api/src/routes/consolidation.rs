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
}

/// POST /consolidation/trial-balance — consolidated trial balance across the
/// selected (authorized) entities as at a date. Functional amounts are summed
/// per account; FX translation + intercompany elimination are not yet applied,
/// so a mixed-currency selection is flagged.
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

    let movements = sqlx::query_as::<_, (String, Decimal, Decimal)>(
        "SELECT account_code,
                COALESCE(SUM(functional_debit), 0)  AS debit,
                COALESCE(SUM(functional_credit), 0) AS credit
         FROM journal_lines
         WHERE entity_id = ANY($1) AND entry_date <= $2
         GROUP BY account_code
         ORDER BY account_code",
    )
    .bind(&selected)
    .bind(as_at)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default();

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
    let lines: Vec<_> = movements
        .into_iter()
        .map(|(code, d, c)| {
            let net = d - c;
            let (closing_debit, closing_credit) = if net >= Decimal::ZERO { (net, Decimal::ZERO) } else { (Decimal::ZERO, -net) };
            total_debit += closing_debit;
            total_credit += closing_credit;
            serde_json::json!({
                "account_code": code,
                "account_name": names.get(&code).cloned().unwrap_or_default(),
                "closing_debit": closing_debit,
                "closing_credit": closing_credit,
            })
        })
        .collect();

    let chosen: Vec<_> = authorized.iter().filter(|(id, ..)| selected.contains(id)).collect();
    let currencies: std::collections::HashSet<&String> = chosen.iter().map(|(_, _, cur, _)| cur).collect();
    let difference = total_debit - total_credit;

    Ok(Json(serde_json::json!({
        "as_at": as_at,
        "entities": chosen.iter().map(|(_, name, cur, _)| serde_json::json!({ "name": name, "currency": cur })).collect::<Vec<_>>(),
        "mixed_currency": currencies.len() > 1,
        "lines": lines,
        "total_debits": total_debit,
        "total_credits": total_credit,
        "is_balanced": difference.abs() < Decimal::new(1, 2),
        "difference": difference,
    })))
}
