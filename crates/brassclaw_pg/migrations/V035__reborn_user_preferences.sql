-- V035__reborn_user_preferences.sql
-- Per-user runtime preferences for BrassClaw Reborn (Phase 5 Step 5.1a2).
--
-- Simple key-value store for per-user UX preferences (spec §3.12, §7 Q18).
-- NOT exposed in the Settings UI — these are chat-surface runtime preferences.
-- Currently used for the "AI before User" flip switch (ai_before_user = true/false).
--
-- Hidden/disabled in DB-less mode (no intent system to fall back from).
--
-- Spec references: §3.12 rule f-ai, §7 Q18 (ai_before_user, default OFF,
-- per-user, not per-scope).

CREATE TABLE IF NOT EXISTS reborn_user_preferences (
    id              UUID        NOT NULL DEFAULT gen_random_uuid(),

    -- User-level scope (not full scope tuple — preferences are per-user).
    user_id         TEXT        NOT NULL,

    -- Preference key (e.g. 'ai_before_user').
    preference_key  TEXT        NOT NULL
        CHECK (length(preference_key) BETWEEN 1 AND 128),

    -- Preference value (boolean represented as 'true'/'false', or other text).
    preference_value TEXT       NOT NULL DEFAULT 'false',

    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT reborn_user_preferences_pk PRIMARY KEY (id),

    -- One value per (user_id, preference_key).
    CONSTRAINT reborn_user_preferences_user_key_unique
        UNIQUE (user_id, preference_key)
);

-- Index for fast per-user lookups.
CREATE INDEX IF NOT EXISTS reborn_user_preferences_user_idx
    ON reborn_user_preferences (user_id);

-- ── updated_at trigger ──────────────────────────────────────────────────────

CREATE TRIGGER reborn_user_preferences_updated_at
    BEFORE UPDATE ON reborn_user_preferences
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
