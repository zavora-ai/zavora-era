use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// User roles as defined in spec section 14.1.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum UserRole {
    Viewer,
    Editor,
    Approver,
    /// HR Manager — full HR (people, leave) without finance/GL access.
    HrManager,
    Accountant,
    Admin,
    Owner,
}

impl UserRole {
    /// Stable string key for this role — matches the value stored in
    /// `era_users.role`, the JWT `role` claim, and `roles.key` for the seeded
    /// system roles.
    pub fn key(&self) -> &'static str {
        match self {
            Self::Owner => "Owner",
            Self::Admin => "Admin",
            Self::Accountant => "Accountant",
            Self::HrManager => "HrManager",
            Self::Editor => "Editor",
            Self::Approver => "Approver",
            Self::Viewer => "Viewer",
        }
    }

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
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct EraUserRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub is_active: bool,
    #[sqlx(default)]
    pub status: String,
    pub invited_by: Option<Uuid>,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Request to invite/create a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub display_name: String,
    /// Role KEY (system role e.g. "Admin", or a custom-role slug). Validated
    /// against the `roles` table by the handler.
    pub role: String,
    /// Optional initial password. When provided the account is immediately active
    /// and can sign in; when omitted the user is created in `invited` status and
    /// cannot sign in until a password is set.
    #[serde(default)]
    pub password: Option<String>,
}

/// Request to update a user.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    /// Role KEY (system or custom-role slug). Validated by the handler.
    pub role: Option<String>,
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

// ─────────────────────────────────────────────────────────────────────────────
// Data-driven RBAC (Phase 0): permission catalog, system roles, and the
// behaviour-preserving seed. The catalog of *what permissions exist* is defined
// here in code (versioned) and synced into the `permissions` table on startup;
// *which role has which permission* is stored in `role_permissions`. The seed
// below reproduces the existing `middleware::auth` const role-groups exactly, so
// enabling this model changes no behaviour (verified by a golden test).
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashSet;

/// A single permission in the catalog. `key` is the stable `module.action`-style
/// identifier used by `require_permission` and stored in `role_permissions`.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PermissionRow {
    pub key: String,
    pub category: String,
    pub label: String,
    pub description: Option<String>,
}

/// A role (system or per-tenant custom).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RoleRow {
    pub id: Uuid,
    pub entity_id: Option<Uuid>,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub is_assignable: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single generated catalog permission (runtime; the catalog is generated from
/// the resource×verb table below).
#[derive(Debug, Clone)]
pub struct Permission {
    pub key: String,
    pub category: String,
    pub label: String,
    pub description: String,
}

/// A resource and the action verbs it supports (see docs/RBAC_V2_GRANULAR.md).
struct Resource {
    key: &'static str,
    category: &'static str,
    label: &'static str,
    verbs: &'static [&'static str],
}

/// Human label for an action verb (used to build the permission label).
fn verb_label(v: &str) -> &'static str {
    match v {
        "read" => "View", "create" => "Create", "update" => "Edit", "delete" => "Delete",
        "post" => "Post", "approve" => "Approve", "reject" => "Reject", "send" => "Send",
        "void" => "Void", "reverse" => "Reverse", "close" => "Close/Reopen", "run" => "Run",
        "pay" => "Pay", "convert" => "Convert", "publish" => "Publish", "award" => "Award",
        "submit" => "Submit", "apply" => "Apply", "categorise" => "Categorise",
        "reconcile" => "Reconcile", "import" => "Import", "receive" => "Receive",
        "adjust" => "Adjust", "issue" => "Issue", "remit" => "Remit", "complete" => "Complete",
        "config" => "Configure", "manage" => "Manage", _ => "Access",
    }
}

/// The full resource taxonomy. `permission_catalog()` generates one permission per
/// `(resource, verb)`. Grouped by `category` for the roles matrix UI.
const RESOURCES: &[Resource] = &[
    // Sales
    Resource { key: "invoice", category: "Sales", label: "Invoices", verbs: &["read","create","update","delete","post","send","void","reverse"] },
    Resource { key: "credit_note", category: "Sales", label: "Credit Notes", verbs: &["read","create"] },
    Resource { key: "estimate", category: "Sales", label: "Estimates", verbs: &["read","create","update","delete","send","convert"] },
    Resource { key: "recurring_invoice", category: "Sales", label: "Recurring Invoices", verbs: &["read","create","update","delete"] },
    // Receivables
    Resource { key: "customer", category: "Receivables", label: "Customers", verbs: &["read","create","update","delete"] },
    Resource { key: "customer_statement", category: "Receivables", label: "Customer Statements", verbs: &["read","send"] },
    // Purchases
    Resource { key: "bill", category: "Purchases", label: "Bills", verbs: &["read","create","update","delete","approve","post","void"] },
    Resource { key: "supplier_credit", category: "Purchases", label: "Supplier Credits", verbs: &["read","create"] },
    Resource { key: "debit_note", category: "Purchases", label: "Debit Notes", verbs: &["read","create"] },
    Resource { key: "expense_claim", category: "Purchases", label: "Expense Claims", verbs: &["read","create","submit","approve"] },
    // Vendors
    Resource { key: "vendor", category: "Vendors", label: "Vendors", verbs: &["read","create","update","delete"] },
    // Procurement
    Resource { key: "requisition", category: "Procurement", label: "Requisitions", verbs: &["read","create","submit","approve","reject","convert"] },
    Resource { key: "tender", category: "Procurement", label: "Tenders", verbs: &["read","create","publish","award"] },
    Resource { key: "purchase_order", category: "Procurement", label: "Purchase Orders", verbs: &["read","create","send","receive"] },
    Resource { key: "goods_receipt", category: "Procurement", label: "Goods Receipts", verbs: &["read","create"] },
    Resource { key: "vendor_application", category: "Procurement", label: "Vendor Applications", verbs: &["read","approve","reject"] },
    Resource { key: "approval_limit", category: "Procurement", label: "Approval Limits", verbs: &["read","config"] },
    // Banking
    Resource { key: "payment", category: "Banking", label: "Payments", verbs: &["read","create","apply","delete"] },
    Resource { key: "bank_account", category: "Banking", label: "Bank Accounts", verbs: &["read","create","delete"] },
    Resource { key: "bank_transaction", category: "Banking", label: "Bank Transactions", verbs: &["read","categorise","reconcile","import"] },
    Resource { key: "reconciliation", category: "Banking", label: "Reconciliations", verbs: &["read","run","complete"] },
    // Inventory
    Resource { key: "product", category: "Inventory", label: "Products", verbs: &["read","create","update","delete"] },
    Resource { key: "inventory", category: "Inventory", label: "Inventory", verbs: &["read","adjust","receive","issue"] },
    // Assets
    Resource { key: "asset", category: "Assets", label: "Fixed Assets", verbs: &["read","create","run"] },
    // Accounting
    Resource { key: "journal", category: "Accounting", label: "Journal Entries", verbs: &["read","post","reverse"] },
    Resource { key: "account", category: "Accounting", label: "Chart of Accounts", verbs: &["read","create","update"] },
    Resource { key: "recurring_journal", category: "Accounting", label: "Recurring Journals", verbs: &["read","create","delete","run"] },
    Resource { key: "period", category: "Accounting", label: "Fiscal Periods", verbs: &["read","close"] },
    Resource { key: "opening_balance", category: "Accounting", label: "Opening Balances", verbs: &["read","create"] },
    Resource { key: "dimension", category: "Accounting", label: "Dimensions", verbs: &["read","create"] },
    // Tax
    Resource { key: "tax_filing", category: "Tax", label: "Tax Filings", verbs: &["read","create","remit"] },
    Resource { key: "wht_rate", category: "Tax", label: "WHT Rates", verbs: &["read","config"] },
    Resource { key: "etims", category: "Tax", label: "eTIMS (KRA)", verbs: &["read","config","run"] },
    Resource { key: "attachment", category: "Documents", label: "Attachments", verbs: &["read","create","delete"] },
    Resource { key: "posting_group", category: "Accounting", label: "Posting Groups", verbs: &["read","config"] },
    // FX
    Resource { key: "fx_rate", category: "FX", label: "FX Rates", verbs: &["read","create","delete","run"] },
    // Reports
    Resource { key: "report", category: "Reports", label: "Reports", verbs: &["read","export"] },
    Resource { key: "budget", category: "Reports", label: "Budgets", verbs: &["read","config"] },
    Resource { key: "custom_report", category: "Reports", label: "Custom Reports", verbs: &["read","create","delete"] },
    Resource { key: "report_schedule", category: "Reports", label: "Report Schedules", verbs: &["read","create","delete"] },
    Resource { key: "consolidation", category: "Reports", label: "Consolidation", verbs: &["read"] },
    // Payroll & HR (sensitive)
    Resource { key: "employee", category: "HR", label: "Employees", verbs: &["read","create","update"] },
    Resource { key: "pay_run", category: "HR", label: "Pay Runs", verbs: &["read","create","approve","post","pay","delete"] },
    Resource { key: "payroll_config", category: "HR", label: "Payroll Config", verbs: &["read","config"] },
    Resource { key: "leave", category: "HR", label: "Leave", verbs: &["read","create","approve"] },
    Resource { key: "leave_type", category: "HR", label: "Leave Types", verbs: &["read","config"] },
    Resource { key: "holiday", category: "HR", label: "Holidays", verbs: &["read","config"] },
    Resource { key: "onboarding", category: "HR", label: "Onboarding", verbs: &["read","create","update"] },
    // CRM
    Resource { key: "crm", category: "CRM", label: "CRM Settings", verbs: &["read","config"] },
    Resource { key: "lead", category: "CRM", label: "Leads", verbs: &["read","create","update","convert"] },
    Resource { key: "opportunity", category: "CRM", label: "Opportunities", verbs: &["read","create","update","close"] },
    Resource { key: "activity", category: "CRM", label: "Activities", verbs: &["read","create","update"] },
    Resource { key: "ticket", category: "CRM", label: "Tickets", verbs: &["read","create","update"] },
    // POS
    Resource { key: "pos_sale", category: "POS", label: "POS Sales", verbs: &["read","create"] },
    Resource { key: "pos_session", category: "POS", label: "Till Sessions", verbs: &["read","run"] },
    Resource { key: "pos_stock", category: "POS", label: "POS Stock", verbs: &["read","adjust"] },
    // Administration
    Resource { key: "user", category: "Admin", label: "Users", verbs: &["read","manage"] },
    Resource { key: "role", category: "Admin", label: "Roles", verbs: &["read","create","update","delete"] },
    Resource { key: "settings", category: "Admin", label: "Settings", verbs: &["read","config"] },
    Resource { key: "notification_provider", category: "Admin", label: "Notification Providers", verbs: &["read","config"] },
    Resource { key: "audit", category: "Admin", label: "Audit Trail", verbs: &["read"] },
    Resource { key: "portal_invite", category: "Admin", label: "Portal Invites", verbs: &["create"] },
];

/// Generate the full permission catalog (`resource.verb` keys).
pub fn permission_catalog() -> Vec<Permission> {
    let mut out = Vec::new();
    for r in RESOURCES {
        for v in r.verbs {
            out.push(Permission {
                key: format!("{}.{}", r.key, v),
                category: r.category.to_string(),
                label: format!("{} {}", verb_label(v), r.label),
                description: format!("{} — {}", verb_label(v), r.label),
            });
        }
    }
    out
}

/// The system roles seeded on startup (immutable, shared across tenants).
pub const SYSTEM_ROLES: &[(UserRole, &str)] = &[
    (UserRole::Owner, "Full access to everything, including billing and ownership transfer"),
    (UserRole::Admin, "Manage the workspace, users and all data"),
    (UserRole::Accountant, "Create, post and manage the books; cannot approve or administer users (SoD)"),
    (UserRole::HrManager, "Full HR & payroll without finance/GL/admin access"),
    (UserRole::Editor, "Create and edit operational records; cannot post, approve or delete"),
    (UserRole::Approver, "Authorize (approve) documents; cannot create or post (SoD)"),
    (UserRole::Viewer, "Read-only access to non-sensitive data (no payroll/HR/admin)"),
];

// ─── Seed rules (see docs/RBAC_V2_GRANULAR.md §4) ───────────────────────────
const APPROVE_VERBS: &[&str] = &["approve", "reject", "award", "publish"];
const CONFIG_VERBS: &[&str] = &["config", "manage"];
const EDITOR_VERBS: &[&str] = &["create", "update", "submit", "convert", "apply", "adjust", "receive", "issue", "categorise", "import"];
const FINANCE_CATS: &[&str] = &["Sales", "Receivables", "Purchases", "Vendors", "Banking", "Accounting", "Tax", "FX", "Assets"];
const OPS_CATS: &[&str] = &["Sales", "Receivables", "Purchases", "Vendors", "Inventory", "CRM", "Procurement"];
/// Resources whose reads expose salary/PII/audit data — excluded from Viewer.
const SENSITIVE_RESOURCES: &[&str] = &["pay_run", "employee", "payroll_config", "audit"];

/// Whether a system role is granted `(resource, verb)` per the seed rules.
fn role_grants(role: &UserRole, res: &Resource, verb: &str) -> bool {
    let sensitive = SENSITIVE_RESOURCES.contains(&res.key);
    let is_admin_cat = res.category == "Admin";
    // General read of a resource that is neither sensitive nor admin-only.
    let general_read = verb == "read" && !sensitive && !is_admin_cat;
    match role {
        UserRole::Owner | UserRole::Admin => true,
        UserRole::Viewer => general_read || (res.key == "report" && verb == "export"),
        UserRole::Editor => general_read || (OPS_CATS.contains(&res.category) && EDITOR_VERBS.contains(&verb)),
        UserRole::Approver => verb == "read" || APPROVE_VERBS.contains(&verb),
        UserRole::Accountant => {
            verb == "read" // all reads incl. sensitive (finance needs payroll/employee)
                || (FINANCE_CATS.contains(&res.category) && !APPROVE_VERBS.contains(&verb) && !CONFIG_VERBS.contains(&verb))
                || res.category == "Reports"
                || (res.key == "pay_run" && (verb == "post" || verb == "pay"))
        }
        UserRole::HrManager => res.category == "HR", // all HR verbs incl. reads
    }
}

/// The set of permission keys a system role holds under the seed rules.
pub fn seeded_permissions_for(role: &UserRole) -> HashSet<String> {
    let mut out = HashSet::new();
    for r in RESOURCES {
        for v in r.verbs {
            if role_grants(role, r, v) {
                out.insert(format!("{}.{}", r.key, v));
            }
        }
    }
    out
}

/// `(role_key, permission_key)` pairs for all system roles — the source of truth
/// the startup seeder reconciles `role_permissions` to.
pub fn system_role_permissions() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (role, _) in SYSTEM_ROLES {
        for perm in seeded_permissions_for(role) {
            out.push((role.key().to_string(), perm));
        }
    }
    out
}
