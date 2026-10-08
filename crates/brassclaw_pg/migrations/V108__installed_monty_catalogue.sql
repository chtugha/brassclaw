-- Installation qualification is separate from authored/human-Q2 receipts.
-- Exact package generations remain available to admitted/running tasks.
CREATE TABLE IF NOT EXISTS brassclaw_monty_boot_catalogues (
    catalogue_id uuid PRIMARY KEY,
    catalogue_bytes text NOT NULL CHECK (octet_length(catalogue_bytes) <= 67108864),
    checksum text NOT NULL CHECK (checksum = encode(sha256(convert_to(catalogue_bytes, 'UTF8')), 'hex')),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE FUNCTION brassclaw_protect_monty_boot_catalogue() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'installation catalogue generations are immutable' USING ERRCODE='23514';
END $$;
CREATE TRIGGER protect_monty_boot_catalogue BEFORE UPDATE OR DELETE OR TRUNCATE
    ON brassclaw_monty_boot_catalogues FOR EACH STATEMENT EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();

CREATE TABLE IF NOT EXISTS brassclaw_instance_tool_settings (
    tool_id uuid PRIMARY KEY REFERENCES reborn_component_revision_heads(component_id),
    enabled boolean NOT NULL DEFAULT true,
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0)
);
CREATE FUNCTION brassclaw_check_instance_tool_settings_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.revision=9223372036854775807 OR NEW.revision<>OLD.revision+1 OR NEW.tool_id<>OLD.tool_id THEN
        RAISE EXCEPTION 'instance Tool setting requires its next revision' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER check_instance_tool_settings_revision BEFORE UPDATE ON brassclaw_instance_tool_settings
    FOR EACH ROW EXECUTE FUNCTION brassclaw_check_instance_tool_settings_revision();

CREATE FUNCTION brassclaw_protect_instance_tool_settings() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'disable Tool settings without deleting their persistent revision' USING ERRCODE='23514';
END $$;
CREATE TRIGGER protect_instance_tool_settings BEFORE DELETE OR TRUNCATE ON brassclaw_instance_tool_settings
    FOR EACH STATEMENT EXECUTE FUNCTION brassclaw_protect_instance_tool_settings();
