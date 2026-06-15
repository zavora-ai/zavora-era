-- Zavora ERP — Posting setup (account determination)
-- Phase 1: a single default posting setup per entity that centralises GL account
-- determination. Empty object means "use code defaults" (PostingSetup::default()).

ALTER TABLE entity_settings
    ADD COLUMN IF NOT EXISTS posting_setup JSONB NOT NULL DEFAULT '{}'::jsonb;
