use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::notifications::*;

/// Queue a notification for delivery.
pub async fn send_notification(
    _engine: &ErpEngine,
    _req: SendNotificationRequest,
) -> ErpResult<()> {
    // Queue to Redis for async delivery by notification worker
    // TODO: XADD to notification stream
    Ok(())
}
