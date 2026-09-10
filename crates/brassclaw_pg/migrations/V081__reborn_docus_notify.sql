-- V081__reborn_docus_notify.sql
-- Add a NOTIFY trigger to reborn_docus so Postgres pushes a change signal
-- to listening composition processes when a Docu row is inserted or updated.
--
-- Channel: 'reborn_docus_changed'
-- Payload: the doc's slug (name column).
--
-- The composition layer opens a dedicated non-pooled tokio_postgres connection,
-- issues LISTEN reborn_docus_changed, and on each notification upserts a
-- one-shot TriggerRecord ('doc-sync::docus-change') so the trigger poller
-- fires the 'run doc-sync' prompt and the orchestrator routes it to the
-- doc-sync Action. See brassclaw_reborn_composition::doc_sync_watcher.
--
-- Why AFTER and not BEFORE: AFTER INSERT OR UPDATE fires after the row is
-- durable on disk — the LISTEN consumer can safely read the new row.

CREATE OR REPLACE FUNCTION reborn_docus_notify_fn()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('reborn_docus_changed', NEW.name);
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS reborn_docus_notify_tg ON reborn_docus;
CREATE TRIGGER reborn_docus_notify_tg
    AFTER INSERT OR UPDATE ON reborn_docus
    FOR EACH ROW EXECUTE FUNCTION reborn_docus_notify_fn();
