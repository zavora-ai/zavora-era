//! Tenant-level notification event preferences.
//!
//! For each notification **event type** an admin can configure whether the event
//! fires and on which channels, overriding the built-in defaults baked into the
//! notification call sites. A tenant that has never touched its settings uses the
//! built-in defaults; only explicit overrides are persisted (one row per
//! `(entity_id, event_type)` in `notification_settings`).
//!
//! Invoice reminders are intentionally NOT governed here — their schedule and
//! channels are configured per-customer via the customer `ReminderPolicy`. The
//! events covered here are the system/transactional ones whose channels were
//! previously hardcoded:
//!   * `InvoiceSent`, `PaymentReceived`, `CreditLimitExceeded`,
//!     `PeriodCloseWarning`, `BillApprovalNeeded`, `BillOverdue`,
//!     `PayRunApprovalNeeded`, `BankFeedError`, `ReceiptProcessed`,
//!     `ScheduledReport`, `InvoicePaid`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::notifications::NotificationEventType;
use crate::types::Channel;

/// One event's effective preference: whether it is enabled and its channels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventPref {
    /// Stored event-type key (the serde name of [`NotificationEventType`]).
    pub event_type: String,
    pub enabled: bool,
    pub channels: Vec<Channel>,
    /// `true` when this row is a built-in default (no stored override).
    #[serde(default)]
    pub is_default: bool,
}

/// The serde string key for an event type (e.g. `"InvoiceSent"`).
pub fn event_key(ev: &NotificationEventType) -> String {
    serde_json::to_value(ev)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "Unknown".to_string())
}

/// Which delivery channels are actually configured on this deployment, derived
/// from the same environment the worker uses to build its transports. This lets
/// the admin UI hint when a channel is ticked but won't deliver (e.g. SMS chosen
/// but Africa's Talking credentials are absent). In-app is always available.
///
/// Returns pairs of (`Channel`, `configured`).
pub fn channel_availability() -> Vec<(Channel, bool)> {
    let env_set = |k: &str| std::env::var(k).map(|v| !v.trim().is_empty()).unwrap_or(false);
    let email = env_set("SMTP_HOST");
    let sms = env_set("AT_USERNAME") && env_set("AT_API_KEY");
    let whatsapp = env_set("TWILIO_ACCOUNT_SID")
        && env_set("TWILIO_AUTH_TOKEN")
        && env_set("TWILIO_WHATSAPP_FROM");
    vec![
        (Channel::Email, email),
        (Channel::Sms, sms),
        (Channel::WhatsApp, whatsapp),
        (Channel::InApp, true),
    ]
}

/// The set of events that are configurable at the tenant level, with their
/// built-in default enabled state and channels. Order is the display order.
///
/// `InvoiceReminder` is deliberately excluded (per-customer `ReminderPolicy`).
pub fn configurable_defaults() -> Vec<(NotificationEventType, bool, Vec<Channel>)> {
    use Channel::*;
    use NotificationEventType::*;
    vec![
        (InvoiceSent, true, vec![Email]),
        (InvoicePaid, true, vec![InApp, Email]),
        (PaymentReceived, true, vec![InApp]),
        (CreditLimitExceeded, true, vec![InApp, Email]),
        (BillApprovalNeeded, true, vec![InApp, Email]),
        (BillOverdue, true, vec![InApp]),
        (PayRunApprovalNeeded, true, vec![InApp, Email]),
        (PeriodCloseWarning, true, vec![InApp, Email]),
        (BankFeedError, true, vec![InApp]),
        (ReceiptProcessed, true, vec![InApp]),
        (ScheduledReport, true, vec![Email]),
    ]
}

/// Look up the built-in default `(enabled, channels)` for one event.
fn default_for(ev: &NotificationEventType) -> (bool, Vec<Channel>) {
    configurable_defaults()
        .into_iter()
        .find(|(e, _, _)| e == ev)
        .map(|(_, enabled, channels)| (enabled, channels))
        // Events not in the configurable set (only InvoiceReminder today) default
        // to enabled with no extra channels — they manage their own routing.
        .unwrap_or((true, Vec::new()))
}

/// Row shape for a stored override.
#[derive(Debug, sqlx::FromRow)]
struct OverrideRow {
    event_type: String,
    enabled: bool,
    channels: serde_json::Value,
}

/// Fetch all stored overrides for a tenant, keyed by event_type.
async fn overrides(
    engine: &ErpEngine,
    entity_id: Uuid,
) -> ErpResult<std::collections::HashMap<String, (bool, Vec<Channel>)>> {
    let rows = sqlx::query_as::<_, OverrideRow>(
        "SELECT event_type, enabled, channels FROM notification_settings WHERE entity_id = $1",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await
    .map_err(ErpError::Database)?;

    let mut map = std::collections::HashMap::new();
    for r in rows {
        let channels: Vec<Channel> = serde_json::from_value(r.channels).unwrap_or_default();
        map.insert(r.event_type, (r.enabled, channels));
    }
    Ok(map)
}

/// Return every configurable event's effective preference for a tenant: a stored
/// override when present, otherwise the built-in default (flagged `is_default`).
pub async fn get_all(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<EventPref>> {
    let ov = overrides(engine, entity_id).await?;
    let mut out = Vec::new();
    for (ev, def_enabled, def_channels) in configurable_defaults() {
        let key = event_key(&ev);
        let pref = match ov.get(&key) {
            Some((enabled, channels)) => EventPref {
                event_type: key,
                enabled: *enabled,
                channels: channels.clone(),
                is_default: false,
            },
            None => EventPref {
                event_type: key,
                enabled: def_enabled,
                channels: def_channels,
                is_default: true,
            },
        };
        out.push(pref);
    }
    Ok(out)
}

/// Resolve the effective `(enabled, channels)` for a single event at a call
/// site. Falls back to the built-in default when there is no stored override.
/// Used by the notification-emitting services to decide whether and how to send.
pub async fn effective_channels(
    engine: &ErpEngine,
    entity_id: Uuid,
    ev: &NotificationEventType,
) -> (bool, Vec<Channel>) {
    let key = event_key(ev);
    match overrides(engine, entity_id).await {
        Ok(ov) => match ov.get(&key) {
            Some((enabled, channels)) => (*enabled, channels.clone()),
            None => default_for(ev),
        },
        // On any DB error, fall back to the built-in default so notifications are
        // never silently dropped by a settings lookup failure.
        Err(_) => default_for(ev),
    }
}

/// Upsert one event's override. Validates the event is configurable.
pub async fn upsert(
    engine: &ErpEngine,
    entity_id: Uuid,
    event_type: &str,
    enabled: bool,
    channels: &[Channel],
    updated_by: Uuid,
) -> ErpResult<EventPref> {
    // Reject unknown / non-configurable events (e.g. InvoiceReminder).
    let is_configurable = configurable_defaults()
        .iter()
        .any(|(e, _, _)| event_key(e) == event_type);
    if !is_configurable {
        return Err(ErpError::ValidationFailed {
            message: format!("'{event_type}' is not a configurable notification event"),
        });
    }

    let channels_json = serde_json::to_value(channels).unwrap_or_else(|_| serde_json::json!([]));
    sqlx::query(
        r#"INSERT INTO notification_settings (entity_id, event_type, enabled, channels, updated_at, updated_by)
           VALUES ($1, $2, $3, $4, now(), $5)
           ON CONFLICT (entity_id, event_type)
           DO UPDATE SET enabled = EXCLUDED.enabled, channels = EXCLUDED.channels,
                         updated_at = now(), updated_by = EXCLUDED.updated_by"#,
    )
    .bind(entity_id)
    .bind(event_type)
    .bind(enabled)
    .bind(&channels_json)
    .bind(updated_by)
    .execute(engine.pool())
    .await
    .map_err(ErpError::Database)?;

    Ok(EventPref {
        event_type: event_type.to_string(),
        enabled,
        channels: channels.to_vec(),
        is_default: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_configurable_event_has_a_default() {
        // Defaults cover all non-reminder events declared on the enum.
        let keys: Vec<String> = configurable_defaults().iter().map(|(e, _, _)| event_key(e)).collect();
        for ev in [
            "InvoiceSent", "InvoicePaid", "PaymentReceived", "CreditLimitExceeded",
            "BillApprovalNeeded", "BillOverdue", "PayRunApprovalNeeded",
            "PeriodCloseWarning", "BankFeedError", "ReceiptProcessed", "ScheduledReport",
        ] {
            assert!(keys.contains(&ev.to_string()), "missing default for {ev}");
        }
        // InvoiceReminder is intentionally excluded (per-customer policy).
        assert!(!keys.contains(&"InvoiceReminder".to_string()));
    }

    #[test]
    fn event_key_matches_serde_name() {
        assert_eq!(event_key(&NotificationEventType::InvoiceSent), "InvoiceSent");
        assert_eq!(event_key(&NotificationEventType::PeriodCloseWarning), "PeriodCloseWarning");
    }

    #[test]
    fn default_for_known_event() {
        let (enabled, channels) = default_for(&NotificationEventType::ScheduledReport);
        assert!(enabled);
        assert_eq!(channels, vec![Channel::Email]);
    }
}
