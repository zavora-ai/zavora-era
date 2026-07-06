-- Clean up employment_type values that were stored JSON-quoted (e.g. "Permanent")
-- by an earlier create path. Strip surrounding double-quotes so the UI shows the
-- plain type. Idempotent.
UPDATE employees
   SET employment_type = btrim(employment_type, '"')
 WHERE employment_type LIKE '"%"';
