//! Background worker that consumes the `erp:notifications` Redis stream and
//! delivers notifications via the appropriate channel (Email, InApp, SMS,
//! WhatsApp). Failed deliveries are retried up to 3 times with a 2-second
//! backoff between attempts.

use chrono::Utc;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;
use uuid::Uuid;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

use crate::notifications::SendNotificationRequest;
use crate::services::messaging::{SmsProvider, WhatsAppProvider};
use crate::types::Channel;

const STREAM_KEY: &str = "erp:notifications";
const GROUP_NAME: &str = "notification-workers";
const CONSUMER_NAME: &str = "worker-1";
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_SECS: u64 = 2;

/// Delivery transports built once at worker startup and shared across messages.
/// Each is `None` when its channel is not configured for this deployment, in
/// which case delivery on that channel reports a clear "not configured" error.
struct Transports {
    smtp: Option<AsyncSmtpTransport<Tokio1Executor>>,
    sms: Option<SmsProvider>,
    whatsapp: Option<WhatsAppProvider>,
}

/// SMTP configuration loaded from environment variables.
struct SmtpConfig {
    host: String,
    port: u16,
    user: Option<String>,
    pass: Option<String>,
    /// From-address override (per-tenant); falls back to SMTP_FROM/default.
    from: Option<String>,
}

impl SmtpConfig {
    fn from_env() -> Option<Self> {
        let host = std::env::var("SMTP_HOST").ok()?;
        if host.is_empty() {
            return None;
        }
        Some(Self {
            host,
            port: std::env::var("SMTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(587),
            user: std::env::var("SMTP_USER").ok(),
            pass: std::env::var("SMTP_PASS").ok(),
            from: std::env::var("SMTP_FROM").ok().filter(|v| !v.trim().is_empty()),
        })
    }

    /// Build from a tenant's resolved email provider settings + decrypted secret.
    /// Expects `settings.host`, optional `settings.port`, `settings.user`,
    /// `settings.from`; the secret is the SMTP password. Returns `None` if no host.
    fn from_provider(p: &crate::services::notification_providers::ResolvedProvider) -> Option<Self> {
        let s = &p.settings;
        let host = s.get("host").and_then(|v| v.as_str()).map(str::trim).filter(|h| !h.is_empty())?;
        let port = s.get("port").and_then(|v| v.as_u64()).map(|v| v as u16).unwrap_or(587);
        let user = s.get("user").and_then(|v| v.as_str()).map(String::from).filter(|v| !v.is_empty());
        let from = s.get("from").and_then(|v| v.as_str()).map(String::from).filter(|v| !v.is_empty());
        Some(Self {
            host: host.to_string(),
            port,
            user,
            pass: p.secret.clone(),
            from,
        })
    }
}

/// Build an async SMTP transport from a config. Returns `None` on builder error.
fn smtp_transport_from(config: &SmtpConfig) -> Option<AsyncSmtpTransport<Tokio1Executor>> {
    let builder = match AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("Failed to create SMTP transport for '{}': {e}", config.host);
            return None;
        }
    };
    let mut transport = builder.port(config.port);
    if let (Some(user), Some(pass)) = (config.user.clone(), config.pass.clone()) {
        transport = transport.credentials(Credentials::new(user, pass));
    }
    Some(transport.build())
}

/// Start the notification worker. This function runs forever, consuming
/// messages from the Redis stream and delivering notifications.
///
/// It should be spawned as a background tokio task.
pub async fn run(mut redis_conn: redis::aio::MultiplexedConnection, pool: PgPool) {
    tracing::info!("Notification worker starting");

    // Ensure the consumer group exists (MKSTREAM creates the stream if needed).
    let result: Result<(), redis::RedisError> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(STREAM_KEY)
        .arg(GROUP_NAME)
        .arg("0")
        .arg("MKSTREAM")
        .query_async(&mut redis_conn)
        .await;

    match result {
        Ok(()) => tracing::info!("Created consumer group '{GROUP_NAME}' on '{STREAM_KEY}'"),
        Err(e) if e.to_string().contains("BUSYGROUP") => {
            tracing::debug!("Consumer group '{GROUP_NAME}' already exists");
        }
        Err(e) => {
            tracing::error!("Failed to create consumer group: {e}");
            return;
        }
    }

    // Build delivery transports once (reused across messages).
    let smtp = build_smtp_transport();
    let sms = SmsProvider::from_env();
    let whatsapp = WhatsAppProvider::from_env();
    tracing::info!(
        smtp = smtp.is_some(),
        sms = sms.is_some(),
        whatsapp = whatsapp.is_some(),
        "Notification delivery channels configured"
    );
    let transports = Transports { smtp, sms, whatsapp };

    tracing::info!("Notification worker ready, listening on '{STREAM_KEY}'");

    loop {
        // XREADGROUP GROUP notification-workers worker-1 COUNT 10 BLOCK 5000 STREAMS erp:notifications >
        let messages: redis::Value = match redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(GROUP_NAME)
            .arg(CONSUMER_NAME)
            .arg("COUNT")
            .arg(10)
            .arg("BLOCK")
            .arg(5000)
            .arg("STREAMS")
            .arg(STREAM_KEY)
            .arg(">")
            .query_async(&mut redis_conn)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("XREADGROUP error: {e}");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        let entries = parse_stream_entries(&messages);
        if entries.is_empty() {
            continue;
        }

        for (message_id, entity_id, request) in &entries {
            process_message(&pool, &transports, entity_id, request).await;

            // ACK the message regardless of per-recipient outcome (retries are
            // handled within process_message).
            let ack_result: Result<(), redis::RedisError> = redis::cmd("XACK")
                .arg(STREAM_KEY)
                .arg(GROUP_NAME)
                .arg(message_id.as_str())
                .query_async(&mut redis_conn)
                .await;

            if let Err(e) = ack_result {
                tracing::error!("Failed to ACK message {message_id}: {e}");
            }
        }
    }
}

/// Process a single notification message: for each (channel, recipient) pair,
/// insert a row, attempt delivery with retries, and update status.
async fn process_message(
    pool: &PgPool,
    transports: &Transports,
    entity_id: &Uuid,
    req: &SendNotificationRequest,
) {
    let event_type_str = serde_json::to_string(&req.event_type)
        .unwrap_or_else(|_| "unknown".to_string());

    for channel in &req.channels {
        for recipient in &req.recipients {
            let notif_id = Uuid::new_v4();
            let now = Utc::now();
            let channel_str = channel_to_str(channel);

            // Insert the notification row (status = 'queued').
            let insert_result = sqlx::query(
                r#"INSERT INTO notifications
                   (id, entity_id, event_type, channel, recipient, subject, body,
                    related_type, related_id, status, scheduled_at, created_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'queued', $10, $11)"#,
            )
            .bind(notif_id)
            .bind(entity_id)
            .bind(&event_type_str)
            .bind(channel_str)
            .bind(recipient)
            .bind(req.subject.as_deref())
            .bind(&req.body)
            .bind(req.related_type.as_deref())
            .bind(req.related_id)
            .bind(req.schedule_at)
            .bind(now)
            .execute(pool)
            .await;

            if let Err(e) = insert_result {
                tracing::error!(
                    "Failed to insert notification row for {recipient} via {channel_str}: {e}"
                );
                continue;
            }

            // Attempt delivery with retries.
            let mut last_error: Option<String> = None;
            let mut delivered = false;

            for attempt in 1..=MAX_RETRIES {
                match deliver(pool, transports, entity_id, channel, recipient, req).await {
                    Ok(()) => {
                        delivered = true;
                        break;
                    }
                    Err(err) => {
                        last_error = Some(err.clone());
                        if attempt < MAX_RETRIES {
                            tracing::warn!(
                                "Delivery attempt {attempt}/{MAX_RETRIES} failed for \
                                 {recipient} via {channel_str}: {err}. Retrying..."
                            );
                            tokio::time::sleep(tokio::time::Duration::from_secs(
                                RETRY_DELAY_SECS,
                            ))
                            .await;
                        } else {
                            tracing::error!(
                                "All {MAX_RETRIES} delivery attempts failed for \
                                 {recipient} via {channel_str}: {err}"
                            );
                        }
                    }
                }
            }

            // Update notification status.
            if delivered {
                let status = match channel {
                    Channel::InApp => "delivered",
                    _ => "sent",
                };
                let _ = sqlx::query(
                    "UPDATE notifications SET status = $1, sent_at = $2 WHERE id = $3",
                )
                .bind(status)
                .bind(Utc::now())
                .bind(notif_id)
                .execute(pool)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to update notification {notif_id} to {status}: {e}");
                });
            } else {
                let error_msg = last_error.unwrap_or_else(|| "unknown error".to_string());
                let _ = sqlx::query(
                    "UPDATE notifications SET status = 'failed', error = $1 WHERE id = $2",
                )
                .bind(&error_msg)
                .bind(notif_id)
                .execute(pool)
                .await
                .map_err(|e| {
                    tracing::error!(
                        "Failed to update notification {notif_id} to failed: {e}"
                    );
                });
            }
        }
    }
}

/// Attempt delivery for a single (channel, recipient) pair.
///
/// Resolves the **tenant's own** provider first (per-message), falling back to
/// the deployment/env transports when the tenant hasn't configured (or has
/// disabled) that channel. Returns `Ok(())` on success or `Err(msg)` on failure.
async fn deliver(
    pool: &PgPool,
    transports: &Transports,
    entity_id: &Uuid,
    channel: &Channel,
    recipient: &str,
    req: &SendNotificationRequest,
) -> Result<(), String> {
    use crate::services::notification_providers as np;

    // Resolve a per-tenant provider for the external channels.
    let channel_key = match channel {
        Channel::Email => "email",
        Channel::Sms => "sms",
        Channel::WhatsApp => "whatsapp",
        Channel::InApp => "in_app",
    };
    let tenant = match channel {
        Channel::InApp => None,
        _ => np::resolve(pool, *entity_id, channel_key).await.unwrap_or(None),
    };

    match channel {
        Channel::Email => {
            // Tenant SMTP if configured, else the env transport.
            if let Some(ref p) = tenant {
                if let Some(cfg) = SmtpConfig::from_provider(p) {
                    let from = cfg.from.clone();
                    if let Some(transport) = smtp_transport_from(&cfg) {
                        return deliver_email_with(&transport, from.as_deref(), recipient, req).await;
                    }
                }
            }
            let from = std::env::var("SMTP_FROM").ok();
            match &transports.smtp {
                Some(t) => deliver_email_with(t, from.as_deref(), recipient, req).await,
                None => {
                    tracing::warn!("SMTP not configured (tenant or deployment) for {recipient}");
                    Err("SMTP not configured".to_string())
                }
            }
        }
        Channel::InApp => Ok(()),
        Channel::Sms => {
            let body = crate::services::messaging::html_to_text(&req.body);
            // Tenant Africa's Talking creds if configured, else env provider.
            let tenant_provider = tenant.as_ref().and_then(|p| {
                crate::services::messaging::SmsProvider::from_parts(
                    p.settings.get("username").and_then(|v| v.as_str()).map(String::from),
                    p.secret.clone(),
                    p.settings.get("sender_id").and_then(|v| v.as_str()).map(String::from),
                    p.settings.get("base_url").and_then(|v| v.as_str()).map(String::from),
                )
            });
            if let Some(provider) = tenant_provider {
                return provider.send(recipient, &body).await;
            }
            match &transports.sms {
                Some(provider) => provider.send(recipient, &body).await,
                None => {
                    tracing::warn!("SMS delivery not configured for {recipient}");
                    Err("SMS channel not configured".to_string())
                }
            }
        }
        Channel::WhatsApp => {
            let body = crate::services::messaging::html_to_text(&req.body);
            let tenant_provider = tenant.as_ref().and_then(|p| {
                crate::services::messaging::WhatsAppProvider::from_parts(
                    p.settings.get("account_sid").and_then(|v| v.as_str()).map(String::from),
                    p.secret.clone(),
                    p.settings.get("from").and_then(|v| v.as_str()).map(String::from),
                    p.settings.get("base_url").and_then(|v| v.as_str()).map(String::from),
                )
            });
            if let Some(provider) = tenant_provider {
                return provider.send(recipient, &body).await;
            }
            match &transports.whatsapp {
                Some(provider) => provider.send(recipient, &body).await,
                None => {
                    tracing::warn!("WhatsApp delivery not configured for {recipient}");
                    Err("WhatsApp channel not configured".to_string())
                }
            }
        }
    }
}

/// Send an email through a resolved SMTP transport, with an optional
/// from-address override (per-tenant); falls back to `SMTP_FROM` then a default.
async fn deliver_email_with(
    transport: &AsyncSmtpTransport<Tokio1Executor>,
    from_override: Option<&str>,
    recipient: &str,
    req: &SendNotificationRequest,
) -> Result<(), String> {
    let from_addr = from_override
        .map(|s| s.to_string())
        .or_else(|| std::env::var("SMTP_FROM").ok())
        .unwrap_or_else(|| "noreply@zavora.app".to_string());

    let subject = req
        .subject
        .as_deref()
        .unwrap_or("Notification from Zavora ERP");

    let builder = Message::builder()
        .from(
            from_addr
                .parse()
                .map_err(|e| format!("Invalid from address: {e}"))?,
        )
        .to(recipient
            .parse()
            .map_err(|e| format!("Invalid recipient address '{recipient}': {e}"))?)
        .subject(subject);

    // With attachments, build a mixed multipart (HTML body + each file). Without,
    // keep the simple single-part HTML message.
    let email = if req.attachments.is_empty() {
        builder
            .header(ContentType::TEXT_HTML)
            .body(req.body.clone())
            .map_err(|e| format!("Failed to build email: {e}"))?
    } else {
        let mut mp = MultiPart::mixed().singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(req.body.clone()),
        );
        for att in &req.attachments {
            let bytes = B64
                .decode(att.content_base64.as_bytes())
                .map_err(|e| format!("Invalid base64 attachment '{}': {e}", att.filename))?;
            let ct = ContentType::parse(&att.mime_type)
                .unwrap_or(ContentType::parse("application/octet-stream").unwrap());
            mp = mp.singlepart(
                Attachment::new(att.filename.clone()).body(bytes, ct),
            );
        }
        builder
            .multipart(mp)
            .map_err(|e| format!("Failed to build email: {e}"))?
    };

    transport
        .send(email)
        .await
        .map_err(|e| format!("SMTP send failed: {e}"))?;

    tracing::info!("Email delivered to {recipient}");
    Ok(())
}

/// Build the async SMTP transport from environment configuration.
/// Returns `None` if SMTP is not configured (SMTP_HOST not set).
fn build_smtp_transport() -> Option<AsyncSmtpTransport<Tokio1Executor>> {
    let config = SmtpConfig::from_env()?;
    smtp_transport_from(&config)
}

/// Parse Redis XREADGROUP response into a list of (message_id, entity_id, request).
///
/// The Redis response structure for XREADGROUP is:
/// ```text
/// [
///   ["stream_name", [
///     ["message_id", ["field1", "value1", "field2", "value2", ...]],
///     ...
///   ]],
///   ...
/// ]
/// ```
fn parse_stream_entries(
    value: &redis::Value,
) -> Vec<(String, Uuid, SendNotificationRequest)> {
    let mut results = Vec::new();

    // Top level: array of streams
    let streams = match value {
        redis::Value::Array(arr) => arr,
        redis::Value::Nil => return results,
        _ => return results,
    };

    for stream in streams {
        // Each stream: [stream_name, entries_array]
        let stream_parts = match stream {
            redis::Value::Array(arr) if arr.len() >= 2 => arr,
            _ => continue,
        };

        // entries_array
        let entries = match &stream_parts[1] {
            redis::Value::Array(arr) => arr,
            _ => continue,
        };

        for entry in entries {
            // Each entry: [message_id, fields_array]
            let entry_parts = match entry {
                redis::Value::Array(arr) if arr.len() >= 2 => arr,
                _ => continue,
            };

            let message_id = match &entry_parts[0] {
                redis::Value::BulkString(bytes) => {
                    String::from_utf8_lossy(bytes).to_string()
                }
                _ => continue,
            };

            // fields_array: [key, value, key, value, ...]
            let fields = match &entry_parts[1] {
                redis::Value::Array(arr) => arr,
                _ => continue,
            };

            let mut entity_id_str: Option<String> = None;
            let mut data_str: Option<String> = None;

            let mut i = 0;
            while i + 1 < fields.len() {
                let key = match &fields[i] {
                    redis::Value::BulkString(bytes) => {
                        String::from_utf8_lossy(bytes).to_string()
                    }
                    _ => {
                        i += 2;
                        continue;
                    }
                };
                let val = match &fields[i + 1] {
                    redis::Value::BulkString(bytes) => {
                        String::from_utf8_lossy(bytes).to_string()
                    }
                    _ => {
                        i += 2;
                        continue;
                    }
                };

                match key.as_str() {
                    "entity_id" => entity_id_str = Some(val),
                    "data" => data_str = Some(val),
                    _ => {}
                }

                i += 2;
            }

            let entity_id = match entity_id_str.and_then(|s| s.parse::<Uuid>().ok()) {
                Some(id) => id,
                None => {
                    tracing::error!("Message {message_id}: missing or invalid entity_id");
                    continue;
                }
            };

            let request = match data_str.and_then(|s| {
                serde_json::from_str::<SendNotificationRequest>(&s).ok()
            }) {
                Some(r) => r,
                None => {
                    tracing::error!(
                        "Message {message_id}: missing or invalid data payload"
                    );
                    continue;
                }
            };

            results.push((message_id, entity_id, request));
        }
    }

    results
}

/// Convert a Channel enum variant to its database string representation.
fn channel_to_str(channel: &Channel) -> &'static str {
    match channel {
        Channel::Email => "email",
        Channel::WhatsApp => "whatsapp",
        Channel::Sms => "sms",
        Channel::InApp => "in_app",
    }
}

/// Send a one-off **test message** on a single channel for a tenant, using the
/// same provider-resolution + delivery path as real notifications (tenant
/// provider first, env fallback). Used by the admin "Send test" button so an
/// admin can verify credentials without waiting for a real event.
///
/// `channel` is one of `"email" | "sms" | "whatsapp"`. Returns `Ok(())` on a
/// successful provider send, or `Err(msg)` describing why it failed.
pub async fn send_test_message(
    pool: &PgPool,
    entity_id: Uuid,
    channel: &str,
    recipient: &str,
) -> Result<(), String> {
    let ch = match channel {
        "email" => Channel::Email,
        "sms" => Channel::Sms,
        "whatsapp" => Channel::WhatsApp,
        other => return Err(format!("unknown channel '{other}'")),
    };

    // Build env-fallback transports once (same as the worker does at startup).
    let transports = Transports {
        smtp: build_smtp_transport(),
        sms: SmsProvider::from_env(),
        whatsapp: WhatsAppProvider::from_env(),
    };

    let req = SendNotificationRequest {
        event_type: crate::notifications::NotificationEventType::ScheduledReport,
        channels: vec![ch.clone()],
        recipients: vec![recipient.to_string()],
        subject: Some("Zavora ERP — test notification".to_string()),
        body: "<p>This is a <strong>test</strong> notification from Zavora ERP. \
               If you received it, this channel is configured correctly.</p>"
            .to_string(),
        related_type: None,
        related_id: None,
        schedule_at: None,
        attachments: Vec::new(),
    };

    deliver(pool, &transports, &entity_id, &ch, recipient, &req).await
}
