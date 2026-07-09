//! Platform super-admin services: bootstrap, auth helpers, tenant directory,
//! suspend/unsuspend, and support impersonation.

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth;
use crate::error::{ErpError, ErpResult};
use crate::platform::{
    PlatformUserRow, TenantOwnerRow, TenantRow, TenantSummary, ROLE_PLATFORM_SUPER_ADMIN,
};

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

    let total: i64 = if q.is_some() || status.is_some() {
        sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM tenants t
               WHERE ($1::text IS NULL OR t.organization_name ILIKE '%' || $1 || '%'
                      OR t.entity_id::text ILIKE '%' || $1 || '%')
                 AND ($2::text IS NULL OR t.plan_status = $2)"#,
        )
        .bind(&q)
        .bind(&status)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM tenants")
            .fetch_one(pool)
            .await?
    };

    let rows = sqlx::query_as::<_, TenantRow>(
        r#"SELECT * FROM tenants t
           WHERE ($1::text IS NULL OR t.organization_name ILIKE '%' || $1 || '%'
                  OR t.entity_id::text ILIKE '%' || $1 || '%')
             AND ($2::text IS NULL OR t.plan_status = $2)
           ORDER BY t.created_at DESC
           LIMIT $3 OFFSET $4"#,
    )
    .bind(&q)
    .bind(&status)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((rows.into_iter().map(TenantSummary::from).collect(), total))
}

pub async fn get_tenant(pool: &PgPool, entity_id: Uuid) -> ErpResult<Option<TenantSummary>> {
    let _ = refresh_tenant_counts(pool, entity_id).await;
    let row = sqlx::query_as::<_, TenantRow>("SELECT * FROM tenants WHERE entity_id = $1")
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
