-- A global Tool policy addresses a class-0 Tool, not an arbitrary component.
-- Keep old policy/revision/override values intact. Existing malformed identities
-- fail migration visibly; do not drop their evidence or manufacture a mapping.
ALTER TABLE brassclaw_instance_tool_settings
    ADD COLUMN class_code SMALLINT NOT NULL DEFAULT 0 CHECK (class_code = 0),
    ADD CONSTRAINT instance_tool_setting_class_identity
        FOREIGN KEY (tool_id, class_code)
        REFERENCES reborn_component_revision_heads(component_id, class_code);
