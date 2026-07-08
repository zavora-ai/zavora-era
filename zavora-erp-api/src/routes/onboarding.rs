use axum::{extract::State, Json};
use rust_decimal::Decimal;
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;
use zavora_erp_core::AgentOrUserId;
use zavora_erp_core::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};

#[derive(serde::Deserialize)]
pub struct OpeningBalanceLine {
    pub account_code: String,
    #[serde(default)]
    pub debit: Option<Decimal>,
    #[serde(default)]
    pub credit: Option<Decimal>,
}

#[derive(serde::Deserialize)]
pub struct OpeningBalancesRequest {
    pub as_of_date: chrono::NaiveDate,
    pub lines: Vec<OpeningBalanceLine>,
}

/// POST /opening-balances — post an opening trial balance as an OpeningBalance
/// journal. An opening TB must balance by definition (the user includes their
/// opening equity/retained-earnings line), so we reject an unbalanced entry and
/// report the difference — no hidden plug account.
pub async fn post_opening_balances(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<OpeningBalancesRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let base_ccy = state.engine.config().base_currency.clone();

    let mut lines: Vec<CreateJournalLineRequest> = Vec::new();
    let mut total_debit = Decimal::ZERO;
    let mut total_credit = Decimal::ZERO;
    for l in &req.lines {
        let debit = l.debit.unwrap_or(Decimal::ZERO);
        let credit = l.credit.unwrap_or(Decimal::ZERO);
        if debit.is_zero() && credit.is_zero() {
            continue;
        }
        total_debit += debit;
        total_credit += credit;
        lines.push(CreateJournalLineRequest {
            account_code: l.account_code.clone(),
            debit: if debit > Decimal::ZERO { Some(debit) } else { None },
            credit: if credit > Decimal::ZERO { Some(credit) } else { None },
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some("Opening balance".to_string()),
            dimensions: None,
        });
    }

    if lines.len() < 2 {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: "Enter opening balances for at least two accounts".to_string(),
        }));
    }

    let difference = total_debit - total_credit;
    if difference.abs() >= Decimal::new(1, 2) {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: format!(
                "Opening trial balance does not balance — debits {} vs credits {} (difference {}). Add the missing equity/retained-earnings line.",
                total_debit, total_credit, difference
            ),
        }));
    }

    let entry_req = CreateJournalEntryRequest {
        date: req.as_of_date,
        source: JournalSource::OpeningBalance,
        source_id: None,
        reference: "OPENING".to_string(),
        description: "Opening balances".to_string(),
        lines,
        post_immediately: true,
    };

    let period = zavora_erp_core::services::periods::period_for_date(&state.engine, ctx.entity_id, req.as_of_date)
        .await
        .map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    let entry = zavora_erp_core::services::journal::create_and_post(&state.engine, ctx.entity_id, entry_req, period.id, actor)
        .await
        .map_err(err_response)?;

    Ok(Json(serde_json::json!({ "journal_entry_id": entry.id, "number": entry.number })))
}
