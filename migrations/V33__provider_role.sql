-- V33: Sempai–Kohai schema marker and new provider-role settings seeds.
--
-- The active provider selection is now split into two named slots:
--   llm.kohai_provider  — primary inference model (maps to former llm.active_provider)
--   llm.sempai_provider — audit/interception model (new; absent = passthrough mode)
--
-- Both Postgres and libSQL run this via Database::run_migrations().
-- The settings table is key-value; no schema column changes are needed.

INSERT INTO settings (key, value)
  VALUES ('schema.sempai_kohai_version', '"1"')
  ON CONFLICT DO NOTHING;
