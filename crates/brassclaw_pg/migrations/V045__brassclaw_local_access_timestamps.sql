-- V045__brassclaw_local_access_timestamps.sql
--
-- Fix V021: brassclaw_local_access.created_at and updated_at were created as
-- TEXT (RFC-3339 formatted strings) to match the pre-migration trigger_records
-- schema.  All other tables use TIMESTAMPTZ.  This migration converts them and
-- adds the set_updated_at() trigger that was also missing.
--
-- The USING clause safely casts the existing RFC-3339 text values to TIMESTAMPTZ.
-- ON fresh deployments the columns already have the TEXT type from V021 and this
-- migration converts them correctly.

ALTER TABLE brassclaw_local_access
    ALTER COLUMN created_at TYPE TIMESTAMPTZ
    USING created_at::TIMESTAMPTZ;

ALTER TABLE brassclaw_local_access
    ALTER COLUMN updated_at TYPE TIMESTAMPTZ
    USING updated_at::TIMESTAMPTZ;

-- Add updated_at trigger (also absent from V021).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger t
        JOIN pg_class c ON c.oid = t.tgrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE t.tgname = 'brassclaw_local_access_updated_at'
          AND c.relname = 'brassclaw_local_access'
          AND n.nspname = current_schema()
    ) THEN
        CREATE TRIGGER brassclaw_local_access_updated_at
            BEFORE UPDATE ON brassclaw_local_access
            FOR EACH ROW EXECUTE FUNCTION set_updated_at();
    END IF;
END $$;
