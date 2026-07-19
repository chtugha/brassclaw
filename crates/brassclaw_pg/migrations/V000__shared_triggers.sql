-- V000__shared_triggers.sql
-- Runs first: installs pgvector extension and defines the shared set_updated_at()
-- trigger function used by all subsequent migrations.
--
-- pgvector must be installed before any migration that defines a 'vector' column.
-- For embedded-Postgres, brassclaw_embedded_postgres/src/initdb.rs bundles and
-- installs the pgvector shared library before this migration runs.
-- For external-Postgres operators, ensure pgvector is installed on the server.

CREATE EXTENSION IF NOT EXISTS vector;

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;
