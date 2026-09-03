-- V067__reborn_tools_cdylib_artifact_path.sql
-- Step C.3 — Two Tool Systems: cdylib artifact path on `reborn_tools`.
--
-- The Executioner (Rust) holds Two Tool Systems:
--   1. Built-in Tools — precompiled into the binary. For these rows
--      `cdylib_artifact_path` IS NULL (they are dispatched by the static
--      `match call.function_name` in the engine orchestrator).
--   2. Dynamic Tools — kohai/sempai-minted Tools+ToolSkills that ship as
--      separate `cdylib` crates. For these rows `cdylib_artifact_path` holds the
--      filesystem path the `DynamicToolLoader` (brassclaw_host_runtime) dlopens
--      at runtime on demand, bound into the `host` namespace by a recipe and
--      unloaded at main-process task end.
--
-- The column stores ONLY the path pointer — the cdylib artifact itself is never
-- persisted in the DB. Only Q2+ validated dynamic tools are runnable: a Q1
-- component is stuck in the validation queue and never reachable by Rust or the
-- Orchestrator, so a non-NULL path implies a trusted (Q2+) artifact.
--
-- Nullable TEXT; defaults to NULL (all current built-in seed rows stay NULL).
-- The dynamic-tool authoring surface (a later step) writes the path; the
-- composition-mechanism reads it when building cdylib load directives.

ALTER TABLE reborn_tools
    ADD COLUMN IF NOT EXISTS cdylib_artifact_path TEXT;

-- Partial index for the "which tools are dynamic / which path to dlopen" lookup
-- the composition-mechanism runs when building cdylib load directives.
CREATE INDEX IF NOT EXISTS reborn_tools_cdylib_path_idx
    ON reborn_tools (tenant_id, user_id, agent_id, project_id, name)
    WHERE cdylib_artifact_path IS NOT NULL;
