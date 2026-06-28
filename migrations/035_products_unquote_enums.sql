-- Zavora ERP — Migration 035: normalise products enum columns to bare strings
--
-- `products.product_type`, `uom`, and `vat_treatment` were written via
-- serde_json::to_string(), which wraps the value in quotes (e.g. '"Service"').
-- Several readers, however, expect the BARE value:
--   * invoicing builds `format!("\"{}\"", vat_treatment)` before parsing, so a
--     quoted column became `""Exempt""` → parse failed → it silently fell back
--     to Standard16 (16% VAT) regardless of the product's real treatment;
--   * posting-group assignment compares `product_type <> 'Service'`, so quoted
--     `"Service"` never matched and services were mis-grouped as goods.
--
-- create_product / update_product now store bare strings; this backfills any
-- existing rows by stripping a single pair of surrounding double quotes.
-- Idempotent: rows already bare are unaffected by the trim.

UPDATE products
   SET product_type  = btrim(product_type,  '"'),
       uom           = btrim(uom,           '"'),
       vat_treatment = btrim(vat_treatment, '"')
 WHERE product_type LIKE '"%"'
    OR uom           LIKE '"%"'
    OR vat_treatment LIKE '"%"';
