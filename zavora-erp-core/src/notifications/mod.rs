use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Channel;

/// Notification event types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationEventType {
    InvoiceReminder,
    InvoicePaid,
    BillApprovalNeeded,
    BillOverdue,
    PayRunApprovalNeeded,
    PeriodCloseWarning,
    BankFeedError,
    ReceiptProcessed,
    PaymentReceived,
    CreditLimitExceeded,
    ScheduledReport,
    InvoiceSent,
}

/// Status of a notification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationStatus {
    Queued,
    Sent,
    Delivered,
    Failed,
    Read,
}

/// A notification record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub event_type: NotificationEventType,
    pub channel: Channel,
    pub recipient: String,
    pub subject: Option<String>,
    pub body: String,
    pub related_type: Option<String>,
    pub related_id: Option<Uuid>,
    pub status: NotificationStatus,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub sent_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// An email attachment carried with a notification. `content_base64` holds the
/// file bytes base64-encoded so the attachment survives JSON/Redis transport.
/// Attachments are delivery-only — they are NOT persisted to the notifications
/// table (only the body/subject are).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAttachment {
    pub filename: String,
    pub mime_type: String,
    pub content_base64: String,
}

/// Request to send a notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendNotificationRequest {
    pub event_type: NotificationEventType,
    pub channels: Vec<Channel>,
    pub recipients: Vec<String>,
    pub subject: Option<String>,
    pub body: String,
    pub related_type: Option<String>,
    pub related_id: Option<Uuid>,
    pub schedule_at: Option<DateTime<Utc>>,
    /// Optional email attachments (e.g. an invoice PDF). Only used by the Email
    /// channel; ignored elsewhere. Defaults to empty for backward compatibility.
    #[serde(default)]
    pub attachments: Vec<NotificationAttachment>,
}

/// A scheduled reminder job (for invoice reminders).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderJob {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub invoice_id: Uuid,
    pub customer_id: Uuid,
    pub channels: Vec<Channel>,
    pub scheduled_for: DateTime<Utc>,
    pub template_id: Option<Uuid>,
    pub executed: bool,
    pub executed_at: Option<DateTime<Utc>>,
}
