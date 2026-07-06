-- ============================================================
-- HR — Leave approval routing (Cycle 1)
-- Each employee may have a designated leave approver (a back-office era_user).
-- When unset, leave requests route to the HR/Owner/Admin pool — the common
-- Kenyan SME default. This keeps approval a back-office action while allowing
-- per-employee manager routing where desired.
-- ============================================================

ALTER TABLE employees ADD COLUMN IF NOT EXISTS approver_user_id UUID NULL REFERENCES era_users(id);
CREATE INDEX IF NOT EXISTS idx_employees_approver ON employees(approver_user_id);

-- Snapshot the resolved approver on each request (who it was routed to), for
-- the "assigned to me" approver view and audit.
ALTER TABLE leave_requests ADD COLUMN IF NOT EXISTS assigned_approver_id UUID NULL;
CREATE INDEX IF NOT EXISTS idx_leave_requests_assigned ON leave_requests(entity_id, assigned_approver_id);
