-- V083: add generation_ms to reborn_basic_prompt_store.
--
-- Records how many milliseconds the bundle assembly took on the last run.
-- NULL for rows written before this migration (no back-fill needed).
-- The WebUI Prefix Tab displays this as "Total Generation Time: X minutes".

ALTER TABLE reborn_basic_prompt_store
    ADD COLUMN IF NOT EXISTS generation_ms BIGINT;
