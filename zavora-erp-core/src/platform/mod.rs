//! Platform operator plane — Super Admin identities and tenant directory.
//!
//! Separate from tenant `era_users` / RBAC. Operators manage tenants as objects;
//! they do not automatically receive ledger access inside a customer company.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// JWT / DB role for full platform operators.
pub const ROLE_PLATFORM_SUPER_ADMIN: &str = "PlatformSuperAdmin";

/// Nil entity id embedded in platform JWTs (no tenant scope).
pub fn platform_entity_id() -> Uuid {
    Uuid::nil()
}

pub fn is_platform_role(role: &str) -> bool {
    role.eq_ignore_ascii_case(ROLE_PLATFORM_SUPER_ADMIN)
        || role.eq_ignore_ascii_case("PlatformSupport")
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PlatformUserRow {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
    pub role: String,
    pub is_active: bool,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TenantRow {
    pub entity_id: Uuid,
    pub organization_name: String,
    pub organization_type: Option<String>,
    pub plan_key: Option<String>,
    pub plan_status: String,
    pub suspended_at: Option<DateTime<Utc>>,
    pub suspended_reason: Option<String>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub user_count: i32,
    pub invoice_count: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TenantSummary {
    pub entity_id: Uuid,
    pub organization_name: String,
    pub organization_type: Option<String>,
    pub plan_key: Option<String>,
    pub plan_status: String,
    pub suspended: bool,
    pub suspended_at: Option<DateTime<Utc>>,
    pub suspended_reason: Option<String>,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub user_count: i32,
    pub invoice_count: i32,
    /// Primary contact email (prefer active Owner, else first active user).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_email: Option<String>,
    /// Display name for that primary contact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_contact: Option<String>,
}

/// List/detail projection: tenant row + primary contact from `era_users`.
#[derive(Debug, Clone, FromRow)]
pub struct TenantListRow {
    pub entity_id: Uuid,
    pub organization_name: String,
    pub organization_type: Option<String>,
    pub plan_key: Option<String>,
    pub plan_status: String,
    pub suspended_at: Option<DateTime<Utc>>,
    pub suspended_reason: Option<String>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub user_count: i32,
    pub invoice_count: i32,
    pub primary_email: Option<String>,
    pub primary_contact: Option<String>,
}

impl From<TenantRow> for TenantSummary {
    fn from(r: TenantRow) -> Self {
        Self {
            entity_id: r.entity_id,
            organization_name: r.organization_name,
            organization_type: r.organization_type,
            plan_key: r.plan_key,
            plan_status: r.plan_status.clone(),
            suspended: r.suspended_at.is_some() || r.plan_status == "suspended",
            suspended_at: r.suspended_at,
            suspended_reason: r.suspended_reason,
            archived: r.archived_at.is_some(),
            created_at: r.created_at,
            last_activity_at: r.last_activity_at,
            user_count: r.user_count,
            invoice_count: r.invoice_count,
            primary_email: None,
            primary_contact: None,
        }
    }
}

impl From<TenantListRow> for TenantSummary {
    fn from(r: TenantListRow) -> Self {
        Self {
            entity_id: r.entity_id,
            organization_name: r.organization_name,
            organization_type: r.organization_type,
            plan_key: r.plan_key,
            plan_status: r.plan_status.clone(),
            suspended: r.suspended_at.is_some() || r.plan_status == "suspended",
            suspended_at: r.suspended_at,
            suspended_reason: r.suspended_reason,
            archived: r.archived_at.is_some(),
            created_at: r.created_at,
            last_activity_at: r.last_activity_at,
            user_count: r.user_count,
            invoice_count: r.invoice_count,
            primary_email: r.primary_email,
            primary_contact: r.primary_contact,
        }
    }
}

/// Active Owner (or first active user) chosen as the impersonation target.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TenantOwnerRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
}

/// Tenant staff user as seen by the platform directory (no password hash).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TenantUserSummary {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Operator audit event for the platform console.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PlatformAuditEvent {
    pub id: Uuid,
    pub actor_platform_user_id: Uuid,
    pub actor_email: Option<String>,
    pub action: String,
    pub target_entity_id: Option<Uuid>,
    pub organization_name: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Full tenant detail for the ops drawer.
#[derive(Debug, Clone, Serialize)]
pub struct TenantDetail {
    #[serde(flatten)]
    pub tenant: TenantSummary,
    pub users: Vec<TenantUserSummary>,
    pub recent_audit: Vec<PlatformAuditEvent>,
}
