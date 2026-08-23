-- V065__drop_session_threads_mission_id.sql
-- Drops the dead `mission_id TEXT` column from brassclaw_session_threads.
-- Why: the mission system was removed in v3 H.5 obsolescence cleanup (engine
-- mission system deleted; brassclaw_host_api::MissionId purged). The column was
-- always NULL in practice (missions were never live-active) and no Rust code
-- reads/writes it after the O2.3 purge (pg_service.sql no longer references it).
ALTER TABLE brassclaw_session_threads DROP COLUMN IF EXISTS mission_id;
