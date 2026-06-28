-- Zavora ERP — Migration 030: notification delivery-history indexes
--
-- The admin delivery-history view lists notification rows across ALL channels
-- (email/sms/whatsapp/in_app) newest-first, filtered by channel/status/event.
-- These indexes keep that view and its stats summary fast as the notifications
-- table grows. Idempotent.

-- Primary listing order: per tenant, newest first.
CREATE INDEX IF NOT EXISTS idx_notifications_entity_created
    ON notifications (entity_id, created_at DESC);

-- Supports filtering/grouping by channel and status within a tenant
-- (the stats summary groups by these).
CREATE INDEX IF NOT EXISTS idx_notifications_entity_channel_status
    ON notifications (entity_id, channel, status);
