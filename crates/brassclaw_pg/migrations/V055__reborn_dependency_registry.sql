-- V055__reborn_dependency_registry.sql
--
-- Phase J.2 — adds dependency_registry JSONB and formatted_content TEXT to all
-- component tables that participate in dependency traversal (§0.19) and
-- persisted LLM-formatted content (§0.23.4).
--
-- TABLES COVERED (13 class tables that hold authored components):
--   reborn_skills             (class 1–3,  V027)
--   reborn_tools              (class 0,    V030)
--   reborn_tool_skills        (class 13,   V037)
--   reborn_recipes            (class 21,   V033 — dependency_registry already
--                               from V050; IF NOT EXISTS → idempotent no-op)
--   reborn_actions            (class 16,   V029)
--   reborn_specs              (class 12,   V036)
--   reborn_plans              (class 14,   V038)
--   reborn_summaries          (class 15,   V039)
--   reborn_lessons            (class 18,   V041)
--   reborn_docus              (class 17,   V040)
--   reborn_issues             (class 19,   V042)
--   reborn_notes              (class 20,   V043)
--   reborn_extensions_unified (class 4–9,  V032 — EXT-NAME: bare "reborn_extensions"
--                               would fail; use the canonical table name)
--
-- NEW TABLES from Phases B/C (V052/V053) already have dependency_registry at
-- creation time — no ALTER needed. They DO need formatted_content (V052/V053
-- predate §0.23.4).
--   reborn_python_code          (class 22, V052 — dep_reg already; fc new)
--   reborn_extension_catalogues (class 23, V053 — dep_reg already; fc new)
--
-- NULLABILITY:
--   dependency_registry JSONB: nullable (NULL = no declared dependencies)
--   formatted_content   TEXT:  nullable (NULL = not yet formatted; computed
--                               at save time by the per-class formatter
--                               PythonCode, seeded in Phase L)
--
-- ALL statements use ADD COLUMN IF NOT EXISTS → idempotent across repeated
-- applies / schemas partially migrated by hand.

-- ─── dependency_registry ────────────────────────────────────────────────────
-- (reborn_python_code / reborn_extension_catalogues already have it from V052/V053)
-- (reborn_recipes already has it from V050 — IF NOT EXISTS → no-op)

ALTER TABLE reborn_skills             ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_tools              ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_tool_skills        ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_recipes            ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_actions            ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_specs              ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_plans              ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_summaries          ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_lessons            ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_docus              ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_issues             ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_notes              ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_extensions_unified ADD COLUMN IF NOT EXISTS dependency_registry JSONB;

-- ─── formatted_content ──────────────────────────────────────────────────────
-- Added to ALL 15 class tables (including V052/V053 which predate §0.23.4).
-- The per-class formatter PythonCode components are seeded in Phase L; until
-- then the column stays NULL. Re-computed on every content change at save time.

ALTER TABLE reborn_skills             ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_tools              ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_tool_skills        ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_recipes            ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_actions            ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_specs              ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_plans              ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_summaries          ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_lessons            ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_docus              ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_issues             ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_notes              ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_extensions_unified ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_python_code        ADD COLUMN IF NOT EXISTS formatted_content TEXT;
ALTER TABLE reborn_extension_catalogues ADD COLUMN IF NOT EXISTS formatted_content TEXT;
