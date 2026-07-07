-- 048_rbac.sql — Data-driven RBAC (Phase 0).
--
-- Additive and non-breaking: introduces a permission catalog, roles (system +
-- per-tenant custom), and a role→permission join. Enforcement is unchanged in
-- this phase; the tables are populated from code on startup (permission catalog
-- + system roles), reproducing the existing role-group behaviour exactly.
--
-- `era_users.role` intentionally remains the role KEY (text) — no destructive
-- data migration, and the JWT role claim is untouched. We add an activation
-- token so invited internal users can set a password (mirrors employee_users /
-- customer_users).

-- Catalog of all known permissions. Owned/synced by the application on startup.
CREATE TABLE IF NOT EXISTS permissions (
    key         TEXT PRIMARY KEY,          -- e.g. 'journal.post'
    category    TEXT NOT NULL,
    label       TEXT NOT NULL,
    description TEXT
);

-- Roles: system roles (entity_id IS NULL, immutable) + per-tenant custom roles.
CREATE TABLE IF NOT EXISTS roles (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NULL,               -- NULL = built-in/system role (shared)
    key           TEXT NOT NULL,           -- 'Owner','Admin',… or a custom slug
    name          TEXT NOT NULL,
    description   TEXT,
    is_system     BOOLEAN NOT NULL DEFAULT false,
    is_assignable BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- System keys are globally unique (entity_id NULL); custom keys unique per tenant.
CREATE UNIQUE INDEX IF NOT EXISTS idx_roles_system_key
    ON roles (key) WHERE entity_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_roles_tenant_key
    ON roles (entity_id, key) WHERE entity_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id        UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_key TEXT NOT NULL REFERENCES permissions(key) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_key)
);

-- Activation / password-reset token for internal users (invited era_users).
ALTER TABLE era_users
    ADD COLUMN IF NOT EXISTS set_token         TEXT,
    ADD COLUMN IF NOT EXISTS set_token_expires TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_era_users_set_token ON era_users(set_token);
