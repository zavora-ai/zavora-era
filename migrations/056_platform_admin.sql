-- 056: Platform super-admin plane (Zavora ops).
-- Separate from tenant era_users / RBAC. Operators manage tenants as objects;
-- they do not receive a tenant Owner role by default.

-- Platform operators (global identity)
CREATE TABLE IF NOT EXISTS platform_users (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email           TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    password_hash   TEXT NOT NULL,
    role            TEXT NOT NULL DEFAULT 'PlatformSuperAdmin',
    is_active       BOOLEAN NOT NULL DEFAULT true,
    last_login      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT platform_users_email_lower UNIQUE (email)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_platform_users_email_ci
    ON platform_users (lower(email));

-- Tenant registry (directory for ops; entity_id matches entity_settings)
CREATE TABLE IF NOT EXISTS tenants (
    entity_id           UUID PRIMARY KEY,
    organization_name   TEXT NOT NULL DEFAULT 'My Company',
    organization_type   TEXT,
    plan_key            TEXT,
    plan_status         TEXT NOT NULL DEFAULT 'active',
    -- active | trial | past_due | suspended
    suspended_at        TIMESTAMPTZ,
    suspended_reason    TEXT,
    archived_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_activity_at    TIMESTAMPTZ,
    user_count          INT NOT NULL DEFAULT 0,
    invoice_count       INT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_tenants_plan_status ON tenants (plan_status);
CREATE INDEX IF NOT EXISTS idx_tenants_name ON tenants (organization_name);

-- Operator audit trail
CREATE TABLE IF NOT EXISTS platform_audit_events (
    id                      UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    actor_platform_user_id  UUID NOT NULL REFERENCES platform_users(id),
    action                  TEXT NOT NULL,
    target_entity_id        UUID,
    metadata                JSONB,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_platform_audit_actor
    ON platform_audit_events (actor_platform_user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_platform_audit_entity
    ON platform_audit_events (target_entity_id, created_at DESC);

-- Backfill tenants from entity_settings (every provisioned company).
INSERT INTO tenants (
    entity_id, organization_name, organization_type, plan_key, plan_status,
    archived_at, created_at
)
SELECT
    s.entity_id,
    COALESCE(NULLIF(trim(s.organization_name), ''), 'My Company'),
    s.organization_type,
    COALESCE(
        NULLIF(trim(s.branding->>'plan'), ''),
        NULLIF(trim(s.subscription->>'plan'), '')
    ),
    CASE
        WHEN s.archived_at IS NOT NULL THEN 'suspended'
        WHEN COALESCE(s.subscription->>'status', '') IN ('trialing', 'trial') THEN 'trial'
        WHEN COALESCE(s.subscription->>'status', '') = 'past_due' THEN 'past_due'
        WHEN COALESCE(s.subscription->>'status', '') = 'active' THEN 'active'
        ELSE 'active'
    END,
    s.archived_at,
    COALESCE(
        (SELECT MIN(u.created_at) FROM era_users u WHERE u.entity_id = s.entity_id),
        NOW()
    )
FROM entity_settings s
ON CONFLICT (entity_id) DO NOTHING;

-- Denormalized counts (best-effort)
UPDATE tenants t SET
    user_count = (SELECT COUNT(*)::int FROM era_users u WHERE u.entity_id = t.entity_id AND u.is_active),
    invoice_count = (SELECT COUNT(*)::int FROM invoices i WHERE i.entity_id = t.entity_id),
    last_activity_at = (
        SELECT MAX(x.ts) FROM (
            SELECT MAX(last_login) AS ts FROM era_users WHERE entity_id = t.entity_id
            UNION ALL
            SELECT MAX(created_at) FROM invoices WHERE entity_id = t.entity_id
        ) x
    );
