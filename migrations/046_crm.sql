-- ============================================================
-- CRM MODULE (optional, non-blocking add-in) — Phase 1 schema.
-- Additive only: new tables, no core schema changes. All CRM behaviour is gated
-- by crm_settings.enabled (default false), so this is safe to ship dark.
-- Customer portal principal (customer_users) mirrors vendor_users/employee_users.
-- See docs/CRM_MODULE_SPEC.md.
-- ============================================================

-- ── Per-tenant feature flag ─────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS crm_settings (
    entity_id          UUID PRIMARY KEY,
    enabled            BOOLEAN NOT NULL DEFAULT FALSE,
    default_pipeline_id UUID,
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ── Pipelines & stages ──────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS crm_pipelines (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id  UUID NOT NULL,
    name       TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_crm_pipelines_entity ON crm_pipelines(entity_id);

CREATE TABLE IF NOT EXISTS crm_stages (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID NOT NULL,
    pipeline_id UUID NOT NULL REFERENCES crm_pipelines(id),
    name        TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    probability NUMERIC(5,2) NOT NULL DEFAULT 0,   -- 0..100
    is_won      BOOLEAN NOT NULL DEFAULT FALSE,
    is_lost     BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX IF NOT EXISTS idx_crm_stages_pipeline ON crm_stages(pipeline_id, sort_order);

-- ── Leads ───────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS crm_leads (
    id                       UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id                UUID NOT NULL,
    name                     TEXT NOT NULL,
    company                  TEXT,
    email                    TEXT,
    phone                    TEXT,
    source                   TEXT,
    status                   TEXT NOT NULL DEFAULT 'New', -- New|Working|Qualified|Unqualified|Converted
    rating                   TEXT,                        -- Hot|Warm|Cold
    owner_user_id            UUID,
    notes                    TEXT,
    converted_customer_id    UUID,
    converted_opportunity_id UUID,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_crm_leads_entity ON crm_leads(entity_id, status);
CREATE INDEX IF NOT EXISTS idx_crm_leads_owner  ON crm_leads(owner_user_id);

-- ── Opportunities (deals) ───────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS crm_opportunities (
    id                  UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id           UUID NOT NULL,
    name                TEXT NOT NULL,
    pipeline_id         UUID NOT NULL REFERENCES crm_pipelines(id),
    stage_id            UUID NOT NULL REFERENCES crm_stages(id),
    customer_id         UUID,
    lead_id             UUID,
    amount              NUMERIC(16,2) NOT NULL DEFAULT 0,
    currency            TEXT NOT NULL DEFAULT 'KES',
    expected_close_date DATE,
    probability         NUMERIC(5,2) NOT NULL DEFAULT 0,
    status              TEXT NOT NULL DEFAULT 'Open',  -- Open|Won|Lost
    owner_user_id       UUID,
    lost_reason         TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at           TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_crm_opps_entity   ON crm_opportunities(entity_id, status);
CREATE INDEX IF NOT EXISTS idx_crm_opps_stage    ON crm_opportunities(stage_id);
CREATE INDEX IF NOT EXISTS idx_crm_opps_customer ON crm_opportunities(customer_id);

CREATE TABLE IF NOT EXISTS crm_opportunity_events (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id      UUID NOT NULL,
    opportunity_id UUID NOT NULL REFERENCES crm_opportunities(id),
    from_stage     UUID,
    to_stage       UUID,
    note           TEXT,
    actor_id       UUID,
    at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_crm_opp_events_opp ON crm_opportunity_events(opportunity_id);

-- ── Activities ──────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS crm_activities (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,
    kind          TEXT NOT NULL DEFAULT 'Task', -- Task|Call|Meeting|Email|Note
    subject       TEXT NOT NULL,
    notes         TEXT,
    due_date      TIMESTAMPTZ,
    done          BOOLEAN NOT NULL DEFAULT FALSE,
    done_at       TIMESTAMPTZ,
    related_type  TEXT,                          -- Lead|Opportunity|Customer
    related_id    UUID,
    owner_user_id UUID,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_crm_activities_entity  ON crm_activities(entity_id, done);
CREATE INDEX IF NOT EXISTS idx_crm_activities_related ON crm_activities(related_type, related_id);

-- ── Customer portal principal (mirrors vendor_users/employee_users) ─────────
CREATE TABLE IF NOT EXISTS customer_users (
    id                UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id         UUID NOT NULL,
    email             TEXT NOT NULL,
    display_name      TEXT NOT NULL,
    password_hash     TEXT,                              -- NULL until invite accepted
    status            TEXT NOT NULL DEFAULT 'invited',   -- invited|active|suspended
    customer_id       UUID,                              -- linked AR customer account
    set_token         TEXT,
    set_token_expires TIMESTAMPTZ,
    last_login        TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, email)
);
CREATE INDEX IF NOT EXISTS idx_customer_users_entity   ON customer_users(entity_id);
CREATE INDEX IF NOT EXISTS idx_customer_users_customer ON customer_users(customer_id);
CREATE INDEX IF NOT EXISTS idx_customer_users_settoken ON customer_users(set_token);

-- ── Support tickets (customer self-service) ─────────────────────────────────
CREATE TABLE IF NOT EXISTS crm_tickets (
    id                          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id                   UUID NOT NULL,
    customer_id                 UUID,
    subject                     TEXT NOT NULL,
    description                 TEXT,
    status                      TEXT NOT NULL DEFAULT 'Open',   -- Open|Pending|Resolved|Closed
    priority                    TEXT NOT NULL DEFAULT 'Normal', -- Low|Normal|High|Urgent
    assigned_to_user_id         UUID,
    created_by_customer_user_id UUID,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_crm_tickets_entity   ON crm_tickets(entity_id, status);
CREATE INDEX IF NOT EXISTS idx_crm_tickets_customer ON crm_tickets(customer_id);

CREATE TABLE IF NOT EXISTS crm_ticket_messages (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID NOT NULL,
    ticket_id   UUID NOT NULL REFERENCES crm_tickets(id),
    author_kind TEXT NOT NULL,          -- staff|customer
    author_id   UUID,
    body        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_crm_ticket_messages_ticket ON crm_ticket_messages(ticket_id);
