use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// User roles as defined in spec section 14.1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum UserRole {
    Viewer,
    Editor,
    Approver,
    Accountant,
    Admin,
    Owner,
}

impl UserRole {
    /// Check if this role can post journal entries.
    pub fn can_post(&self) -> bool {
        matches!(self, Self::Accountant | Self::Admin | Self::Owner)
    }

    /// Check if this role can approve (bills, pay runs).
    pub fn can_approve(&self) -> bool {
        matches!(self, Self::Approver | Self::Admin | Self::Owner)
    }

    /// Check if this role can close periods.
    pub fn can_close_periods(&self) -> bool {
        matches!(self, Self::Admin | Self::Owner)
    }

    /// Check if this role can manage users.
    pub fn can_manage_users(&self) -> bool {
        matches!(self, Self::Admin | Self::Owner)
    }

    /// Check if this role can manage settings.
    pub fn can_manage_settings(&self) -> bool {
        matches!(self, Self::Admin | Self::Owner)
    }

    /// Check if this role can create drafts (invoices, bills).
    pub fn can_create_drafts(&self) -> bool {
        matches!(
            self,
            Self::Editor | Self::Approver | Self::Accountant | Self::Admin | Self::Owner
        )
    }

    /// Check if this role has read access.
    pub fn can_read(&self) -> bool {
        true // All roles have read access
    }

    /// Check if this role can delete attachments.
    pub fn can_delete_attachments(&self) -> bool {
        matches!(self, Self::Admin | Self::Owner)
    }
}

/// An ERA user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EraUser {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
    pub is_active: bool,
    pub invited_by: Option<Uuid>,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Database row for user.
#[derive(Debug, Clone, FromRow)]
pub struct EraUserRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    pub invited_by: Option<Uuid>,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Request to invite/create a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
}

/// Request to update a user.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub role: Option<UserRole>,
    pub is_active: Option<bool>,
}

/// Permission check result.
#[derive(Debug, Clone)]
pub struct PermissionCheck {
    pub allowed: bool,
    pub role: UserRole,
    pub action: String,
    pub reason: Option<String>,
}
