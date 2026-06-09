use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::fx::*;

/// Upsert an exchange rate.
pub async fn upsert_rate(engine: &ErpEngine, req: UpsertRateRequest) -> ErpResult<ExchangeRate> {
    let id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO exchange_rates (id, entity_id, from_ccy, to_ccy, rate_date, rate_type, rate, source)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           ON CONFLICT (entity_id, from_ccy, to_ccy, rate_date, rate_type) 
           DO UPDATE SET rate = $7, source = $8"#,
    )
    .bind(id)
    .bind(engine.entity_id())
    .bind(&req.from_ccy)
    .bind(&req.to_ccy)
    .bind(req.rate_date)
    .bind(serde_json::to_string(&req.rate_type).unwrap_or_default())
    .bind(req.rate)
    .bind(&req.source)
    .execute(engine.pool())
    .await?;

    Ok(ExchangeRate {
        id,
        entity_id: engine.entity_id(),
        from_ccy: req.from_ccy,
        to_ccy: req.to_ccy,
        rate_date: req.rate_date,
        rate_type: req.rate_type,
        rate: req.rate,
        source: req.source,
    })
}
