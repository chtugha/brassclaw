-- Preserve all old values: provenance cannot distinguish an old default from
-- an explicit operator selection. New rows use conservative startup sizing.
ALTER TABLE reborn_monty_vm_settings ADD COLUMN memory_policy JSONB NOT NULL
    DEFAULT '{"mode":"startup","reserve_bytes":536870912,"sample_interval_secs":60,"growth_step_bytes":67108864,"growth_headroom_bytes":67108864}'::jsonb;
UPDATE reborn_monty_vm_settings SET
    memory_policy = jsonb_set(memory_policy, '{mode}', '"manual"'::jsonb),
    revision = revision + 1;
ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN max_memory_bytes SET DEFAULT 536870912,
    ADD CONSTRAINT reborn_monty_memory_policy_object CHECK (jsonb_typeof(memory_policy) = 'object');
