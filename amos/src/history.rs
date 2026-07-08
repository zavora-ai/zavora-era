//! Session history — the durable record of past conversations.
//!
//! Transcripts were previously distilled into memory and then discarded; for
//! an accountant, the dialogue in which the owner approved a posting is
//! compliance material. Each closed session is stored whole (best-effort,
//! mirroring the audit sink: a missing store never blocks Amos).

use serde::Serialize;
use sqlx::{PgPool, Row};
use tracing::warn;

/// Listing row for the "Past sessions" panel: metadata + a short preview,
/// never the full transcript.
#[derive(Debug, Serialize)]
pub struct SessionMeta {
    pub id: uuid::Uuid,
    pub user_id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: chrono::DateTime<chrono::Utc>,
    pub preview: String,
}

pub struct SessionHistory {
    pool: PgPool,
    entity: String,
}

impl SessionHistory {
    /// Connect and ensure the table exists. Best-effort: any failure returns
    /// `None` so a missing history store never blocks Amos from starting.
    pub async fn connect(url: &str, entity: uuid::Uuid) -> Option<Self> {
        let pool = match PgPool::connect(url).await {
            Ok(p) => p,
            Err(e) => {
                warn!("history: db connect failed ({e}); session history disabled");
                return None;
            }
        };
        let ddl = r#"
            CREATE TABLE IF NOT EXISTS amos_sessions (
                id          UUID PRIMARY KEY,
                entity_id   TEXT NOT NULL,
                user_id     TEXT NOT NULL,
                started_at  TIMESTAMPTZ NOT NULL,
                ended_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
                transcript  TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_amos_sessions_entity ON amos_sessions (entity_id, started_at DESC);
        "#;
        if let Err(e) = sqlx::raw_sql(ddl).execute(&pool).await {
            warn!("history: table setup failed ({e}); session history disabled");
            return None;
        }
        Some(Self { pool, entity: entity.to_string() })
    }

    /// Persist a closed session. Trivial transcripts (empty connects) are
    /// skipped — same bar as the memory distillation.
    pub async fn save(
        &self,
        session_id: uuid::Uuid,
        user_id: uuid::Uuid,
        started_at: chrono::DateTime<chrono::Utc>,
        transcript: &str,
    ) {
        if transcript.trim().len() < 40 {
            return;
        }
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO amos_sessions (id, entity_id, user_id, started_at, transcript)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(session_id)
        .bind(&self.entity)
        .bind(user_id.to_string())
        .bind(started_at)
        .bind(transcript)
        .execute(&self.pool)
        .await
        {
            warn!("history: failed to save session {session_id}: {e}");
        }
    }

    /// Recent sessions for this entity, newest first.
    pub async fn list(&self, limit: i64) -> Vec<SessionMeta> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, started_at, ended_at, LEFT(transcript, 200) AS preview
            FROM amos_sessions
            WHERE entity_id = $1
            ORDER BY started_at DESC
            LIMIT $2
            "#,
        )
        .bind(&self.entity)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        rows.into_iter()
            .map(|r| SessionMeta {
                id: r.get("id"),
                user_id: r.get("user_id"),
                started_at: r.get("started_at"),
                ended_at: r.get("ended_at"),
                preview: clean_preview(r.get::<String, _>("preview")),
            })
            .collect()
    }

    /// One session's full transcript (entity-scoped).
    pub async fn transcript(&self, id: uuid::Uuid) -> Option<String> {
        sqlx::query("SELECT transcript FROM amos_sessions WHERE id = $1 AND entity_id = $2")
            .bind(id)
            .bind(&self.entity)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .map(|r| r.get("transcript"))
    }
}

/// First meaningful line of a transcript, without the speaker tag.
fn clean_preview(raw: String) -> String {
    raw.lines()
        .map(|l| l.trim().trim_start_matches("[owner]:").trim_start_matches("[amos]:").trim())
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .chars()
        .take(120)
        .collect()
}
