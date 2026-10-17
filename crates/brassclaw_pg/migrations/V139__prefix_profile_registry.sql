-- Instance-wide profile metadata only. This is not compiler qualification,
-- source approval, a model-design store, an active selection or a job queue.
CREATE TABLE IF NOT EXISTS reborn_prefix_profiles (
    name TEXT PRIMARY KEY CHECK (name ~ '^[a-z][a-z0-9-]{0,63}$'),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    label TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 200),
    description TEXT NOT NULL CHECK (length(description) BETWEEN 1 AND 4000),
    compiler_entry TEXT CHECK (compiler_entry ~ '^[A-Za-z][A-Za-z0-9_-]*[.]py$'),
    sort_order INTEGER NOT NULL DEFAULT 100,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Metadata for shipped profiles. A filename never authorizes executing code.
-- Future installation upgrades must preserve operator edits; no upsert overwrite.
INSERT INTO reborn_prefix_profiles (name,label,description,compiler_entry,sort_order) VALUES
('base-prompt','BrassClaw Base Prompt','BrassClaw architecture, current documentation and approved components.','base-promt-compiler.py',0),
('defensive','Defensive / Servers and Networks','Linux, Debian, Ubuntu, firewalls, routers, defensive security and incident response.','Defensive-compiler.py',1),
('homeassistant','Home Assistant / MQTT / Modbus / YAML','Home Assistant configuration, automations, MQTT and Modbus with YAML authoring.','Homeassistant-compiler.py',2),
('tomedo','tomedo / API / macOS / PostgreSQL','tomedo integrations, REST API, macOS, PostgreSQL, GOÄ, EBM and coding; excludes clinical knowledge.','tomedo-compiler.py',3),
('sempai','Sempai / Prompt Review and Component Authoring','Prompt diagnosis and reusable BrassClaw component authoring with validation contracts.','sempai-compiler.py',4),
('prefix-maker','Prefix-Maker / Prefix Design','Evidence, formatting, model-specific token layout and cache-aware prefix design.',NULL,5)
ON CONFLICT (name) DO NOTHING;
