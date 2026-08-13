-- V049__session_threads_version.sql
-- Add optimistic CAS version counter to brassclaw_session_threads.
-- PgSessionThreadService uses this for write_snapshot conflict detection.
-- Existing rows receive version = 0 (no live state to protect; safe default).
ALTER TABLE brassclaw_session_threads
    ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 0;
