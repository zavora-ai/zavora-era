-- Zavora ERP — Migration 017: analytical dimensions (segments)
--
-- Masters for dimensional/segment accounting. journal_lines.dimensions (JSONB)
-- already stores { type_code: value_code } per line; these tables define the
-- allowed types (e.g. Cost Centre, Project) and their values so capture can be
-- validated and reports can resolve codes to names.
-- Idempotent.

CREATE TABLE IF NOT EXISTS dimension_types (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id  UUID NOT NULL,
    code       TEXT NOT NULL,
    name       TEXT NOT NULL,
    is_active  BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, code)
);

CREATE TABLE IF NOT EXISTS dimension_values (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id  UUID NOT NULL,
    type_code  TEXT NOT NULL,
    code       TEXT NOT NULL,
    name       TEXT NOT NULL,
    is_active  BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, type_code, code)
);

CREATE INDEX IF NOT EXISTS idx_dimension_values_entity_type
    ON dimension_values (entity_id, type_code);
