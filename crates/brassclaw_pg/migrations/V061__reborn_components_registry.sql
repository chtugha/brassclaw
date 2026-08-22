--
-- Component-class registry (FIND-IBS-02 resolution, Phase E).
--
-- A flat UUID -> class_code + scope lookup, one row per component across all
-- 14 class tables. The IBS emits `IbsRecipeStep.include: Vec<Uuid>` per recipe
-- step with NO per-UUID class_code ("UUIDs are opaque to the IBS" —
-- instruction_builder.rs:135/:593), but `fetch_component_by_id` needs the
-- class_code to pick the class-specific table. No per-UUID class source
-- existed in the data model (`reborn_intent_inputs` holds intent-matched
-- components, not recipe-step includes). Per user decision (Q1->C then Q-F1->B)
-- this registry resolves each step UUID's class via one indexed scoped lookup.
--
-- Kept in sync by AFTER INSERT OR UPDATE OR DELETE row triggers on every class
-- table, so it is always a faithful mirror of the 14 source tables. The trigger
-- function is GENERIC: it reads `NEW.class_code` directly (every class table
-- exposes `class_code` — verified by `fetch_component_by_id`'s parameterised
-- `SELECT class_code::int FROM {table}`), so multi-class tables
-- (`reborn_skills` -> 1/2/3/10/50, `reborn_extensions_unified` -> 4-9) are
-- handled correctly without a per-table class argument.
--
-- Scope columns are denormalized onto the registry so the lookup enforces
-- SEC-01 tenant isolation: a foreign-tenant UUID never resolves to a class
-- (the scoped WHERE clause simply returns no row).
--
-- This is a schema change beyond Phase E's original "no migration" scope. It
-- is an explicit upgrade accepted per the task rule "do not blindly remove
-- upgrades; document, repair, complete or leave them". See
-- `docs/agents-v3/subplan_problem_stepE_of_saved_plan_to_v3.md` §3.
--
-- Additive only — no DROP of existing objects, no renames, no existing rows
-- break. All statements are idempotent (CREATE TABLE IF NOT EXISTS,
-- CREATE OR REPLACE FUNCTION, DROP TRIGGER IF EXISTS + CREATE TRIGGER) so the
-- migration is safe to apply on a partially-migrated schema and re-runnable.

-- ---------------------------------------------------------------------------
-- 1. Registry table
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS reborn_components (
    id          UUID        NOT NULL,
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    agent_id    TEXT        NOT NULL,
    project_id  TEXT        NOT NULL,
    class_code  INT         NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id)
);

-- Scoped class filter (the lookup path). The PK on `id` already gives the
-- per-UUID indexed probe; this index supports scoped class-list queries and
-- operational visibility.
CREATE INDEX IF NOT EXISTS reborn_components_scope_class_idx
    ON reborn_components (tenant_id, user_id, agent_id, project_id, class_code);

-- ---------------------------------------------------------------------------
-- 2. Generic trigger function
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION maintain_components_registry() RETURNS trigger AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        INSERT INTO reborn_components
            (id, tenant_id, user_id, agent_id, project_id, class_code, created_at, updated_at)
        VALUES
            (NEW.id, NEW.tenant_id, NEW.user_id, NEW.agent_id, NEW.project_id, NEW.class_code::int, now(), now())
        ON CONFLICT (id) DO UPDATE SET
            tenant_id  = EXCLUDED.tenant_id,
            user_id    = EXCLUDED.user_id,
            agent_id   = EXCLUDED.agent_id,
            project_id = EXCLUDED.project_id,
            class_code = EXCLUDED.class_code,
            updated_at = now();
        RETURN NEW;
    ELSIF (TG_OP = 'UPDATE') THEN
        UPDATE reborn_components SET
            tenant_id  = NEW.tenant_id,
            user_id    = NEW.user_id,
            agent_id   = NEW.agent_id,
            project_id = NEW.project_id,
            class_code = NEW.class_code::int,
            updated_at = now()
        WHERE id = NEW.id;
        RETURN NEW;
    ELSIF (TG_OP = 'DELETE') THEN
        DELETE FROM reborn_components WHERE id = OLD.id;
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- ---------------------------------------------------------------------------
-- 3. Trigger attachments — one multi-event trigger per class table (14).
--    Trigger names are scoped to their table in Postgres, so 14 identically-
--    named `maintain_components_registry` triggers coexist. DROP IF EXISTS +
--    CREATE keeps the migration re-runnable.
-- ---------------------------------------------------------------------------

-- reborn_skills (classes 1-3, 10, 50)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_skills;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_skills
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_extensions_unified (classes 4-9)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_extensions_unified;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_extensions_unified
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_actions (class 16)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_actions;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_actions
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_specs (class 12)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_specs;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_specs
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_tool_skills (class 13)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_tool_skills;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_tool_skills
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_plans (class 14)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_plans;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_plans
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_summaries (class 15)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_summaries;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_summaries
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_docus (class 17)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_docus;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_docus
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_lessons (class 18)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_lessons;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_lessons
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_issues (class 19)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_issues;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_issues
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_notes (class 20)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_notes;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_notes
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_recipes (class 21)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_recipes;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_recipes
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_python_code (class 22)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_python_code;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_python_code
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- reborn_extension_catalogues (class 23)
DROP TRIGGER IF EXISTS maintain_components_registry ON reborn_extension_catalogues;
CREATE TRIGGER maintain_components_registry
    AFTER INSERT OR UPDATE OR DELETE ON reborn_extension_catalogues
    FOR EACH ROW EXECUTE FUNCTION maintain_components_registry();

-- ---------------------------------------------------------------------------
-- 4. Backfill — seed the registry from existing rows across all 14 tables.
--    Registry timestamps use now() (the registry is a derived lookup index;
--    its timestamps record "when the registry row was last touched", which is
--    also what the trigger writes — so now() on backfill is consistent and
--    avoids depending on any source table exposing created_at/updated_at).
--    ON CONFLICT (id) DO NOTHING preserves trigger-maintained rows.
-- ---------------------------------------------------------------------------

INSERT INTO reborn_components (id, tenant_id, user_id, agent_id, project_id, class_code, created_at, updated_at)
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_skills
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_extensions_unified
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_actions
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_specs
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_tool_skills
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_plans
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_summaries
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_docus
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_lessons
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_issues
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_notes
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_recipes
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_python_code
UNION ALL
SELECT id, tenant_id, user_id, agent_id, project_id, class_code::int, now(), now() FROM reborn_extension_catalogues
ON CONFLICT (id) DO NOTHING;
