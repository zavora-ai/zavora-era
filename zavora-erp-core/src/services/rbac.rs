//! Data-driven RBAC service (Phase 0).
//!
//! `sync_catalog` + `seed_system_roles` run on startup so code remains the source
//! of truth for the permission catalog and the built-in system roles (custom
//! per-tenant roles are never touched here). `resolve_permissions` maps a
//! `(entity_id, role_key)` to its effective permission-key set; `PermissionCache`
//! memoises that so per-request checks are cheap.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::rbac::{permission_catalog, system_role_permissions, SYSTEM_ROLES};
use crate::ErpError;

/// Upsert the permission catalog from code (idempotent). Permissions no longer
/// present in code are removed (cascading any grants to them).
pub async fn sync_catalog(engine: &ErpEngine) -> ErpResult<()> {
    let catalog = permission_catalog();
    for p in &catalog {
        sqlx::query(
            "INSERT INTO permissions (key, category, label, description) VALUES ($1,$2,$3,$4) \
             ON CONFLICT (key) DO UPDATE SET category = EXCLUDED.category, \
                 label = EXCLUDED.label, description = EXCLUDED.description",
        )
        .bind(&p.key)
        .bind(&p.category)
        .bind(&p.label)
        .bind(&p.description)
        .execute(engine.pool())
        .await
        .map_err(ErpError::Database)?;
    }
    let keys: Vec<String> = catalog.iter().map(|p| p.key.clone()).collect();
    sqlx::query("DELETE FROM permissions WHERE key <> ALL($1)")
        .bind(&keys)
        .execute(engine.pool())
        .await
        .map_err(ErpError::Database)?;
    Ok(())
}

/// Upsert the built-in system roles and reconcile their permissions to exactly
/// match the code-defined seed. Idempotent; safe to run on every boot. Custom
/// (per-tenant) roles are untouched.
pub async fn seed_system_roles(engine: &ErpEngine) -> ErpResult<()> {
    let mut tx = engine.pool().begin().await.map_err(ErpError::Database)?;

    // 1) Upsert each system role row (entity_id IS NULL, is_system = true).
    for (role, desc) in SYSTEM_ROLES {
        let key = role.key();
        let existing: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM roles WHERE key = $1 AND entity_id IS NULL",
        )
        .bind(key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(ErpError::Database)?;

        if let Some((id,)) = existing {
            sqlx::query(
                "UPDATE roles SET name = $2, description = $3, is_system = true, \
                     is_assignable = true, updated_at = NOW() WHERE id = $1",
            )
            .bind(id)
            .bind(key)
            .bind(*desc)
            .execute(&mut *tx)
            .await
            .map_err(ErpError::Database)?;
        } else {
            sqlx::query(
                "INSERT INTO roles (entity_id, key, name, description, is_system, is_assignable) \
                 VALUES (NULL, $1, $2, $3, true, true)",
            )
            .bind(key)
            .bind(key)
            .bind(*desc)
            .execute(&mut *tx)
            .await
            .map_err(ErpError::Database)?;
        }
    }

    // 2) Reconcile role_permissions for system roles to exactly match the seed.
    // Map role_key -> role_id for the system roles.
    let role_ids: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT key, id FROM roles WHERE entity_id IS NULL AND is_system = true",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(ErpError::Database)?;
    let id_by_key: HashMap<String, Uuid> = role_ids.into_iter().collect();

    // Desired grants grouped by role_id.
    let mut desired: HashMap<Uuid, HashSet<String>> = HashMap::new();
    for (role_key, perm_key) in system_role_permissions() {
        if let Some(rid) = id_by_key.get(&role_key) {
            desired.entry(*rid).or_default().insert(perm_key);
        }
    }

    for (&role_id, want) in &desired {
        // Delete grants that are no longer desired.
        let want_vec: Vec<String> = want.iter().cloned().collect();
        sqlx::query(
            "DELETE FROM role_permissions WHERE role_id = $1 AND permission_key <> ALL($2)",
        )
        .bind(role_id)
        .bind(&want_vec)
        .execute(&mut *tx)
        .await
        .map_err(ErpError::Database)?;
        // Insert missing grants.
        for perm_key in want {
            sqlx::query(
                "INSERT INTO role_permissions (role_id, permission_key) VALUES ($1, $2) \
                 ON CONFLICT DO NOTHING",
            )
            .bind(role_id)
            .bind(perm_key)
            .execute(&mut *tx)
            .await
            .map_err(ErpError::Database)?;
        }
    }

    tx.commit().await.map_err(ErpError::Database)?;
    Ok(())
}

/// Resolve the effective permission-key set for `(entity_id, role_key)`. A
/// tenant's own role of the same key takes precedence over the system role.
pub async fn resolve_permissions(
    engine: &ErpEngine,
    entity_id: Uuid,
    role_key: &str,
) -> ErpResult<HashSet<String>> {
    let role: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM roles \
         WHERE key = $1 AND (entity_id = $2 OR (entity_id IS NULL AND is_system)) \
         ORDER BY entity_id NULLS LAST LIMIT 1",
    )
    .bind(role_key)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await
    .map_err(ErpError::Database)?;

    let Some((role_id,)) = role else {
        return Ok(HashSet::new());
    };

    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT permission_key FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .fetch_all(engine.pool())
            .await
            .map_err(ErpError::Database)?;
    Ok(rows.into_iter().map(|(k,)| k).collect())
}

/// In-memory memoisation of `resolve_permissions`, keyed by `(entity_id, role_key)`.
/// Invalidate on any change to roles/role_permissions (Phase 3) or clear wholesale.
#[derive(Default)]
pub struct PermissionCache {
    inner: RwLock<HashMap<(Uuid, String), Arc<HashSet<String>>>>,
}

impl PermissionCache {
    pub fn new() -> Self {
        Self { inner: RwLock::new(HashMap::new()) }
    }

    /// Effective permissions for `(entity_id, role_key)`, loading + caching on miss.
    pub async fn effective(
        &self,
        engine: &ErpEngine,
        entity_id: Uuid,
        role_key: &str,
    ) -> ErpResult<Arc<HashSet<String>>> {
        let cache_key = (entity_id, role_key.to_string());
        if let Some(hit) = self.inner.read().unwrap().get(&cache_key).cloned() {
            return Ok(hit);
        }
        let perms = Arc::new(resolve_permissions(engine, entity_id, role_key).await?);
        self.inner.write().unwrap().insert(cache_key, perms.clone());
        Ok(perms)
    }

    /// Whether `(entity_id, role_key)` grants `perm` (loads on miss).
    pub async fn has(
        &self,
        engine: &ErpEngine,
        entity_id: Uuid,
        role_key: &str,
        perm: &str,
    ) -> ErpResult<bool> {
        Ok(self.effective(engine, entity_id, role_key).await?.contains(perm))
    }

    /// Drop a single role's cached entry (call after editing its permissions).
    pub fn invalidate(&self, entity_id: Uuid, role_key: &str) {
        self.inner.write().unwrap().remove(&(entity_id, role_key.to_string()));
    }

    /// Clear the whole cache (e.g. after a bulk role change).
    pub fn clear(&self) {
        self.inner.write().unwrap().clear();
    }
}

// ─── Roles administration (Phase 3) ─────────────────────────────────────────

/// The full permission catalog (from the DB, synced from code on startup).
pub async fn list_permissions(engine: &ErpEngine) -> ErpResult<Vec<crate::rbac::PermissionRow>> {
    sqlx::query_as::<_, crate::rbac::PermissionRow>(
        "SELECT key, category, label, description FROM permissions ORDER BY category, key",
    )
    .fetch_all(engine.pool())
    .await
    .map_err(ErpError::Database)
}

/// A role visible to a tenant (its own custom role, or a system role) + its
/// permission keys.
pub async fn get_role_with_perms(
    engine: &ErpEngine,
    entity_id: uuid::Uuid,
    role_id: uuid::Uuid,
) -> ErpResult<(crate::rbac::RoleRow, Vec<String>)> {
    let role = sqlx::query_as::<_, crate::rbac::RoleRow>(
        "SELECT id, entity_id, key, name, description, is_system, is_assignable, created_at, updated_at \
         FROM roles WHERE id = $1 AND (entity_id = $2 OR entity_id IS NULL)",
    )
    .bind(role_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await
    .map_err(ErpError::Database)?
    .ok_or_else(|| ErpError::NotFound { entity_type: "role".into(), id: role_id })?;

    let perms: Vec<(String,)> =
        sqlx::query_as("SELECT permission_key FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .fetch_all(engine.pool())
            .await
            .map_err(ErpError::Database)?;
    Ok((role, perms.into_iter().map(|(k,)| k).collect()))
}

/// Derive a URL-safe slug from a role name (lowercase, dashes), unique per tenant.
fn slugify(name: &str) -> String {
    let mut s: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "role".into() } else { s }
}

/// Validate that all keys exist in the catalog (reject unknown permission keys).
async fn validate_permission_keys(engine: &ErpEngine, keys: &[String]) -> ErpResult<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let known: Vec<(String,)> =
        sqlx::query_as("SELECT key FROM permissions WHERE key = ANY($1)")
            .bind(keys)
            .fetch_all(engine.pool())
            .await
            .map_err(ErpError::Database)?;
    if known.len() != keys.len() {
        return Err(ErpError::ValidationFailed {
            message: "One or more permission keys are not recognised".into(),
        });
    }
    Ok(())
}

/// Create a per-tenant custom role with the given permissions. Returns its id.
pub async fn create_custom_role(
    engine: &ErpEngine,
    entity_id: uuid::Uuid,
    name: &str,
    description: Option<&str>,
    permissions: &[String],
) -> ErpResult<uuid::Uuid> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ErpError::ValidationFailed { message: "Role name is required".into() });
    }
    validate_permission_keys(engine, permissions).await?;

    // Unique slug within the tenant (and not colliding with a system key).
    let base = slugify(name);
    let mut key = base.clone();
    let mut n = 2;
    loop {
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM roles WHERE key = $1 AND (entity_id = $2 OR entity_id IS NULL))",
        )
        .bind(&key)
        .bind(entity_id)
        .fetch_one(engine.pool())
        .await
        .map_err(ErpError::Database)?;
        if !taken {
            break;
        }
        key = format!("{base}-{n}");
        n += 1;
    }

    let mut tx = engine.pool().begin().await.map_err(ErpError::Database)?;
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO roles (entity_id, key, name, description, is_system, is_assignable) \
         VALUES ($1, $2, $3, $4, false, true) RETURNING id",
    )
    .bind(entity_id)
    .bind(&key)
    .bind(name)
    .bind(description)
    .fetch_one(&mut *tx)
    .await
    .map_err(ErpError::Database)?;

    for p in permissions {
        sqlx::query("INSERT INTO role_permissions (role_id, permission_key) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(id)
            .bind(p)
            .execute(&mut *tx)
            .await
            .map_err(ErpError::Database)?;
    }
    tx.commit().await.map_err(ErpError::Database)?;
    Ok(id)
}

/// Update a custom role's name/description and/or reconcile its permission set.
/// System roles are immutable (rejected).
pub async fn update_custom_role(
    engine: &ErpEngine,
    entity_id: uuid::Uuid,
    role_id: uuid::Uuid,
    name: Option<&str>,
    description: Option<&str>,
    permissions: Option<&[String]>,
) -> ErpResult<()> {
    let (role, _) = get_role_with_perms(engine, entity_id, role_id).await?;
    if role.is_system || role.entity_id != Some(entity_id) {
        return Err(ErpError::ValidationFailed {
            message: "Built-in roles cannot be edited. Duplicate it to customise.".into(),
        });
    }
    if let Some(keys) = permissions {
        validate_permission_keys(engine, keys).await?;
    }

    let mut tx = engine.pool().begin().await.map_err(ErpError::Database)?;
    if name.is_some() || description.is_some() {
        sqlx::query(
            "UPDATE roles SET name = COALESCE($2, name), description = COALESCE($3, description), \
                 updated_at = NOW() WHERE id = $1",
        )
        .bind(role_id)
        .bind(name.map(|s| s.trim()))
        .bind(description)
        .execute(&mut *tx)
        .await
        .map_err(ErpError::Database)?;
    }
    if let Some(keys) = permissions {
        sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .execute(&mut *tx)
            .await
            .map_err(ErpError::Database)?;
        for p in keys {
            sqlx::query("INSERT INTO role_permissions (role_id, permission_key) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(role_id)
                .bind(p)
                .execute(&mut *tx)
                .await
                .map_err(ErpError::Database)?;
        }
    }
    tx.commit().await.map_err(ErpError::Database)?;
    Ok(())
}

/// Delete a custom role. Blocked for system roles or when users still hold it.
pub async fn delete_custom_role(
    engine: &ErpEngine,
    entity_id: uuid::Uuid,
    role_id: uuid::Uuid,
) -> ErpResult<()> {
    let (role, _) = get_role_with_perms(engine, entity_id, role_id).await?;
    if role.is_system || role.entity_id != Some(entity_id) {
        return Err(ErpError::ValidationFailed { message: "Built-in roles cannot be deleted.".into() });
    }
    let in_use: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM era_users WHERE entity_id = $1 AND role = $2 AND is_active = true",
    )
    .bind(entity_id)
    .bind(&role.key)
    .fetch_one(engine.pool())
    .await
    .map_err(ErpError::Database)?;
    if in_use > 0 {
        return Err(ErpError::ValidationFailed {
            message: format!("{in_use} active user(s) still have this role. Reassign them first."),
        });
    }
    sqlx::query("DELETE FROM roles WHERE id = $1 AND entity_id = $2")
        .bind(role_id)
        .bind(entity_id)
        .execute(engine.pool())
        .await
        .map_err(ErpError::Database)?;
    Ok(())
}
