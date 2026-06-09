use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::notifications::*;

/// Queue a notification for delivery via Redis stream.
pub async fn send_notification(
    engine: &ErpEngine,
    req: SendNotificationRequest,
) -> ErpResult<()> {
    let mut redis_conn = engine.redis_conn().await;
    let payload = serde_json::to_string(&req)?;
    let stream_key = format!("erp:notifications:{}", engine.entity_id());

    redis::cmd("XADD")
        .arg(&stream_key)
        .arg("*")
        .arg("data")
        .arg(&payload)
        .query_async::<()>(&mut redis_conn)
        .await
        .map_err(|e| crate::error::ErpError::Redis(e))?;

    Ok(())
}
