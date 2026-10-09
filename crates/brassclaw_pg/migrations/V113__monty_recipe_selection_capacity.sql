-- Preserve the existing capacity while making it a live operator setting.
-- Adding the default changes neither existing settings values nor revisions.
ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN max_recipes_per_task INT NOT NULL DEFAULT 8
    CHECK (max_recipes_per_task > 0);
