-- ============================================================
-- HR — Employee self-service invite / password-set / reset (Cycle 6)
-- A single-use, time-boxed token lets an employee set their own password
-- (accept invite) or reset a forgotten one, without HR handling passwords.
-- ============================================================

ALTER TABLE employee_users ADD COLUMN IF NOT EXISTS set_token          TEXT NULL;
ALTER TABLE employee_users ADD COLUMN IF NOT EXISTS set_token_expires  TIMESTAMPTZ NULL;
CREATE INDEX IF NOT EXISTS idx_employee_users_set_token ON employee_users(set_token);
