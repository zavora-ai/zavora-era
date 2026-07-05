//! Amos audit trail — a queryable record of session authentication and every
//! tool access (allowed/denied), per user and session.
//!
//! Uses a dedicated `amos_audit_events` table (the ERP owns its own
//! `audit_events`), so we don't collide with or depend on the ERP's audit
//! schema. Implements adk-auth's `AuditSink` so the scope layer can log through
//! the standard interface.

use adk_auth::{AuditEvent, AuditSink, AuthError};
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::warn;

pub struct AmosAuditSink {
    pool: PgPool,
    entity: String,
}

impl AmosAuditSink {
    /// Connect and ensure the table exists. Best-effort: any failure returns
    /// `None` so a missing audit trail never blocks Amos from starting.
    pub async fn connect(url: &str, entity: uuid::Uuid) -> Option<Self> {
        let pool = match PgPool::connect(url).await {
            Ok(p) => p,
            Err(e) => {
                warn!("audit: db connect failed ({e}); auditing disabled");
                return None;
            }
        };
        let ddl = r#"
            CREATE TABLE IF NOT EXISTS amos_audit_events (
                id          BIGSERIAL PRIMARY KEY,
                at          TIMESTAMPTZ NOT NULL DEFAULT now(),
                entity_id   TEXT NOT NULL,
                user_id     TEXT NOT NULL,
                session_id  TEXT,
                event_type  TEXT NOT NULL,
                resource    TEXT NOT NULL,
                outcome     TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_amos_audit_entity ON amos_audit_events (entity_id, at DESC);
            CREATE INDEX IF NOT EXISTS idx_amos_audit_user   ON amos_audit_events (user_id, at DESC);
        "#;
        if let Err(e) = sqlx::raw_sql(ddl).execute(&pool).await {
            warn!("audit: table setup failed ({e}); auditing disabled");
            return None;
        }
        Some(Self { pool, entity: entity.to_string() })
    }
}

#[async_trait]
impl AuditSink for AmosAuditSink {
    async fn log(&self, event: AuditEvent) -> Result<(), AuthError> {
        let event_type = format!("{:?}", event.event_type);
        let outcome = format!("{:?}", event.outcome);
        sqlx::query(
            "INSERT INTO amos_audit_events (entity_id, user_id, session_id, event_type, resource, outcome) \
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(&self.entity)
        .bind(&event.user)
        .bind(event.session_id.as_deref())
        .bind(event_type)
        .bind(&event.resource)
        .bind(outcome)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::AuditError(e.to_string()))?;
        Ok(())
    }
}
