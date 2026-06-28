//! Multi-tenant signup: the `Tenant_Provisioner` and its request/result types.
//!
//! This module owns true tenant creation, distinct from the existing invite
//! flow. It atomically creates a new `entity_id`, its `entity_settings` row,
//! its first **Owner** user, optionally seeds the chart of accounts, and
//! records a tenant-creation audit event — all inside one database transaction.
//!
//! This file scaffolds the public data types. The pure `validate_signup`
//! function, the transaction-aware `seed_coa_in_tx` helper, and the
//! `provision_tenant` orchestrator are implemented in later tasks.

use crate::error::{ErpError, ErpResult};
use crate::ledger::account::CreateAccountRequest;
use crate::ledger::coa_template::{kenya_standard_coa, CoaTemplate};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use uuid::Uuid;

/// Raw signup inputs as received from the API layer, before validation.
///
/// `validate_signup` consumes this and produces a normalised
/// [`ProvisionTenantRequest`]; it never persists anything.
#[derive(Debug, Clone)]
pub struct SignupInput {
    /// Human-readable organisation name (validated non-empty after trimming).
    pub organization_name: String,
    /// Legal type of organisation (validated non-empty after trimming), e.g.
    /// "sole_proprietor", "limited_company", "partnership", "ngo".
    pub organization_type: String,
    /// KRA PIN (optional; trimmed + upper-cased when present).
    pub kra_pin: Option<String>,
    /// Owner email (validated for syntactic validity).
    pub owner_email: String,
    /// Owner display name (validated non-empty after trimming).
    pub owner_display_name: String,
    /// Owner password in plaintext (validated for length; hashed downstream).
    pub owner_password: String,
}

/// Validated, normalised signup inputs (produced by `validate_signup`).
#[derive(Debug, Clone)]
pub struct ProvisionTenantRequest {
    /// Trimmed, non-empty organisation name.
    pub organization_name: String,
    /// Trimmed, non-empty organisation type.
    pub organization_type: String,
    /// Normalised KRA PIN (trimmed + upper-cased), if supplied.
    pub kra_pin: Option<String>,
    /// Syntactically valid, normalised (trimmed + lower-cased) owner email.
    pub owner_email: String,
    /// Trimmed, non-empty owner display name.
    pub owner_display_name: String,
    /// Owner password in plaintext (>= 8 chars); hashed inside the provisioner.
    pub owner_password: String,
    /// Whether to auto-seed the chart of accounts within the same transaction.
    pub seed_chart_of_accounts: bool,
}

/// Result returned to the API layer after a successful commit.
#[derive(Debug, Clone)]
pub struct ProvisionedTenant {
    /// The newly created tenant key.
    pub entity_id: Uuid,
    /// The first Owner user's id.
    pub owner_user_id: Uuid,
    /// The normalised owner email.
    pub owner_email: String,
    /// The owner display name.
    pub owner_display_name: String,
    /// The owner's role — always `"Owner"`.
    pub role: String,
    /// The stored organisation name.
    pub organization_name: String,
    /// Number of chart-of-accounts rows seeded (0 when seeding is disabled).
    pub accounts_seeded: u32,
    /// Number of fiscal periods seeded for the current fiscal year.
    pub periods_seeded: u32,
}

/// Minimum accepted password length, per the Password_Policy (Req 7.2, 7.3).
const MIN_PASSWORD_LEN: usize = 8;

/// Check whether `email` is syntactically valid: exactly one `@`, a non-empty
/// local part, and a dotted domain with non-empty labels.
///
/// This is a deliberately conservative syntactic check (not a deliverability
/// check) and reveals nothing about whether the address exists anywhere.
fn is_syntactically_valid_email(email: &str) -> bool {
    // Exactly one '@' splitting a non-empty local part from a domain.
    let mut parts = email.split('@');
    let (local, domain) = match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => (local, domain),
        _ => return false,
    };

    if local.is_empty() || domain.is_empty() {
        return false;
    }

    // No whitespace anywhere in the address.
    if email.chars().any(char::is_whitespace) {
        return false;
    }

    // Domain must be dotted with non-empty labels (e.g. `example.com`).
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    if labels.iter().any(|label| label.is_empty()) {
        return false;
    }

    true
}

/// Validate and normalise raw signup input.
///
/// Returns the first failing field's error as
/// [`ErpError::ValidationFailed`], naming exactly one offending field and
/// revealing no tenant or user identifiers. On success, returns a normalised
/// [`ProvisionTenantRequest`] with `seed_chart_of_accounts` defaulted to `true`
/// (auto-seed). This function is pure and never persists anything (Req 7.4).
///
/// Normalisation: organization name and display name are trimmed; the email is
/// trimmed and lower-cased; the password is left unchanged.
///
/// _Requirements: 1.6, 7.1, 7.2, 7.3, 7.4, 7.5, 10.3_
pub fn validate_signup(input: SignupInput) -> ErpResult<ProvisionTenantRequest> {
    // Organisation name: reject if empty or whitespace-only (Req 1.6, 7.5).
    let organization_name = input.organization_name.trim();
    if organization_name.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "organization_name must not be empty".to_string(),
        });
    }

    // Organisation type: reject if empty or whitespace-only.
    let organization_type = input.organization_type.trim();
    if organization_type.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "organization_type must not be empty".to_string(),
        });
    }

    // KRA PIN: optional; trim + upper-case, and drop if blank after trimming.
    let kra_pin = input
        .kra_pin
        .map(|p| p.trim().to_uppercase())
        .filter(|p| !p.is_empty());

    // Owner email: trim + lower-case, then check syntactic validity (Req 1.6, 7.1).
    let owner_email = input.owner_email.trim().to_lowercase();
    if owner_email.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "email must not be empty".to_string(),
        });
    }
    if !is_syntactically_valid_email(&owner_email) {
        return Err(ErpError::ValidationFailed {
            message: "email is not a valid email address".to_string(),
        });
    }

    // Owner display name: reject if empty or whitespace-only (Req 1.6).
    let owner_display_name = input.owner_display_name.trim();
    if owner_display_name.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "display_name must not be empty".to_string(),
        });
    }

    // Owner password: reject if shorter than 8 characters (Req 1.6, 7.2, 7.3).
    // Password is not normalised.
    if input.owner_password.chars().count() < MIN_PASSWORD_LEN {
        return Err(ErpError::ValidationFailed {
            message: "password must be at least 8 characters".to_string(),
        });
    }

    Ok(ProvisionTenantRequest {
        organization_name: organization_name.to_string(),
        organization_type: organization_type.to_string(),
        kra_pin,
        owner_email,
        owner_display_name: owner_display_name.to_string(),
        owner_password: input.owner_password,
        // Default to auto-seeding the chart of accounts (Req 3.2).
        seed_chart_of_accounts: true,
    })
}

/// Insert all template accounts for `entity_id` within the caller's open
/// transaction, returning the number of accounts seeded.
///
/// Unlike [`crate::services::accounts::seed_coa`], which writes through the
/// auto-committing pool and is scoped to `engine.entity_id()`, this helper runs
/// inside a caller-supplied [`sqlx::Transaction`] and is parameterised by the
/// target `entity_id`. This lets tenant provisioning seed the chart of accounts
/// atomically alongside the `entity_settings` and Owner rows: any failure here
/// aborts the whole transaction and persists nothing.
///
/// Every seeded row is scoped to the supplied `entity_id` (Req 3.4). On a fresh
/// tenant there are no pre-existing accounts, so this inserts each template
/// account unconditionally.
///
/// _Requirements: 3.2, 3.4_
pub(crate) async fn seed_coa_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_id: Uuid,
    template: &CoaTemplate,
) -> ErpResult<u32> {
    // Resolve the template to its concrete set of accounts, mirroring
    // `services::accounts::seed_coa`.
    let accounts: Vec<CreateAccountRequest> = match template {
        CoaTemplate::KenyaStandard => kenya_standard_coa(),
        CoaTemplate::Minimal => kenya_standard_coa()
            .into_iter()
            .filter(|a| a.parent_code.is_none())
            .collect(),
        CoaTemplate::Custom => return Ok(0),
    };

    let mut count: u32 = 0;
    for req in accounts {
        let id = Uuid::new_v4();
        let now = Utc::now();
        // Serialise the account type to its stored text form, matching the
        // mapping used by `services::accounts::create_account`.
        let account_type_str = serde_json::to_value(&req.account_type)
            .unwrap_or_default()
            .as_str()
            .unwrap_or("asset")
            .to_string();

        sqlx::query(
            r#"INSERT INTO accounts
               (id, entity_id, code, name, account_type, parent_code, currency, is_control, is_active, tags, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
        )
        .bind(id)
        .bind(entity_id)
        .bind(&req.code)
        .bind(&req.name)
        .bind(&account_type_str)
        .bind(&req.parent_code)
        .bind(&req.currency)
        .bind(req.is_control)
        .bind(true)
        .bind(serde_json::to_value(&req.tags).unwrap_or_default())
        .bind(now)
        .execute(&mut **tx)
        .await?;

        count += 1;
    }

    Ok(count)
}

/// Seed 12 monthly fiscal periods for `fiscal_year` within the caller's open
/// transaction, returning the number of periods created.
///
/// `year_start_month` is the first month of the fiscal year (1 = January). Each
/// period that has already started (relative to today) is opened; periods that
/// begin in the future are marked `future`. Without these rows a brand-new
/// tenant can create drafts but cannot post to the GL, since posting resolves
/// the period for the document date.
///
/// Mirrors [`crate::services::periods::generate_periods`] but runs inside a
/// caller-supplied transaction and is parameterised by `entity_id`, so tenant
/// provisioning seeds periods atomically alongside the settings, Owner, and
/// chart-of-accounts rows.
pub(crate) async fn seed_periods_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entity_id: Uuid,
    fiscal_year: i32,
    year_start_month: u32,
) -> ErpResult<u32> {
    let today = Utc::now().date_naive();
    let now = Utc::now();
    let mut count: u32 = 0;

    for month_offset in 0..12u32 {
        let month = ((year_start_month - 1 + month_offset) % 12) + 1;
        let year = if month < year_start_month {
            fiscal_year + 1
        } else {
            fiscal_year
        };

        let start_date = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
            ErpError::ValidationFailed {
                message: format!("Invalid date: {year}-{month}-01"),
            }
        })?;
        let end_date = if month == 12 {
            NaiveDate::from_ymd_opt(year, 12, 31).unwrap()
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap() - Duration::days(1)
        };

        let status = if start_date > today { "future" } else { "open" };

        sqlx::query(
            r#"INSERT INTO fiscal_periods
               (id, entity_id, name, start_date, end_date, status, fiscal_year, period_number, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               ON CONFLICT (entity_id, start_date) DO NOTHING"#,
        )
        .bind(Uuid::new_v4())
        .bind(entity_id)
        .bind(start_date.format("%B %Y").to_string())
        .bind(start_date)
        .bind(end_date)
        .bind(status)
        .bind(fiscal_year)
        .bind((month_offset + 1) as i32)
        .bind(now)
        .execute(&mut **tx)
        .await?;

        count += 1;
    }

    Ok(count)
}

/// Provision a brand-new tenant atomically.
///
/// All writes share a single [`sqlx::Transaction`]; any failure returns `Err`
/// *before* `commit`, so the transaction rolls back and no row ever references
/// the candidate `entity_id` (Req 2.4, 2.5, 3.5, 14.1, 14.2).
///
/// Steps, in order:
/// 1. Generate a fresh `entity_id = Uuid::new_v4()` (Req 2.1, 12.2, 12.3).
/// 2. Open the transaction.
/// 3. Hash the password with `auth::hash_password` (Argon2id) — the plaintext
///    is never persisted (Req 2.3, 2.6).
/// 4. Insert the `entity_settings` row with `base_currency='KES'` and
///    `coa_template='KenyaStandard'`; other columns take their schema defaults
///    (Req 2.2, 3.1, 12.1).
/// 5. Insert the first Owner `era_users` row (role `Owner`, active, Argon2id
///    hash). A `UNIQUE(entity_id, email)` violation maps to a generic
///    [`ErpError::Duplicate`] that reveals nothing about other tenants
///    (Req 2.3, 8.1, 8.3, 13.3).
/// 6. When `seed_chart_of_accounts` is set, seed the `KenyaStandard` chart of
///    accounts inside the same transaction (Req 3.2, 3.4).
/// 7. Insert the tenant-creation `audit_events` row — `event_type='Created'`,
///    `object_type='tenant'`, `object_id=entity_id`, metadata
///    `{ organization_name, owner_user_id, created_at }`. No password or hash
///    is recorded (Req 11.1, 11.2, 11.3).
/// 8. Commit and return the [`ProvisionedTenant`].
///
/// _Requirements: 1.2, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 3.1, 3.2, 3.4, 3.5, 8.1, 8.3, 11.1, 11.2, 11.3, 12.1, 12.2, 12.3, 13.3, 14.1, 14.2_
pub async fn provision_tenant(
    pool: &sqlx::PgPool,
    req: ProvisionTenantRequest,
) -> ErpResult<ProvisionedTenant> {
    // Hash the password before any persistence; plaintext is never stored
    // (Req 2.3, 2.6). Then delegate to the hash-based provisioner so the
    // signup path and the "create an additional tenant for an already
    // authenticated user" path share one atomic implementation.
    let password_hash = crate::auth::hash_password(&req.owner_password)?;
    provision_tenant_with_hash(
        pool,
        ProvisionTenantWithHash {
            organization_name: req.organization_name,
            organization_type: req.organization_type,
            kra_pin: req.kra_pin,
            owner_email: req.owner_email,
            owner_display_name: req.owner_display_name,
            owner_password_hash: password_hash,
            seed_chart_of_accounts: req.seed_chart_of_accounts,
        },
    )
    .await
}

/// Like [`ProvisionTenantRequest`] but carrying an already-computed Argon2id
/// password **hash** instead of plaintext. Used when an authenticated user
/// creates an additional tenant: we reuse their existing stored hash rather
/// than asking for (or re-hashing) their password.
#[derive(Debug, Clone)]
pub struct ProvisionTenantWithHash {
    pub organization_name: String,
    pub organization_type: String,
    pub kra_pin: Option<String>,
    pub owner_email: String,
    pub owner_display_name: String,
    pub owner_password_hash: String,
    pub seed_chart_of_accounts: bool,
}

/// Provision a brand-new tenant atomically, given a pre-computed owner password
/// hash. See [`provision_tenant`] for the full step-by-step contract — this is
/// the shared implementation; the only difference is that the password hash is
/// supplied by the caller (never the plaintext).
pub async fn provision_tenant_with_hash(
    pool: &sqlx::PgPool,
    req: ProvisionTenantWithHash,
) -> ErpResult<ProvisionedTenant> {
    // 1. Fresh, unique tenant key (Req 2.1, 12.2, 12.3). The `entity_settings`
    //    primary key guards against the astronomically unlikely UUID collision:
    //    a clash surfaces as a unique violation that rolls the transaction back.
    let entity_id = Uuid::new_v4();
    let owner_user_id = Uuid::new_v4();
    let created_at = Utc::now();

    let password_hash = req.owner_password_hash;

    // 2. Open the single transaction shared by every write below.
    let mut tx = pool.begin().await?;

    // 4. entity_settings: organisation name + KES base currency + KenyaStandard
    //    COA template; remaining columns fall back to their schema defaults
    //    (Req 2.2, 3.1, 12.1).
    sqlx::query(
        r#"INSERT INTO entity_settings
               (entity_id, organization_name, organization_type, kra_pin, base_currency, coa_template)
           VALUES ($1, $2, $3, $4, 'KES', 'KenyaStandard')"#,
    )
    .bind(entity_id)
    .bind(&req.organization_name)
    .bind(&req.organization_type)
    .bind(&req.kra_pin)
    .execute(&mut *tx)
    .await?;

    // 5. First Owner user (Req 2.3, 8.1, 8.3, 13.3). A duplicate Owner email
    //    within this new tenant trips `UNIQUE(entity_id, email)`; map that to a
    //    generic, non-enumerating duplicate error (Req 10.2).
    let owner_insert = sqlx::query(
        r#"INSERT INTO era_users
               (id, entity_id, email, display_name, role, password_hash, status, is_active)
           VALUES ($1, $2, $3, $4, 'Owner', $5, 'active', true)"#,
    )
    .bind(owner_user_id)
    .bind(entity_id)
    .bind(&req.owner_email)
    .bind(&req.owner_display_name)
    .bind(&password_hash)
    .execute(&mut *tx)
    .await;

    if let Err(e) = owner_insert {
        // Returning before commit rolls the whole transaction back (Req 14.1).
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.is_unique_violation() {
                return Err(ErpError::Duplicate {
                    message: "an account with that email already exists".to_string(),
                });
            }
        }
        return Err(ErpError::Database(e));
    }

    // 6. Optionally seed the chart of accounts inside the same transaction
    //    (Req 3.2, 3.4). A failure here aborts everything (Req 3.5).
    let accounts_seeded = if req.seed_chart_of_accounts {
        seed_coa_in_tx(&mut tx, entity_id, &CoaTemplate::KenyaStandard).await?
    } else {
        0
    };

    // Seed the current fiscal year's monthly periods so the tenant can post to
    // the GL immediately. Defaults to a January-start fiscal year (matching the
    // default Dec-31 year-end in entity_settings).
    let periods_seeded = seed_periods_in_tx(&mut tx, entity_id, created_at.year(), 1).await?;

    // 7. Tenant-creation audit record — no password or hash is written
    //    (Req 11.1, 11.2, 11.3).
    let actor = serde_json::json!({
        "type": "system",
        "user_id": owner_user_id,
    });
    let after_state = serde_json::json!({
        "organization_name": req.organization_name,
        "owner_user_id": owner_user_id,
    });
    let metadata = serde_json::json!({
        "organization_name": req.organization_name,
        "owner_user_id": owner_user_id,
        "created_at": created_at,
    });

    sqlx::query(
        r#"INSERT INTO audit_events
               (entity_id, event_type, object_type, object_id, actor, after_state, metadata, timestamp)
           VALUES ($1, 'Created', 'tenant', $2, $3, $4, $5, $6)"#,
    )
    .bind(entity_id)
    .bind(entity_id)
    .bind(actor)
    .bind(after_state)
    .bind(metadata)
    .bind(created_at)
    .execute(&mut *tx)
    .await?;

    // 8. Commit only after every step succeeds.
    tx.commit().await?;

    Ok(ProvisionedTenant {
        entity_id,
        owner_user_id,
        owner_email: req.owner_email,
        owner_display_name: req.owner_display_name,
        role: "Owner".to_string(),
        organization_name: req.organization_name,
        accounts_seeded,
        periods_seeded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_input() -> SignupInput {
        SignupInput {
            organization_name: "Acme Ltd".to_string(),
            organization_type: "limited_company".to_string(),
            kra_pin: Some("A123456789X".to_string()),
            owner_email: "ada@example.com".to_string(),
            owner_display_name: "Ada Lovelace".to_string(),
            owner_password: "hunter2hunter".to_string(),
        }
    }

    #[test]
    fn accepts_and_normalises_valid_input() {
        let input = SignupInput {
            organization_name: "  Acme Ltd  ".to_string(),
            organization_type: "  limited_company  ".to_string(),
            kra_pin: Some("  a123456789x  ".to_string()),
            owner_email: "  Ada@Example.COM ".to_string(),
            owner_display_name: "  Ada Lovelace  ".to_string(),
            owner_password: "hunter2hunter".to_string(),
        };
        let req = validate_signup(input).expect("valid input should be accepted");
        assert_eq!(req.organization_name, "Acme Ltd");
        assert_eq!(req.organization_type, "limited_company");
        assert_eq!(req.kra_pin.as_deref(), Some("A123456789X")); // trimmed + upper-cased
        assert_eq!(req.owner_email, "ada@example.com");
        assert_eq!(req.owner_display_name, "Ada Lovelace");
        assert_eq!(req.owner_password, "hunter2hunter");
        assert!(req.seed_chart_of_accounts);
    }

    #[test]
    fn rejects_empty_organization_type() {
        let mut input = valid_input();
        input.organization_type = "   ".to_string();
        let err = validate_signup(input).unwrap_err();
        match err {
            ErpError::ValidationFailed { message } => {
                assert!(message.contains("organization_type"), "got: {message}");
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn blank_kra_pin_normalises_to_none() {
        let mut input = valid_input();
        input.kra_pin = Some("   ".to_string());
        let req = validate_signup(input).unwrap();
        assert_eq!(req.kra_pin, None);
    }

    #[test]
    fn rejects_empty_organization_name() {
        let mut input = valid_input();
        input.organization_name = "   ".to_string();
        let err = validate_signup(input).unwrap_err();
        match err {
            ErpError::ValidationFailed { message } => {
                assert!(message.contains("organization_name"), "got: {message}");
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn rejects_empty_display_name() {
        let mut input = valid_input();
        input.owner_display_name = "  ".to_string();
        let err = validate_signup(input).unwrap_err();
        match err {
            ErpError::ValidationFailed { message } => {
                assert!(message.contains("display_name"), "got: {message}");
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_email() {
        for bad in ["not-an-email", "a@b", "@example.com", "ada@", "a@@b.com", "ada example.com"] {
            let mut input = valid_input();
            input.owner_email = bad.to_string();
            let err = validate_signup(input).unwrap_err();
            match err {
                ErpError::ValidationFailed { message } => {
                    assert!(message.contains("email"), "input {bad:?} got: {message}");
                }
                other => panic!("expected ValidationFailed for {bad:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn rejects_short_password() {
        let mut input = valid_input();
        input.owner_password = "short7!".to_string(); // 7 chars
        let err = validate_signup(input).unwrap_err();
        match err {
            ErpError::ValidationFailed { message } => {
                assert!(message.contains("password"), "got: {message}");
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn accepts_exactly_eight_char_password() {
        let mut input = valid_input();
        input.owner_password = "12345678".to_string();
        assert!(validate_signup(input).is_ok());
    }

    #[test]
    fn first_failing_field_is_organization_name() {
        // All fields invalid; organization name is checked first.
        let input = SignupInput {
            organization_name: "  ".to_string(),
            organization_type: "  ".to_string(),
            kra_pin: None,
            owner_email: "bad".to_string(),
            owner_display_name: "  ".to_string(),
            owner_password: "x".to_string(),
        };
        let err = validate_signup(input).unwrap_err();
        match err {
            ErpError::ValidationFailed { message } => {
                assert!(message.contains("organization_name"), "got: {message}");
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn error_messages_reveal_no_identifiers() {
        // Validation errors must not echo back the supplied values.
        let mut input = valid_input();
        input.owner_email = "secretuser@private.example".to_string();
        input.owner_password = "short".to_string();
        let err = validate_signup(input).unwrap_err();
        if let ErpError::ValidationFailed { message } = err {
            assert!(!message.contains("secretuser"));
            assert!(!message.contains("short"));
        } else {
            panic!("expected ValidationFailed");
        }
    }
}
