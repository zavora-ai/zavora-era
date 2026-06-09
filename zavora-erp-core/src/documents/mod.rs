use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{AgentOrUserId, LinkedType};

/// A document attachment stored in object storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub linked_type: LinkedType,
    pub linked_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub storage_key: String,
    pub size_bytes: u64,
    pub uploaded_by: AgentOrUserId,
    pub uploaded_at: DateTime<Utc>,
}

/// Database row for attachment.
#[derive(Debug, Clone, FromRow)]
pub struct AttachmentRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub linked_type: String,
    pub linked_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub storage_key: String,
    pub size_bytes: i64,
    pub uploaded_by: serde_json::Value,
    pub uploaded_at: DateTime<Utc>,
}

/// Request to upload an attachment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadAttachmentRequest {
    pub linked_type: LinkedType,
    pub linked_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Pre-signed URL for direct upload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresignedUploadUrl {
    pub upload_url: String,
    pub storage_key: String,
    pub expires_at: DateTime<Utc>,
}

/// Object storage client interface (trait for DI/testing).
#[async_trait::async_trait]
pub trait ObjectStorage: Send + Sync {
    async fn put_object(&self, key: &str, data: &[u8], content_type: &str)
        -> Result<(), crate::error::ErpError>;
    async fn get_object(&self, key: &str) -> Result<Vec<u8>, crate::error::ErpError>;
    async fn delete_object(&self, key: &str) -> Result<(), crate::error::ErpError>;
    async fn presign_upload(
        &self,
        key: &str,
        content_type: &str,
        expires_secs: u64,
    ) -> Result<String, crate::error::ErpError>;
    async fn presign_download(
        &self,
        key: &str,
        expires_secs: u64,
    ) -> Result<String, crate::error::ErpError>;
}
