//! Document attachments — link a source file (PDF/image) to any record (bill,
//! invoice, payment, …) for the audit trail.
//!
//! Files are stored inline as a base64 data-URL in `attachments.storage_key`,
//! mirroring how receipt captures hold their image. This keeps the feature
//! dependency-free (no object storage) for the document sizes we handle
//! (supplier invoices are tens of KB). The list endpoint returns metadata only;
//! the data-URL is fetched on demand so listings stay light.

use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::types::AgentOrUserId;

/// Attachment metadata (no file bytes).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AttachmentMeta {
    pub id: Uuid,
    pub linked_type: String,
    pub linked_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_at: chrono::DateTime<Utc>,
}

/// Store a file and link it to a record. Returns the new attachment's metadata.
pub async fn upload(
    engine: &ErpEngine,
    entity_id: Uuid,
    linked_type: &str,
    linked_id: Uuid,
    filename: &str,
    mime_type: &str,
    bytes: &[u8],
    uploaded_by: &AgentOrUserId,
) -> ErpResult<AttachmentMeta> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let id = Uuid::new_v4();
    let now = Utc::now();
    let data_url = format!("data:{};base64,{}", mime_type, STANDARD.encode(bytes));

    sqlx::query(
        r#"INSERT INTO attachments
           (id, entity_id, linked_type, linked_id, filename, mime_type, size_bytes, storage_key, uploaded_by, uploaded_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(linked_type)
    .bind(linked_id)
    .bind(filename)
    .bind(mime_type)
    .bind(bytes.len() as i64)
    .bind(&data_url)
    .bind(serde_json::to_value(uploaded_by).unwrap_or_default())
    .bind(now)
    .execute(engine.pool())
    .await?;

    Ok(AttachmentMeta {
        id,
        linked_type: linked_type.to_string(),
        linked_id,
        filename: filename.to_string(),
        mime_type: mime_type.to_string(),
        size_bytes: bytes.len() as i64,
        uploaded_at: now,
    })
}

/// List attachments linked to a record (metadata only — no file bytes).
pub async fn list(
    engine: &ErpEngine,
    entity_id: Uuid,
    linked_type: &str,
    linked_id: Uuid,
) -> ErpResult<Vec<AttachmentMeta>> {
    let rows = sqlx::query_as::<_, AttachmentMeta>(
        r#"SELECT id, linked_type, linked_id, filename, mime_type, size_bytes, uploaded_at
           FROM attachments WHERE entity_id = $1 AND linked_type = $2 AND linked_id = $3
           ORDER BY uploaded_at"#,
    )
    .bind(entity_id)
    .bind(linked_type)
    .bind(linked_id)
    .fetch_all(engine.pool())
    .await?;
    Ok(rows)
}

/// Fetch one attachment's `(filename, mime_type, data_url)` for download/preview.
pub async fn get_data(
    engine: &ErpEngine,
    entity_id: Uuid,
    id: Uuid,
) -> ErpResult<(String, String, String)> {
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT filename, mime_type, storage_key FROM attachments WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Attachment".to_string(), id })?;
    Ok(row)
}

/// Delete an attachment.
pub async fn delete(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<()> {
    sqlx::query("DELETE FROM attachments WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(entity_id)
        .execute(engine.pool())
        .await?;
    Ok(())
}
