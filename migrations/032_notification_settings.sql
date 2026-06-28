-- Zavora ERP — Migration 032: tenant-level notification event preferences
--
-- Lets an admin configure, per notification event type, whether the event fires
-- and on which channels — overriding the hardcoded channel choices at the
-- (non-reminder) notification call sites. A missing row means "use the built-in
-- default" for that event, so the table only stores explicit overrides.
--
-- Invoice reminders are NOT governed here — they remain per-customer via the
-- customer ReminderPolicy.

CREATE TABLE IF NOT EXISTS notification_settings (
    entity_id   UUID NOT NULL,
    event_type  TEXT NOT NULL,
    enabled     BOOLEAN NOT NULL DEFAULT TRUE,
    -- JSON array of channel names: "Email" | "Sms" | "WhatsApp" | "InApp".
    channels    JSONB NOT NULL DEFAULT '[]'::jsonb,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by  UUID,
    PRIMARY KEY (entity_id, event_type)
);
