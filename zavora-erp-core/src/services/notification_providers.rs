//! Per-tenant notification provider configuration.
//!
//! Each tenant can configure its own delivery providers (SMTP / Africa's
//! Talking / Twilio). Non-secret fields live in `settings` (JSONB); the single
//! secret per channel (SMTP password, SMS api key, Twilio auth token) is stored
//! AES-256-GCM encrypted (`crate::crypto`) and is **never** returned to clients.
//!
//! Three views of a provider:
//!   * [`get_masked`] — for the admin UI: non-secret settings + `has_secret`.
//!   * [`resolve`] — for the worker: a fully-decrypted [`ResolvedProvider`].
//!   * [`upsert`] — write path: encrypts a newly-supplied secret, or preserves
//!     the existing one when the secret field is left blank (write-only UI).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ErpError, ErpResult};

/// Channels that support per-tenant provider configuration.
pub const CONFIGURABLE_CHANNELS: [&str; 3] = ["email", "sms", "whatsapp"];

/// A provider as shown to the admin UI — non-secret settings plus whether a
/// secret is on file. Never carries the plaintext secret.
#[derive(Debug, Clone, Serialize)]
pub struct MaskedProvider {
    pub channel: String,
    pub enabled: bool,
    /// Non-secret config keys (host, port, from, sender_id, account_sid, …).
    pub settings: serde_json::Value,
    /// `true` when an encrypted secret is stored for this channel.
    pub has_secret: bool,
}

/// A fully-resolved provider for the worker: non-secret settings + decrypted
/// secret (when present).
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    pub channel: String,
    pub enabled: bool,
    pub settings: serde_json::Value,
    pub secret: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ProviderRow {
    channel: String,
    enabled: bool,
    settings: serde_json::Value,
    secret_enc: Option<Vec<u8>>,
    secret_nonce: Option<Vec<u8>>,
}

/// List all configured providers for a tenant, masked for display. Channels the
/// tenant has not configured are simply absent (the UI shows empty defaults).
pub async fn get_masked(pool: &sqlx::PgPool, entity_id: Uuid) -> ErpResult<Vec<MaskedProvider>> {
    let rows = sqlx::query_as::<_, ProviderRow>(
        "SELECT channel, enabled, settings, secret_enc, secret_nonce \
         FROM notification_providers WHERE entity_id = $1 ORDER BY channel",
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await
    .map_err(ErpError::Database)?;

    Ok(rows
        .into_iter()
        .map(|r| MaskedProvider {
            channel: r.channel,
            enabled: r.enabled,
            settings: r.settings,
            has_secret: r.secret_enc.is_some() && r.secret_nonce.is_some(),
        })
        .collect())
}

/// Resolve a single channel's provider for the worker, decrypting the secret.
/// Returns `None` when the tenant has not configured (or has disabled) that
/// channel — the caller then falls back to the deployment/env provider.
pub async fn resolve(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    channel: &str,
) -> ErpResult<Option<ResolvedProvider>> {
    let row = sqlx::query_as::<_, ProviderRow>(
        "SELECT channel, enabled, settings, secret_enc, secret_nonce \
         FROM notification_providers WHERE entity_id = $1 AND channel = $2",
    )
    .bind(entity_id)
    .bind(channel)
    .fetch_optional(pool)
    .await
    .map_err(ErpError::Database)?;

    let Some(row) = row else { return Ok(None) };
    if !row.enabled {
        return Ok(None);
    }

    let secret = match (row.secret_enc, row.secret_nonce) {
        (Some(ct), Some(nonce)) => Some(crate::crypto::decrypt(&ct, &nonce)?),
        _ => None,
    };

    Ok(Some(ResolvedProvider {
        channel: row.channel,
        enabled: row.enabled,
        settings: row.settings,
        secret,
    }))
}

/// Input for upserting a provider. `secret` is `None`/empty to keep the existing
/// stored secret (write-only UI), or `Some(new)` to replace it.
#[derive(Debug, Clone, Deserialize)]
pub struct UpsertProvider {
    pub channel: String,
    pub enabled: bool,
    pub settings: serde_json::Value,
    #[serde(default)]
    pub secret: Option<String>,
}

/// Upsert one channel's provider config. Encrypts a newly-supplied secret;
/// preserves the existing ciphertext when `secret` is blank. Validates channel.
pub async fn upsert(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    input: UpsertProvider,
    updated_by: Uuid,
) -> ErpResult<MaskedProvider> {
    if !CONFIGURABLE_CHANNELS.contains(&input.channel.as_str()) {
        return Err(ErpError::ValidationFailed {
            message: format!("'{}' is not a configurable provider channel", input.channel),
        });
    }

    // Determine the secret columns: a non-blank secret is (re)encrypted; a blank
    // one preserves whatever is already stored.
    let new_secret = input.secret.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let (secret_enc, secret_nonce): (Option<Vec<u8>>, Option<Vec<u8>>) = match new_secret {
        Some(plain) => {
            if !crate::crypto::encryption_available() {
                return Err(ErpError::ValidationFailed {
                    message: "Cannot store a provider secret: NOTIF_ENC_KEY is not configured on the server.".to_string(),
                });
            }
            let (ct, nonce) = crate::crypto::encrypt(plain)?;
            (Some(ct), Some(nonce))
        }
        None => {
            // Preserve existing secret columns (read them back).
            let existing = sqlx::query_as::<_, (Option<Vec<u8>>, Option<Vec<u8>>)>(
                "SELECT secret_enc, secret_nonce FROM notification_providers WHERE entity_id = $1 AND channel = $2",
            )
            .bind(entity_id)
            .bind(&input.channel)
            .fetch_optional(pool)
            .await
            .map_err(ErpError::Database)?;
            existing.unwrap_or((None, None))
        }
    };

    sqlx::query(
        r#"INSERT INTO notification_providers
               (entity_id, channel, enabled, settings, secret_enc, secret_nonce, updated_at, updated_by)
           VALUES ($1, $2, $3, $4, $5, $6, now(), $7)
           ON CONFLICT (entity_id, channel) DO UPDATE SET
               enabled = EXCLUDED.enabled,
               settings = EXCLUDED.settings,
               secret_enc = EXCLUDED.secret_enc,
               secret_nonce = EXCLUDED.secret_nonce,
               updated_at = now(),
               updated_by = EXCLUDED.updated_by"#,
    )
    .bind(entity_id)
    .bind(&input.channel)
    .bind(input.enabled)
    .bind(&input.settings)
    .bind(&secret_enc)
    .bind(&secret_nonce)
    .bind(updated_by)
    .execute(pool)
    .await
    .map_err(ErpError::Database)?;

    Ok(MaskedProvider {
        channel: input.channel,
        enabled: input.enabled,
        settings: input.settings,
        has_secret: secret_enc.is_some(),
    })
}
