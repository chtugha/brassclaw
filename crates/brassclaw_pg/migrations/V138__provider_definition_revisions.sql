-- Provider configuration history is not component approval or dispatch authority.
-- Existing installation/operator definitions are preserved as revision 1.
ALTER TABLE brassclaw_llm_providers
    ADD COLUMN IF NOT EXISTS revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0);

CREATE TABLE IF NOT EXISTS brassclaw_llm_provider_revisions (
    tenant_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    definition JSONB NOT NULL,
    is_builtin BOOLEAN NOT NULL,
    deleted_at TIMESTAMPTZ,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, provider_id, revision),
    FOREIGN KEY (tenant_id, provider_id)
        REFERENCES brassclaw_llm_providers (tenant_id, id) ON DELETE RESTRICT
);

INSERT INTO brassclaw_llm_provider_revisions
    (tenant_id, provider_id, revision, definition, is_builtin, deleted_at)
SELECT tenant_id, id, revision, definition, is_builtin, deleted_at
FROM brassclaw_llm_providers
ON CONFLICT (tenant_id, provider_id, revision) DO NOTHING;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM brassclaw_llm_providers p
        JOIN brassclaw_llm_provider_revisions r
          ON r.tenant_id=p.tenant_id AND r.provider_id=p.id AND r.revision=p.revision
        WHERE r.definition IS DISTINCT FROM p.definition
           OR r.is_builtin IS DISTINCT FROM p.is_builtin
           OR r.deleted_at IS DISTINCT FROM p.deleted_at
    ) THEN
        RAISE EXCEPTION 'provider bootstrap history differs from retained configuration' USING ERRCODE='23514';
    END IF;
END;
$$;

CREATE FUNCTION assign_brassclaw_provider_revision() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.revision <> 1 THEN
            RAISE EXCEPTION 'new provider revision must start at one' USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id OR NEW.id IS DISTINCT FROM OLD.id THEN
        RAISE EXCEPTION 'provider identity cannot change' USING ERRCODE = '23514';
    END IF;
    IF NEW.revision IS DISTINCT FROM OLD.revision THEN
        RAISE EXCEPTION 'provider revision is assigned by storage' USING ERRCODE = '23514';
    END IF;
    IF NEW.definition IS DISTINCT FROM OLD.definition
       OR NEW.is_builtin IS DISTINCT FROM OLD.is_builtin
       OR NEW.deleted_at IS DISTINCT FROM OLD.deleted_at THEN
        IF OLD.revision = 9223372036854775807 THEN
            RAISE EXCEPTION 'provider revision exhausted' USING ERRCODE = '22003';
        END IF;
        NEW.revision := OLD.revision + 1;
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION record_brassclaw_provider_revision() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND NEW.revision = OLD.revision THEN
        RETURN NEW;
    END IF;
    -- Bind recording to the triggering table's schema, not caller search_path.
    EXECUTE format('INSERT INTO %I.brassclaw_llm_provider_revisions
        (tenant_id, provider_id, revision, definition, is_builtin, deleted_at)
        VALUES ($1, $2, $3, $4, $5, $6)', TG_TABLE_SCHEMA)
        USING NEW.tenant_id, NEW.id, NEW.revision, NEW.definition, NEW.is_builtin, NEW.deleted_at;
    RETURN NEW;
END;
$$;

CREATE FUNCTION protect_brassclaw_provider_revision_history() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'provider revision history is immutable' USING ERRCODE = '42501';
END;
$$;

CREATE TRIGGER brassclaw_provider_revision_assignment
    BEFORE INSERT OR UPDATE ON brassclaw_llm_providers
    FOR EACH ROW EXECUTE FUNCTION assign_brassclaw_provider_revision();
CREATE TRIGGER brassclaw_provider_revision_recording
    AFTER INSERT OR UPDATE ON brassclaw_llm_providers
    FOR EACH ROW EXECUTE FUNCTION record_brassclaw_provider_revision();
CREATE TRIGGER brassclaw_provider_revision_history_immutable
    BEFORE UPDATE OR DELETE ON brassclaw_llm_provider_revisions
    FOR EACH ROW EXECUTE FUNCTION protect_brassclaw_provider_revision_history();
CREATE TRIGGER brassclaw_provider_revision_history_no_truncate
    BEFORE TRUNCATE ON brassclaw_llm_provider_revisions
    FOR EACH STATEMENT EXECUTE FUNCTION protect_brassclaw_provider_revision_history();
