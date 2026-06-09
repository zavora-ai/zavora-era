-- Zavora ERA — Immutability Guarantees (DB-level enforcement)
-- Spec section 25.2: These guarantees cannot be bypassed via application code.

-- ============================================================
-- TRIGGER 1: Posted journal entries cannot be mutated
-- ============================================================
CREATE OR REPLACE FUNCTION prevent_posted_journal_update()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status = 'posted' AND NEW.status != 'reversed' THEN
        RAISE EXCEPTION 'Cannot modify a posted journal entry (id: %). Use reversal instead.', OLD.id;
    END IF;
    -- Allow only status change to 'reversed'
    IF OLD.status = 'posted' AND NEW.status = 'reversed' THEN
        -- Only status may change
        IF NEW.number != OLD.number OR NEW.date != OLD.date OR 
           NEW.reference != OLD.reference OR NEW.description != OLD.description THEN
            RAISE EXCEPTION 'Only status may be updated on a posted journal entry (id: %).', OLD.id;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_prevent_posted_journal_update
    BEFORE UPDATE ON journal_entries
    FOR EACH ROW
    EXECUTE FUNCTION prevent_posted_journal_update();

-- ============================================================
-- TRIGGER 2: Hard-closed periods reject all new journal lines
-- ============================================================
CREATE OR REPLACE FUNCTION prevent_hardclosed_period_insert()
RETURNS TRIGGER AS $$
DECLARE
    period_status TEXT;
BEGIN
    SELECT fp.status INTO period_status
    FROM fiscal_periods fp
    JOIN journal_entries je ON je.period_id = fp.id
    WHERE je.id = NEW.entry_id;

    IF period_status = 'hard_closed' THEN
        RAISE EXCEPTION 'Cannot insert journal lines into a hard-closed period (entry: %).', NEW.entry_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_prevent_hardclosed_insert
    BEFORE INSERT ON journal_lines
    FOR EACH ROW
    EXECUTE FUNCTION prevent_hardclosed_period_insert();

-- ============================================================
-- TRIGGER 3: Journal lines on posted entries cannot be modified
-- ============================================================
CREATE OR REPLACE FUNCTION prevent_posted_line_update()
RETURNS TRIGGER AS $$
DECLARE
    entry_status TEXT;
BEGIN
    SELECT status INTO entry_status FROM journal_entries WHERE id = OLD.entry_id;
    IF entry_status = 'posted' THEN
        RAISE EXCEPTION 'Cannot modify lines on a posted journal entry (entry: %).', OLD.entry_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_prevent_posted_line_update
    BEFORE UPDATE ON journal_lines
    FOR EACH ROW
    EXECUTE FUNCTION prevent_posted_line_update();

-- ============================================================
-- TRIGGER 4: Journal lines on posted entries cannot be deleted
-- ============================================================
CREATE OR REPLACE FUNCTION prevent_posted_line_delete()
RETURNS TRIGGER AS $$
DECLARE
    entry_status TEXT;
BEGIN
    SELECT status INTO entry_status FROM journal_entries WHERE id = OLD.entry_id;
    IF entry_status = 'posted' THEN
        RAISE EXCEPTION 'Cannot delete lines from a posted journal entry (entry: %).', OLD.entry_id;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_prevent_posted_line_delete
    BEFORE DELETE ON journal_lines
    FOR EACH ROW
    EXECUTE FUNCTION prevent_posted_line_delete();

-- ============================================================
-- TRIGGER 5: Audit event on every posted journal entry
-- ============================================================
CREATE OR REPLACE FUNCTION audit_journal_post()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status = 'posted' AND (OLD.status IS NULL OR OLD.status != 'posted') THEN
        INSERT INTO audit_events (entity_id, event_type, object_type, object_id, actor, after_state, timestamp)
        VALUES (NEW.entity_id, 'posted', 'journal_entry', NEW.id, NEW.created_by, 
                jsonb_build_object('number', NEW.number, 'date', NEW.date, 'reference', NEW.reference),
                NOW());
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_audit_journal_post
    AFTER INSERT OR UPDATE ON journal_entries
    FOR EACH ROW
    EXECUTE FUNCTION audit_journal_post();

-- ============================================================
-- TRIGGER 6: Non-negative inventory enforcement
-- ============================================================
CREATE OR REPLACE FUNCTION enforce_nonneg_inventory()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.on_hand < 0 THEN
        RAISE EXCEPTION 'Inventory item % (%) cannot go below zero. Current: %, Attempted: %',
            NEW.sku, NEW.id, OLD.on_hand, NEW.on_hand;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_enforce_nonneg_inventory
    BEFORE UPDATE ON inventory_items
    FOR EACH ROW
    EXECUTE FUNCTION enforce_nonneg_inventory();
