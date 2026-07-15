-- V33: Sempai–Kohai schema marker.
--
-- This migration records that the database has been updated to support the
-- Sempai–Kohai dual-role provider architecture. No schema changes are needed
-- in the v1 settings table: the new llm.kohai_provider and llm.sempai_provider
-- keys follow the existing per-user (user_id, key) convention and are written
-- at runtime via the normal settings API — no seed rows are required.
--
-- A no-op DDL statement is used so refinery records this migration in its
-- schema_migrations table without requiring any data writes.

DO $$ BEGIN
  -- intentional no-op: schema version marker only
  NULL;
END $$;
