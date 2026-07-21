-- V031: Per-class validation configuration.
--
-- One row per (scope, class_code). Immediate-write: changes take effect on the
-- next validation cycle and do NOT retroactively re-validate existing components.
-- Seeded with defaults matching the plan §3.5.2/§7 Q14 spec.

CREATE TABLE IF NOT EXISTS reborn_validation_config (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                   TEXT NOT NULL,
    user_id                     TEXT NOT NULL,
    agent_id                    TEXT NOT NULL,
    project_id                  TEXT NOT NULL,
    class_code                  SMALLINT NOT NULL,
    name_min_len                SMALLINT NOT NULL DEFAULT 1,
    name_max_len                SMALLINT NOT NULL DEFAULT 64,
    name_pattern                TEXT NOT NULL DEFAULT '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$',
    description_min_len         SMALLINT NOT NULL DEFAULT 1,
    description_max_len         SMALLINT NOT NULL DEFAULT 1024,
    -- NULL means no token budget enforced (e.g. Actions class 16).
    token_budget                INTEGER,
    token_budget_hard_error     BOOLEAN NOT NULL DEFAULT FALSE,
    require_tool_name           BOOLEAN NOT NULL DEFAULT FALSE,
    require_param_schema        BOOLEAN NOT NULL DEFAULT FALSE,
    require_activation_criteria BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT reborn_validation_config_scope_class_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, class_code)
);

CREATE INDEX IF NOT EXISTS reborn_validation_config_scope_idx
    ON reborn_validation_config (tenant_id, user_id, agent_id, project_id);

-- Seed global defaults using a synthetic system scope.
-- Host-level composition seeds per-user rows on first access via upsert;
-- these rows serve as the spec-compliant fallback.
--
-- class_code legend (from spec §4):
--   00 = Tool (Rusty)
--   01 = Skill (Rusty)
--   02 = Skill (Monty)
--   03 = Skill (LLM)
--   04-09 = Extensions
--   10 = Orchestrator
--   12-20 = Former DocType classes
--   16 = Action
--   21 = Recipe
--   50 = Scaffold

-- Skills (01-03): full agentskills.io validation
INSERT INTO reborn_validation_config
    (tenant_id, user_id, agent_id, project_id, class_code,
     name_min_len, name_max_len, name_pattern,
     description_min_len, description_max_len,
     token_budget, token_budget_hard_error,
     require_tool_name, require_param_schema, require_activation_criteria)
SELECT '__system__', '__system__', '__system__', '__system__', cls,
       1, 64, '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$',
       10, 1024,
       5000, TRUE,
       FALSE, FALSE, TRUE
FROM (VALUES (1::SMALLINT), (2::SMALLINT), (3::SMALLINT)) AS t(cls)
ON CONFLICT ON CONSTRAINT reborn_validation_config_scope_class_unique DO NOTHING;

-- Tool (00): tool_name + param_schema required
INSERT INTO reborn_validation_config
    (tenant_id, user_id, agent_id, project_id, class_code,
     name_min_len, description_min_len,
     token_budget, token_budget_hard_error,
     require_tool_name, require_param_schema, require_activation_criteria)
VALUES ('__system__', '__system__', '__system__', '__system__', 0,
        1, 1,
        5000, TRUE,
        TRUE, TRUE, FALSE)
ON CONFLICT ON CONSTRAINT reborn_validation_config_scope_class_unique DO NOTHING;

-- Extensions (04-09): soft token budget
INSERT INTO reborn_validation_config
    (tenant_id, user_id, agent_id, project_id, class_code,
     name_min_len, description_min_len,
     token_budget, token_budget_hard_error,
     require_tool_name, require_param_schema, require_activation_criteria)
SELECT '__system__', '__system__', '__system__', '__system__', cls,
       1, 1,
       10000, FALSE,
       FALSE, FALSE, FALSE
FROM (VALUES (4::SMALLINT),(5::SMALLINT),(6::SMALLINT),(7::SMALLINT),(8::SMALLINT),(9::SMALLINT)) AS t(cls)
ON CONFLICT ON CONSTRAINT reborn_validation_config_scope_class_unique DO NOTHING;

-- Orchestrator (10) + Scaffold (50): LLM audit, large budget, soft
INSERT INTO reborn_validation_config
    (tenant_id, user_id, agent_id, project_id, class_code,
     name_min_len, description_min_len,
     token_budget, token_budget_hard_error,
     require_tool_name, require_param_schema, require_activation_criteria)
SELECT '__system__', '__system__', '__system__', '__system__', cls,
       1, 1,
       50000, FALSE,
       FALSE, FALSE, FALSE
FROM (VALUES (10::SMALLINT),(50::SMALLINT)) AS t(cls)
ON CONFLICT ON CONSTRAINT reborn_validation_config_scope_class_unique DO NOTHING;

-- Actions (16): no token budget, no activation criteria
INSERT INTO reborn_validation_config
    (tenant_id, user_id, agent_id, project_id, class_code,
     name_min_len, description_min_len,
     token_budget, token_budget_hard_error,
     require_tool_name, require_param_schema, require_activation_criteria)
VALUES ('__system__', '__system__', '__system__', '__system__', 16,
        1, 1,
        NULL, FALSE,
        FALSE, FALSE, FALSE)
ON CONFLICT ON CONSTRAINT reborn_validation_config_scope_class_unique DO NOTHING;

-- Former DocType classes (12-15, 17-20): soft 10000 budget (Notes: 2000 for class 15)
INSERT INTO reborn_validation_config
    (tenant_id, user_id, agent_id, project_id, class_code,
     name_min_len, description_min_len,
     token_budget, token_budget_hard_error,
     require_tool_name, require_param_schema, require_activation_criteria)
SELECT '__system__', '__system__', '__system__', '__system__', cls,
       1, 1,
       CASE WHEN cls = 15 THEN 2000 ELSE 10000 END, FALSE,
       FALSE, FALSE, FALSE
FROM (VALUES (12::SMALLINT),(13::SMALLINT),(14::SMALLINT),(15::SMALLINT),
             (17::SMALLINT),(18::SMALLINT),(19::SMALLINT),(20::SMALLINT)) AS t(cls)
ON CONFLICT ON CONSTRAINT reborn_validation_config_scope_class_unique DO NOTHING;

-- Recipes (21): soft 10000 budget, requires activation criteria (trigger)
INSERT INTO reborn_validation_config
    (tenant_id, user_id, agent_id, project_id, class_code,
     name_min_len, description_min_len,
     token_budget, token_budget_hard_error,
     require_tool_name, require_param_schema, require_activation_criteria)
VALUES ('__system__', '__system__', '__system__', '__system__', 21,
        1, 1,
        10000, FALSE,
        FALSE, FALSE, TRUE)
ON CONFLICT ON CONSTRAINT reborn_validation_config_scope_class_unique DO NOTHING;
