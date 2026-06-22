use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::notifications::*;
use uuid::Uuid;

/// Queue a notification for delivery via the global Redis stream.
///
/// Messages are written to `erp:notifications` (a single global stream) with
/// `entity_id` included in the payload so the background worker can fan out
/// per-tenant without requiring per-entity streams.
pub async fn send_notification(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: SendNotificationRequest,
) -> ErpResult<()> {
    let mut redis_conn = engine.redis_conn().await;
    let payload = serde_json::to_string(&req)?;

    redis::cmd("XADD")
        .arg("erp:notifications")
        .arg("*")
        .arg("entity_id")
        .arg(entity_id.to_string())
        .arg("data")
        .arg(&payload)
        .query_async::<()>(&mut redis_conn)
        .await
        .map_err(|e| crate::error::ErpError::Redis(e))?;

    Ok(())
}
