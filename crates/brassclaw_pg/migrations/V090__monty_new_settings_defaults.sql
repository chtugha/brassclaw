-- Align defaults for newly created settings. Existing rows can contain an
-- explicit operator choice; never infer that an old default was unedited.
-- Historical V034/V060 files and their recorded checksums stay unchanged.
ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN max_duration_secs SET DEFAULT 600,
    ALTER COLUMN token_budgets_enabled SET DEFAULT false;
