-- Revision zero represents settings which predate revisioned edits. Preserve
-- all operator values. Each accepted patch advances the revision atomically.
ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0);
