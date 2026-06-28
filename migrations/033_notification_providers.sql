-- Zavora ERP — Migration 033: per-tenant notification provider configuration
--
-- Lets each tenant configure its OWN delivery providers (SMTP for email,
-- Africa's Talking for SMS, Twilio for WhatsApp) instead of relying solely on
-- the deployment-wide env credentials. The worker resolves a tenant's provider
-- per message and falls back to the env/deployment provider when a tenant has
-- not configured its own.
--
-- Secrets (SMTP password, API key, auth token) are NEVER stored in plaintext:
-- `secret_enc` holds an AES-256-GCM ciphertext and `secret_nonce` its 96-bit
-- nonce, encrypted with the deployment key `NOTIF_ENC_KEY`. Non-secret fields
-- (host, port, sender id, from address, account sid, etc.) live in `settings`.

CREATE TABLE IF NOT EXISTS notification_providers (
    entity_id     UUID NOT NULL,
    -- 'email' | 'sms' | 'whatsapp'
    channel       TEXT NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT FALSE,
    -- Non-secret configuration (host, port, from, sender_id, account_sid, ...).
    settings      JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- AES-256-GCM ciphertext of the channel secret + its nonce (NULL = no secret set).
    secret_enc    BYTEA,
    secret_nonce  BYTEA,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by    UUID,
    PRIMARY KEY (entity_id, channel)
);
