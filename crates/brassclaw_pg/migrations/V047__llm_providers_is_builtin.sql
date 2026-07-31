-- V047: Add is_builtin column to brassclaw_llm_providers.
--
-- is_builtin = TRUE means the row was seeded from the compiled-in providers.json
-- and is immutable via the normal operator upsert path.
-- Builtin rows may not be deleted (only configured/reset).
--
-- Back-fill: any pre-existing row whose id matches a known builtin id is marked
-- so that V048 seeding does not create a duplicate and so the delete guard works
-- correctly on upgrade.  The id list must match crates/brassclaw_llm/providers.json.
-- New builtins added in future binary versions are seeded by seed_builtin_providers()
-- at boot (in Rust) and receive is_builtin = TRUE at that point.

ALTER TABLE brassclaw_llm_providers
    ADD COLUMN IF NOT EXISTS is_builtin BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE brassclaw_llm_providers
SET    is_builtin = TRUE
WHERE  id IN (
    'nearai', 'gemini_oauth', 'openai_codex', 'openai', 'anthropic',
    'ollama', 'groq', 'bedrock', 'openai_compatible', 'tinfoil', 'deepseek',
    'github_copilot', 'gemini'
)
AND    deleted_at IS NULL;
