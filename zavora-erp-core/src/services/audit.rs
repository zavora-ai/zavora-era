use crate::audit::*;
use crate::engine::ErpEngine;
use crate::error::ErpResult;
use uuid::Uuid;

/// Record an audit event. Best-effort: a logging failure must never fail the
/// business action, so callers can ignore the result. `actor` is stored as an
/// `AgentOrUserId` so the Audit view resolves it to the acting user.
pub async fn record_event(
    engine: &ErpEngine,
    entity_id: Uuid,
    event_type: &str,
    object_type: &str,
    object_id: Uuid,
    actor: &crate::types::AgentOrUserId,
    metadata: Option<serde_json::Value>,
) -> ErpResult<()> {
    let actor_json = serde_json::to_value(actor).unwrap_or_default();
    sqlx::query(
        r#"INSERT INTO audit_events (entity_id, event_type, object_type, object_id, actor, metadata, timestamp)
           VALUES ($1, $2, $3, $4, $5, $6, now())"#,
    )
    .bind(entity_id)
    .bind(event_type)
    .bind(object_type)
    .bind(object_id)
    .bind(actor_json)
    .bind(metadata)
    .execute(engine.pool())
    .await?;
    Ok(())
}

/// Query audit events.
pub async fn query_events(engine: &ErpEngine, query: AuditQuery) -> ErpResult<AuditEventPage> {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, AuditEventRow>(
        r#"SELECT * FROM audit_events 
           WHERE entity_id = $1 
           AND ($2::text IS NULL OR object_type = $2)
           AND ($3::uuid IS NULL OR object_id = $3)
           ORDER BY timestamp DESC
           LIMIT $4 OFFSET $5"#,
    )
    .bind(query.entity_id)
    .bind(query.object_type.as_deref())
    .bind(query.object_id)
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(engine.pool())
    .await?;

    let events: Vec<AuditEvent> = rows
        .iter()
        .map(|r| AuditEvent {
            id: r.id,
            entity_id: r.entity_id,
            event_type: serde_json::from_str(&format!("\"{}\"", r.event_type))
                .unwrap_or(AuditEventType::Created),
            object_type: r.object_type.clone(),
            object_id: r.object_id,
            actor: serde_json::from_value(r.actor.clone())
                .unwrap_or(crate::types::AgentOrUserId::Agent("unknown".to_string())),
            before: r.before_state.clone(),
            after: r.after_state.clone(),
            metadata: r.metadata.clone(),
            timestamp: r.timestamp,
        })
        .collect();

    Ok(AuditEventPage {
        total: events.len() as u64,
        events,
        limit,
        offset,
    })
}
