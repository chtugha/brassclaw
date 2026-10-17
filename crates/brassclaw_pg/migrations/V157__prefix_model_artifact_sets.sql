-- Original model/tokenizer/template bytes, retained once and shared across
-- model specifications and prefix builds. An import is not serving identity,
-- accepted design, cache qualification or activation evidence.
CREATE TABLE reborn_prefix_model_artifact_sets (
    artifact_set_id TEXT PRIMARY KEY CHECK (artifact_set_id ~ '^[0-9a-f]{64}$'),
    artifact_set_bytes TEXT NOT NULL CHECK (octet_length(artifact_set_bytes) BETWEEN 1 AND 67108864),
    CHECK (artifact_set_id=encode(sha256(convert_to(artifact_set_bytes,'UTF8')),'hex')),
    inventory_bytes TEXT NOT NULL CHECK (octet_length(inventory_bytes) BETWEEN 1 AND 16384),
    inventory_checksum TEXT NOT NULL CHECK (inventory_checksum=encode(sha256(convert_to(inventory_bytes,'UTF8')),'hex')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER protect_prefix_model_artifact_set BEFORE UPDATE OR DELETE OR TRUNCATE
    ON reborn_prefix_model_artifact_sets FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();
CREATE TABLE reborn_prefix_model_artifact_bindings (
    specification_id TEXT NOT NULL REFERENCES reborn_prefix_model_specifications(specification_id),
    artifact_set_id TEXT NOT NULL REFERENCES reborn_prefix_model_artifact_sets(artifact_set_id),
    binding_bytes TEXT NOT NULL CHECK (octet_length(binding_bytes) BETWEEN 1 AND 16384),
    checksum TEXT NOT NULL CHECK (checksum=encode(sha256(convert_to(binding_bytes,'UTF8')),'hex')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(specification_id,artifact_set_id)
);
CREATE TRIGGER protect_prefix_model_artifact_binding BEFORE UPDATE OR DELETE OR TRUNCATE
    ON reborn_prefix_model_artifact_bindings FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();
