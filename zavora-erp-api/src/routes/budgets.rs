use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;

/// GET /budgets — all budget entries for the entity, with account + period labels.
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, String, Option<String>, rust_decimal::Decimal, String, chrono::NaiveDate, chrono::NaiveDate)>(
        r#"SELECT be.id, be.period_id, be.account_code, a.name AS account_name, be.amount,
                  fp.name AS period_name, fp.start_date, fp.end_date
           FROM budget_entries be
           JOIN fiscal_periods fp ON fp.id = be.period_id
           LEFT JOIN accounts a ON a.code = be.account_code AND a.entity_id = be.entity_id
           WHERE be.entity_id = $1
           ORDER BY fp.start_date, be.account_code"#,
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;

    match rows {
        Ok(r) => {
            let items: Vec<_> = r
                .into_iter()
                .map(|(id, period_id, account_code, account_name, amount, period_name, start_date, end_date)| {
                    serde_json::json!({
                        "id": id,
                        "period_id": period_id,
                        "account_code": account_code,
                        "account_name": account_name,
                        "amount": amount,
                        "period_name": period_name,
                        "start_date": start_date,
                        "end_date": end_date,
                    })
                })
                .collect();
            Ok(Json(serde_json::to_value(items).unwrap_or_default()))
        }
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

#[derive(serde::Deserialize)]
pub struct SetBudgetRequest {
    pub period_id: uuid::Uuid,
    pub account_code: String,
    pub amount: rust_decimal::Decimal,
}

/// PUT /budgets — set (upsert) the budget for an account in a period.
pub async fn set(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetBudgetRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let res = sqlx::query(
        r#"INSERT INTO budget_entries (entity_id, period_id, account_code, amount)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (entity_id, period_id, account_code)
           DO UPDATE SET amount = EXCLUDED.amount, updated_at = NOW()"#,
    )
    .bind(ctx.entity_id)
    .bind(req.period_id)
    .bind(&req.account_code)
    .bind(req.amount)
    .execute(state.engine.pool())
    .await;

    match res {
        Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}
