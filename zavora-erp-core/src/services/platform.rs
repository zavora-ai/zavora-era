//! Platform super-admin services: bootstrap, auth helpers, tenant directory,
//! suspend/unsuspend, and support impersonation.

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth;
use crate::error::{ErpError, ErpResult};
use crate::platform::{
    PlatformAuditEvent, PlatformMetrics, PlatformOperatorSummary, PlatformUserRow, TenantDetail,
    TenantListRow, TenantOwnerRow, TenantRow, TenantSummary, TenantUserSummary,
    ROLE_PLATFORM_SUPER_ADMIN, ROLE_PLATFORM_SUPPORT,
};

/// Prefer active Owner email/name, else first active user.
const PRIMARY_CONTACT_SQL: &str = r#"
    (SELECT u.email FROM era_users u
     WHERE u.entity_id = t.entity_id AND u.is_active = true
     ORDER BY CASE WHEN u.role = 'Owner' THEN 0 ELSE 1 END, u.created_at ASC NULLS LAST, u.id
     LIMIT 1) AS primary_email,
    (SELECT u.display_name FROM era_users u
     WHERE u.entity_id = t.entity_id AND u.is_active = true
     ORDER BY CASE WHEN u.role = 'Owner' THEN 0 ELSE 1 END, u.created_at ASC NULLS LAST, u.id
     LIMIT 1) AS primary_contact
"#;

/// One-shot backfill of plan_key/plan_status from entity_settings.subscription
/// (and branding.plan). Does not clear ops suspensions. Safe to call repeatedly.
pub async fn backfill_tenant_billing_from_settings(pool: &PgPool) -> ErpResult<u64> {
    let res = sqlx::query(
        r#"UPDATE tenants t SET
               plan_key = COALESCE(
                   NULLIF(trim(s.subscription->>'plan'), ''),
                   NULLIF(trim(s.branding->>'plan'), ''),
                   t.plan_key
               ),
               plan_status = CASE
                   WHEN t.suspended_at IS NOT NULL THEN 'suspended'
                   WHEN lower(COALESCE(s.subscription->>'status', '')) IN ('trialing', 'trial') THEN 'trial'
                   WHEN lower(COALESCE(s.subscription->>'status', '')) = 'past_due' THEN 'past_due'
                   ELSE 'active'
               END
           FROM entity_settings s
           WHERE s.entity_id = t.entity_id"#,
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Ensure a bootstrap Super Admin exists when env credentials are set.
/// Idempotent: if the email already exists, does nothing.
pub async fn bootstrap_from_env(pool: &PgPool) -> ErpResult<Option<Uuid>> {
    let email = match std::env::var("PLATFORM_BOOTSTRAP_EMAIL") {
        Ok(e) if !e.trim().is_empty() => e.trim().to_lowercase(),
        _ => return Ok(None),
    };
    let password = match std::env::var("PLATFORM_BOOTSTRAP_PASSWORD") {
        Ok(p) if p.len() >= 8 => p,
        Ok(_) => {
            return Err(ErpError::Internal(
                "PLATFORM_BOOTSTRAP_PASSWORD must be at least 8 characters".into(),
            ));
        }
        Err(_) => return Ok(None),
    };
    let display = std::env::var("PLATFORM_BOOTSTRAP_NAME")
        .unwrap_or_else(|_| "Platform Super Admin".into());

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM platform_users WHERE lower(email) = lower($1))",
    )
    .bind(&email)
    .fetch_one(pool)
    .await?;

    if exists {
        tracing::info!(%email, "platform bootstrap: operator already exists");
        return Ok(None);
    }

    let id = Uuid::new_v4();
    let hash = auth::hash_password(&password)?;
    sqlx::query(
        r#"INSERT INTO platform_users (id, email, display_name, password_hash, role, is_active)
           VALUES ($1, $2, $3, $4, $5, true)"#,
    )
    .bind(id)
    .bind(&email)
    .bind(display.trim())
    .bind(&hash)
    .bind(ROLE_PLATFORM_SUPER_ADMIN)
    .execute(pool)
    .await?;

    tracing::info!(%email, %id, "platform bootstrap: created Super Admin");
    Ok(Some(id))
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> ErpResult<Option<PlatformUserRow>> {
    let row = sqlx::query_as::<_, PlatformUserRow>(
        "SELECT * FROM platform_users WHERE lower(email) = lower($1)",
    )
    .bind(email.trim())
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> ErpResult<Option<PlatformUserRow>> {
    let row = sqlx::query_as::<_, PlatformUserRow>("SELECT * FROM platform_users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn touch_login(pool: &PgPool, id: Uuid) -> ErpResult<()> {
    sqlx::query("UPDATE platform_users SET last_login = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Upsert a tenant directory row (call after signup / create_tenant).
pub async fn upsert_tenant(
    pool: &PgPool,
    entity_id: Uuid,
    organization_name: &str,
    organization_type: Option<&str>,
    plan_key: Option<&str>,
) -> ErpResult<()> {
    sqlx::query(
        r#"INSERT INTO tenants (
               entity_id, organization_name, organization_type, plan_key, plan_status, created_at
           ) VALUES ($1, $2, $3, $4, 'active', NOW())
           ON CONFLICT (entity_id) DO UPDATE SET
               organization_name = EXCLUDED.organization_name,
               organization_type = COALESCE(EXCLUDED.organization_type, tenants.organization_type),
               plan_key = COALESCE(EXCLUDED.plan_key, tenants.plan_key)"#,
    )
    .bind(entity_id)
    .bind(organization_name)
    .bind(organization_type)
    .bind(plan_key)
    .execute(pool)
    .await?;
    Ok(())
}

/// Map Paystack / entity_settings subscription status → platform `plan_status`.
/// Does not clear ops suspension (`suspended_at`); those stay `suspended`.
pub fn map_subscription_status(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        "trialing" | "trial" => "trial",
        "past_due" | "pastdue" => "past_due",
        "active" | "paid" => "active",
        // Cancelled tenants keep access until period end — still listed as active.
        "cancelled" | "canceled" => "active",
        other if other == "suspended" => "suspended",
        _ => "active",
    }
}

/// Mirror billing subscription plan/status onto the platform tenants directory.
/// Best-effort: never fails the billing path. Skips plan_status when ops-suspended.
pub async fn sync_tenant_billing(
    pool: &PgPool,
    entity_id: Uuid,
    plan_key: Option<&str>,
    subscription_status: Option<&str>,
) -> ErpResult<()> {
    let plan_status = subscription_status.map(map_subscription_status);
    // Ensure a directory row exists (signup should have created one).
    let _ = ensure_tenant_row(pool, entity_id).await;

    sqlx::query(
        r#"UPDATE tenants SET
               plan_key = COALESCE($2, plan_key),
               plan_status = CASE
                   WHEN suspended_at IS NOT NULL THEN 'suspended'
                   WHEN $3::text IS NOT NULL THEN $3
                   ELSE plan_status
               END
           WHERE entity_id = $1"#,
    )
    .bind(entity_id)
    .bind(plan_key)
    .bind(plan_status)
    .execute(pool)
    .await?;
    Ok(())
}

/// Refresh denormalized counts for one tenant (best-effort).
pub async fn refresh_tenant_counts(pool: &PgPool, entity_id: Uuid) -> ErpResult<()> {
    sqlx::query(
        r#"UPDATE tenants SET
            user_count = (SELECT COUNT(*)::int FROM era_users u WHERE u.entity_id = $1 AND u.is_active),
            invoice_count = (SELECT COUNT(*)::int FROM invoices i WHERE i.entity_id = $1),
            last_activity_at = (
                SELECT MAX(x.ts) FROM (
                    SELECT MAX(last_login) AS ts FROM era_users WHERE entity_id = $1
                    UNION ALL
                    SELECT MAX(created_at) FROM invoices WHERE entity_id = $1
                ) x
            ),
            archived_at = (SELECT archived_at FROM entity_settings WHERE entity_id = $1)
           WHERE entity_id = $1"#,
    )
    .bind(entity_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct ListTenantsQuery {
    pub q: Option<String>,
    pub plan_status: Option<String>,
    /// When true, hide tenants with zero users (noise from empty seeds).
    pub hide_empty: bool,
    /// When true, exclude archived tenants.
    pub hide_archived: bool,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list_tenants(pool: &PgPool, query: ListTenantsQuery) -> ErpResult<(Vec<TenantSummary>, i64)> {
    let limit = query.limit.clamp(1, 200);
    let offset = query.offset.max(0);
    let q = query.q.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let status = query
        .plan_status
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Keep directory in sync with any entity_settings rows not yet in tenants.
    sqlx::query(
        r#"INSERT INTO tenants (entity_id, organization_name, organization_type, plan_key, plan_status, archived_at, created_at)
           SELECT s.entity_id,
                  COALESCE(NULLIF(trim(s.organization_name), ''), 'My Company'),
                  s.organization_type,
                  COALESCE(NULLIF(trim(s.branding->>'plan'), ''), NULLIF(trim(s.subscription->>'plan'), '')),
                  CASE WHEN s.archived_at IS NOT NULL THEN 'suspended' ELSE 'active' END,
                  s.archived_at,
                  NOW()
           FROM entity_settings s
           WHERE NOT EXISTS (SELECT 1 FROM tenants t WHERE t.entity_id = s.entity_id)"#,
    )
    .execute(pool)
    .await
    .ok();

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM tenants t
           WHERE ($1::text IS NULL
                  OR t.organization_name ILIKE '%' || $1 || '%'
                  OR t.entity_id::text ILIKE '%' || $1 || '%'
                  OR EXISTS (
                      SELECT 1 FROM era_users u
                      WHERE u.entity_id = t.entity_id
                        AND (u.email ILIKE '%' || $1 || '%'
                             OR u.display_name ILIKE '%' || $1 || '%')
                  ))
             AND ($2::text IS NULL OR t.plan_status = $2)
             AND (NOT $3::bool OR t.user_count > 0)
             AND (NOT $4::bool OR t.archived_at IS NULL)"#,
    )
    .bind(&q)
    .bind(&status)
    .bind(query.hide_empty)
    .bind(query.hide_archived)
    .fetch_one(pool)
    .await?;

    let sql = format!(
        r#"SELECT t.entity_id, t.organization_name, t.organization_type, t.plan_key, t.plan_status,
                  t.suspended_at, t.suspended_reason, t.archived_at, t.created_at, t.last_activity_at,
                  t.user_count, t.invoice_count,
                  {contact}
           FROM tenants t
           WHERE ($1::text IS NULL
                  OR t.organization_name ILIKE '%' || $1 || '%'
                  OR t.entity_id::text ILIKE '%' || $1 || '%'
                  OR EXISTS (
                      SELECT 1 FROM era_users u
                      WHERE u.entity_id = t.entity_id
                        AND (u.email ILIKE '%' || $1 || '%'
                             OR u.display_name ILIKE '%' || $1 || '%')
                  ))
             AND ($2::text IS NULL OR t.plan_status = $2)
             AND (NOT $3::bool OR t.user_count > 0)
             AND (NOT $4::bool OR t.archived_at IS NULL)
           ORDER BY t.created_at DESC
           LIMIT $5 OFFSET $6"#,
        contact = PRIMARY_CONTACT_SQL
    );

    let rows = sqlx::query_as::<_, TenantListRow>(&sql)
        .bind(&q)
        .bind(&status)
        .bind(query.hide_empty)
        .bind(query.hide_archived)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok((rows.into_iter().map(TenantSummary::from).collect(), total))
}

pub async fn get_tenant(pool: &PgPool, entity_id: Uuid) -> ErpResult<Option<TenantSummary>> {
    let _ = refresh_tenant_counts(pool, entity_id).await;
    let sql = format!(
        r#"SELECT t.entity_id, t.organization_name, t.organization_type, t.plan_key, t.plan_status,
                  t.suspended_at, t.suspended_reason, t.archived_at, t.created_at, t.last_activity_at,
                  t.user_count, t.invoice_count,
                  {contact}
           FROM tenants t
           WHERE t.entity_id = $1"#,
        contact = PRIMARY_CONTACT_SQL
    );
    let row = sqlx::query_as::<_, TenantListRow>(&sql)
        .bind(entity_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(TenantSummary::from))
}

pub async fn record_audit(
    pool: &PgPool,
    actor_id: Uuid,
    action: &str,
    target_entity_id: Option<Uuid>,
    metadata: Option<serde_json::Value>,
) -> ErpResult<()> {
    sqlx::query(
        r#"INSERT INTO platform_audit_events (id, actor_platform_user_id, action, target_entity_id, metadata, created_at)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(Uuid::new_v4())
    .bind(actor_id)
    .bind(action)
    .bind(target_entity_id)
    .bind(metadata.unwrap_or(serde_json::json!({})))
    .bind(Utc::now())
    .execute(pool)
    .await?;
    Ok(())
}

/// True when the tenant is ops-suspended (blocks tenant login / refresh).
pub async fn is_tenant_suspended(pool: &PgPool, entity_id: Uuid) -> ErpResult<bool> {
    let suspended: Option<bool> = sqlx::query_scalar(
        r#"SELECT (suspended_at IS NOT NULL OR plan_status = 'suspended')
           FROM tenants WHERE entity_id = $1"#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;
    Ok(suspended.unwrap_or(false))
}

/// Suspend a tenant: set status, reason, and revoke all refresh sessions for that entity.
pub async fn suspend_tenant(
    pool: &PgPool,
    entity_id: Uuid,
    reason: Option<&str>,
) -> ErpResult<TenantSummary> {
    ensure_tenant_row(pool, entity_id).await?;
    let reason = reason.map(str::trim).filter(|s| !s.is_empty());

    let updated = sqlx::query(
        r#"UPDATE tenants SET
               suspended_at = COALESCE(suspended_at, NOW()),
               suspended_reason = COALESCE($2, suspended_reason),
               plan_status = 'suspended'
           WHERE entity_id = $1"#,
    )
    .bind(entity_id)
    .bind(reason)
    .execute(pool)
    .await?
    .rows_affected();

    if updated == 0 {
        return Err(ErpError::NotFound {
            entity_type: "Tenant".into(),
            id: entity_id,
        });
    }

    revoke_entity_refresh_tokens(pool, entity_id).await?;

    get_tenant(pool, entity_id)
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "Tenant".into(),
            id: entity_id,
        })
}

/// Lift suspension and restore plan_status to active (or trial if previously trial — we use active).
pub async fn unsuspend_tenant(pool: &PgPool, entity_id: Uuid) -> ErpResult<TenantSummary> {
    ensure_tenant_row(pool, entity_id).await?;

    let updated = sqlx::query(
        r#"UPDATE tenants SET
               suspended_at = NULL,
               suspended_reason = NULL,
               plan_status = CASE
                   WHEN plan_status = 'suspended' THEN 'active'
                   ELSE plan_status
               END
           WHERE entity_id = $1"#,
    )
    .bind(entity_id)
    .execute(pool)
    .await?
    .rows_affected();

    if updated == 0 {
        return Err(ErpError::NotFound {
            entity_type: "Tenant".into(),
            id: entity_id,
        });
    }

    get_tenant(pool, entity_id)
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "Tenant".into(),
            id: entity_id,
        })
}

/// Kick all live tenant sessions for this entity (used on suspend).
pub async fn revoke_entity_refresh_tokens(pool: &PgPool, entity_id: Uuid) -> ErpResult<u64> {
    let res = sqlx::query(
        "UPDATE refresh_tokens SET revoked = true WHERE entity_id = $1 AND revoked = false",
    )
    .bind(entity_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// Prefer active Owner; fall back to any active user for empty-owner edge cases.
pub async fn pick_impersonation_target(
    pool: &PgPool,
    entity_id: Uuid,
) -> ErpResult<TenantOwnerRow> {
    if let Some(owner) = sqlx::query_as::<_, TenantOwnerRow>(
        r#"SELECT id, entity_id, email, display_name, role
           FROM era_users
           WHERE entity_id = $1 AND is_active = true AND role = 'Owner'
           ORDER BY created_at ASC NULLS LAST, id ASC
           LIMIT 1"#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok(owner);
    }

    sqlx::query_as::<_, TenantOwnerRow>(
        r#"SELECT id, entity_id, email, display_name, role
           FROM era_users
           WHERE entity_id = $1 AND is_active = true
           ORDER BY created_at ASC NULLS LAST, id ASC
           LIMIT 1"#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ErpError::ValidationFailed {
        message: "Tenant has no active users to impersonate".into(),
    })
}

/// Impersonate a specific user in the tenant (must belong to entity and be active).
pub async fn get_impersonation_target(
    pool: &PgPool,
    entity_id: Uuid,
    user_id: Uuid,
) -> ErpResult<TenantOwnerRow> {
    sqlx::query_as::<_, TenantOwnerRow>(
        r#"SELECT id, entity_id, email, display_name, role
           FROM era_users
           WHERE id = $1 AND entity_id = $2 AND is_active = true"#,
    )
    .bind(user_id)
    .bind(entity_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ErpError::ValidationFailed {
        message: "User not found, inactive, or not in this tenant".into(),
    })
}

pub async fn list_tenant_users(
    pool: &PgPool,
    entity_id: Uuid,
) -> ErpResult<Vec<TenantUserSummary>> {
    let rows = sqlx::query_as::<_, TenantUserSummary>(
        r#"SELECT id, email, display_name, role, is_active, last_login, created_at
           FROM era_users
           WHERE entity_id = $1
           ORDER BY
             CASE WHEN role = 'Owner' THEN 0 ELSE 1 END,
             created_at ASC NULLS LAST,
             email ASC"#,
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, Default)]
pub struct ListAuditQuery {
    pub entity_id: Option<Uuid>,
    pub action: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list_audit_events(
    pool: &PgPool,
    query: ListAuditQuery,
) -> ErpResult<(Vec<PlatformAuditEvent>, i64)> {
    let limit = query.limit.clamp(1, 200);
    let offset = query.offset.max(0);
    let action = query
        .action
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM platform_audit_events e
           WHERE ($1::uuid IS NULL OR e.target_entity_id = $1)
             AND ($2::text IS NULL OR e.action = $2)
             AND e.action NOT IN ('list_tenants', 'get_tenant')"#,
    )
    .bind(query.entity_id)
    .bind(&action)
    .fetch_one(pool)
    .await?;

    let rows = sqlx::query_as::<_, PlatformAuditEvent>(
        r#"SELECT e.id,
                  e.actor_platform_user_id,
                  u.email AS actor_email,
                  e.action,
                  e.target_entity_id,
                  t.organization_name,
                  COALESCE(e.metadata, '{}'::jsonb) AS metadata,
                  e.created_at
           FROM platform_audit_events e
           LEFT JOIN platform_users u ON u.id = e.actor_platform_user_id
           LEFT JOIN tenants t ON t.entity_id = e.target_entity_id
           WHERE ($1::uuid IS NULL OR e.target_entity_id = $1)
             AND ($2::text IS NULL OR e.action = $2)
             AND e.action NOT IN ('list_tenants', 'get_tenant')
           ORDER BY e.created_at DESC
           LIMIT $3 OFFSET $4"#,
    )
    .bind(query.entity_id)
    .bind(&action)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((rows, total))
}

pub async fn get_tenant_detail(pool: &PgPool, entity_id: Uuid) -> ErpResult<Option<TenantDetail>> {
    let tenant = match get_tenant(pool, entity_id).await? {
        Some(t) => t,
        None => return Ok(None),
    };
    let users = list_tenant_users(pool, entity_id).await?;
    let (recent_audit, _) = list_audit_events(
        pool,
        ListAuditQuery {
            entity_id: Some(entity_id),
            action: None,
            limit: 20,
            offset: 0,
        },
    )
    .await?;
    Ok(Some(TenantDetail {
        tenant,
        users,
        recent_audit,
    }))
}

/// Update plan_key and/or plan_status. Cannot set plan_status=suspended here —
/// use suspend_tenant so session revocation stays consistent.
pub async fn update_tenant_plan(
    pool: &PgPool,
    entity_id: Uuid,
    plan_key: Option<Option<String>>,
    plan_status: Option<String>,
) -> ErpResult<TenantSummary> {
    ensure_tenant_row(pool, entity_id).await?;

    if let Some(ref status) = plan_status {
        let s = status.trim();
        if !matches!(s, "active" | "trial" | "past_due") {
            return Err(ErpError::ValidationFailed {
                message: "plan_status must be active, trial, or past_due (use suspend for suspended)"
                    .into(),
            });
        }
        if s == "suspended" {
            return Err(ErpError::ValidationFailed {
                message: "Use the suspend endpoint to suspend a tenant".into(),
            });
        }
    }

    // plan_key: Some(None) clears; Some(Some(v)) sets; None leaves unchanged.
    let row = sqlx::query_as::<_, TenantRow>(
        r#"UPDATE tenants SET
               plan_key = CASE
                   WHEN $2::bool THEN $3
                   ELSE plan_key
               END,
               plan_status = COALESCE($4, plan_status)
           WHERE entity_id = $1
             AND (suspended_at IS NULL)
           RETURNING *"#,
    )
    .bind(entity_id)
    .bind(plan_key.is_some())
    .bind(plan_key.clone().flatten())
    .bind(plan_status.as_deref().map(str::trim))
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(TenantSummary::from(r)),
        None => {
            // Distinguish not found vs suspended.
            if is_tenant_suspended(pool, entity_id).await? {
                return Err(ErpError::ValidationFailed {
                    message: "Tenant is suspended; restore before changing plan".into(),
                });
            }
            Err(ErpError::NotFound {
                entity_type: "Tenant".into(),
                id: entity_id,
            })
        }
    }
}

pub async fn archive_tenant(pool: &PgPool, entity_id: Uuid) -> ErpResult<TenantSummary> {
    ensure_tenant_row(pool, entity_id).await?;
    let updated = sqlx::query(
        r#"UPDATE tenants SET archived_at = COALESCE(archived_at, NOW())
           WHERE entity_id = $1"#,
    )
    .bind(entity_id)
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(ErpError::NotFound {
            entity_type: "Tenant".into(),
            id: entity_id,
        });
    }
    // Mirror onto entity_settings when present (best-effort).
    let _ = sqlx::query(
        "UPDATE entity_settings SET archived_at = COALESCE(archived_at, NOW()) WHERE entity_id = $1",
    )
    .bind(entity_id)
    .execute(pool)
    .await;
    revoke_entity_refresh_tokens(pool, entity_id).await?;
    get_tenant(pool, entity_id)
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "Tenant".into(),
            id: entity_id,
        })
}

pub async fn unarchive_tenant(pool: &PgPool, entity_id: Uuid) -> ErpResult<TenantSummary> {
    ensure_tenant_row(pool, entity_id).await?;
    let updated = sqlx::query(
        r#"UPDATE tenants SET archived_at = NULL WHERE entity_id = $1"#,
    )
    .bind(entity_id)
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(ErpError::NotFound {
            entity_type: "Tenant".into(),
            id: entity_id,
        });
    }
    let _ = sqlx::query(
        "UPDATE entity_settings SET archived_at = NULL, archived_by = NULL WHERE entity_id = $1",
    )
    .bind(entity_id)
    .execute(pool)
    .await;
    get_tenant(pool, entity_id)
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "Tenant".into(),
            id: entity_id,
        })
}

// ── Operators ──────────────────────────────────────────────────────────────

pub async fn list_operators(pool: &PgPool) -> ErpResult<Vec<PlatformOperatorSummary>> {
    let rows = sqlx::query_as::<_, PlatformOperatorSummary>(
        r#"SELECT id, email, display_name, role, is_active, last_login, created_at
           FROM platform_users
           ORDER BY created_at ASC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create_operator(
    pool: &PgPool,
    email: &str,
    display_name: &str,
    password: &str,
    role: &str,
) -> ErpResult<PlatformOperatorSummary> {
    let email = email.trim().to_lowercase();
    let display_name = display_name.trim();
    if email.is_empty() || !email.contains('@') {
        return Err(ErpError::ValidationFailed {
            message: "Valid email is required".into(),
        });
    }
    if display_name.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "display_name is required".into(),
        });
    }
    if password.len() < 8 {
        return Err(ErpError::ValidationFailed {
            message: "Password must be at least 8 characters".into(),
        });
    }
    let role = normalize_operator_role(role)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM platform_users WHERE lower(email) = lower($1))",
    )
    .bind(&email)
    .fetch_one(pool)
    .await?;
    if exists {
        return Err(ErpError::ValidationFailed {
            message: "An operator with that email already exists".into(),
        });
    }
    let id = Uuid::new_v4();
    let hash = auth::hash_password(password)?;
    sqlx::query(
        r#"INSERT INTO platform_users (id, email, display_name, password_hash, role, is_active)
           VALUES ($1, $2, $3, $4, $5, true)"#,
    )
    .bind(id)
    .bind(&email)
    .bind(display_name)
    .bind(&hash)
    .bind(role)
    .execute(pool)
    .await?;

    let row = sqlx::query_as::<_, PlatformOperatorSummary>(
        r#"SELECT id, email, display_name, role, is_active, last_login, created_at
           FROM platform_users WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn set_operator_active(
    pool: &PgPool,
    operator_id: Uuid,
    is_active: bool,
    actor_id: Uuid,
) -> ErpResult<PlatformOperatorSummary> {
    if operator_id == actor_id && !is_active {
        return Err(ErpError::ValidationFailed {
            message: "You cannot deactivate your own account".into(),
        });
    }
    // Keep at least one active Super Admin.
    if !is_active {
        let target_role: Option<String> =
            sqlx::query_scalar("SELECT role FROM platform_users WHERE id = $1")
                .bind(operator_id)
                .fetch_optional(pool)
                .await?;
        let Some(role) = target_role else {
            return Err(ErpError::NotFound {
                entity_type: "PlatformUser".into(),
                id: operator_id,
            });
        };
        if role.eq_ignore_ascii_case(ROLE_PLATFORM_SUPER_ADMIN) {
            let active_admins: i64 = sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM platform_users
                   WHERE is_active = true AND lower(role) = lower($1)"#,
            )
            .bind(ROLE_PLATFORM_SUPER_ADMIN)
            .fetch_one(pool)
            .await?;
            if active_admins <= 1 {
                return Err(ErpError::ValidationFailed {
                    message: "Cannot deactivate the last active Super Admin".into(),
                });
            }
        }
    }

    let updated = sqlx::query("UPDATE platform_users SET is_active = $2 WHERE id = $1")
        .bind(operator_id)
        .bind(is_active)
        .execute(pool)
        .await?
        .rows_affected();
    if updated == 0 {
        return Err(ErpError::NotFound {
            entity_type: "PlatformUser".into(),
            id: operator_id,
        });
    }
    let row = sqlx::query_as::<_, PlatformOperatorSummary>(
        r#"SELECT id, email, display_name, role, is_active, last_login, created_at
           FROM platform_users WHERE id = $1"#,
    )
    .bind(operator_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

fn normalize_operator_role(role: &str) -> ErpResult<&'static str> {
    match role.trim() {
        "" | "PlatformSuperAdmin" => Ok(ROLE_PLATFORM_SUPER_ADMIN),
        "PlatformSupport" => Ok(ROLE_PLATFORM_SUPPORT),
        other if other.eq_ignore_ascii_case(ROLE_PLATFORM_SUPER_ADMIN) => {
            Ok(ROLE_PLATFORM_SUPER_ADMIN)
        }
        other if other.eq_ignore_ascii_case(ROLE_PLATFORM_SUPPORT) => Ok(ROLE_PLATFORM_SUPPORT),
        _ => Err(ErpError::ValidationFailed {
            message: "role must be PlatformSuperAdmin or PlatformSupport".into(),
        }),
    }
}

// ── Metrics ────────────────────────────────────────────────────────────────

pub async fn platform_metrics(pool: &PgPool) -> ErpResult<PlatformMetrics> {
    let tenants_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tenants")
        .fetch_one(pool)
        .await?;
    let tenants_suspended: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tenants WHERE suspended_at IS NOT NULL OR plan_status = 'suspended'",
    )
    .fetch_one(pool)
    .await?;
    let tenants_archived: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tenants WHERE archived_at IS NOT NULL")
            .fetch_one(pool)
            .await?;
    let tenants_trial: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tenants WHERE plan_status = 'trial'")
            .fetch_one(pool)
            .await?;
    let tenants_past_due: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tenants WHERE plan_status = 'past_due'")
            .fetch_one(pool)
            .await?;
    let tenants_with_users: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tenants WHERE user_count > 0")
            .fetch_one(pool)
            .await?;
    let tenants_active = (tenants_total - tenants_suspended - tenants_archived).max(0);
    let users_total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM era_users WHERE is_active = true")
            .fetch_one(pool)
            .await?;
    let operators_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM platform_users")
        .fetch_one(pool)
        .await?;
    let operators_active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM platform_users WHERE is_active = true")
            .fetch_one(pool)
            .await?;
    let impersonations_7d: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM platform_audit_events
           WHERE action = 'impersonate_tenant' AND created_at > NOW() - INTERVAL '7 days'"#,
    )
    .fetch_one(pool)
    .await?;
    let suspensions_7d: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM platform_audit_events
           WHERE action = 'suspend_tenant' AND created_at > NOW() - INTERVAL '7 days'"#,
    )
    .fetch_one(pool)
    .await?;
    let signups_7d: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM tenants WHERE created_at > NOW() - INTERVAL '7 days'"#,
    )
    .fetch_one(pool)
    .await?;

    Ok(PlatformMetrics {
        tenants_total,
        tenants_active,
        tenants_suspended,
        tenants_archived,
        tenants_trial,
        tenants_past_due,
        tenants_with_users,
        users_total,
        operators_total,
        operators_active,
        impersonations_7d,
        suspensions_7d,
        signups_7d,
    })
}

/// Ensure a tenants row exists for entity_id (sync from entity_settings if needed).
async fn ensure_tenant_row(pool: &PgPool, entity_id: Uuid) -> ErpResult<()> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenants WHERE entity_id = $1)")
            .bind(entity_id)
            .fetch_one(pool)
            .await?;
    if exists {
        return Ok(());
    }
    let inserted = sqlx::query(
        r#"INSERT INTO tenants (entity_id, organization_name, organization_type, plan_key, plan_status, archived_at, created_at)
           SELECT s.entity_id,
                  COALESCE(NULLIF(trim(s.organization_name), ''), 'My Company'),
                  s.organization_type,
                  COALESCE(NULLIF(trim(s.branding->>'plan'), ''), NULLIF(trim(s.subscription->>'plan'), '')),
                  'active',
                  s.archived_at,
                  NOW()
           FROM entity_settings s
           WHERE s.entity_id = $1
           ON CONFLICT (entity_id) DO NOTHING"#,
    )
    .bind(entity_id)
    .execute(pool)
    .await?
    .rows_affected();

    if inserted == 0 {
        return Err(ErpError::NotFound {
            entity_type: "Tenant".into(),
            id: entity_id,
        });
    }
    Ok(())
}
