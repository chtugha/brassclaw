-- V060: Global token-budget kill switch (§0.21 / Phase O).
--
-- Adds `token_budgets_enabled` to `reborn_monty_vm_settings`.  Existing rows
-- backfill to `true` (today's behaviour).  Additive only; no DROP, no rename.
-- Depends on V034 (reborn_monty_vm_settings table, already live).

ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN token_budgets_enabled BOOLEAN NOT NULL DEFAULT true;
