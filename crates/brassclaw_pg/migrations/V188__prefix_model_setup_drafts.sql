-- Unaccepted setup drafts belong to the existing model/design owner. These
-- append-only edits never replace Provider definitions, specifications or Q2.
CREATE TABLE reborn_prefix_model_setup_drafts (
    specification_id TEXT NOT NULL REFERENCES reborn_prefix_model_specifications(specification_id),
    operation_id UUID NOT NULL CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'::uuid),
    revision BIGINT NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    base_revision BIGINT NOT NULL CHECK (base_revision >= 0 AND revision=base_revision+1),
    predecessor BIGINT GENERATED ALWAYS AS (NULLIF(base_revision,0)) STORED,
    draft_bytes TEXT NOT NULL CHECK (octet_length(draft_bytes) BETWEEN 1 AND 1048576),
    draft_checksum TEXT NOT NULL CHECK (draft_checksum=encode(sha256(convert_to(draft_bytes,'UTF8')),'hex')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(specification_id,revision),
    UNIQUE(specification_id,operation_id),
    FOREIGN KEY(specification_id,predecessor) REFERENCES reborn_prefix_model_setup_drafts(specification_id,revision)
);
CREATE TRIGGER protect_prefix_model_setup_draft BEFORE UPDATE OR DELETE OR TRUNCATE
    ON reborn_prefix_model_setup_drafts FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();
