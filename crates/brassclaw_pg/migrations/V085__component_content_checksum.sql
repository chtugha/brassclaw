-- V085: add content_checksum (nullable SHA-256 hex) to prose-bearing component
-- tables. Populated by builtin_bootstrap seeder for source='system' rows.
-- NULL = not yet checked / user-authored row / JSONB-only table.
--
-- Scoped to the three prose-bearing tables whose source='system' rows carry
-- behavioural text verified by the boot content integrity check.
-- JSONB-primary tables (reborn_recipes, reborn_extension_catalogues,
-- reborn_actions, reborn_docus, etc.) and the reborn_tools descriptor table
-- are excluded — they carry no single seeded prose field to hash.
--
-- Note: reborn_python_code already has a content_hash column (for similarity
-- deduplication). The new content_checksum column is separate and serves the
-- boot integrity check only; the two columns are not interchangeable.
ALTER TABLE reborn_skills      ADD COLUMN IF NOT EXISTS content_checksum TEXT;
ALTER TABLE reborn_tool_skills ADD COLUMN IF NOT EXISTS content_checksum TEXT;
ALTER TABLE reborn_python_code ADD COLUMN IF NOT EXISTS content_checksum TEXT;
