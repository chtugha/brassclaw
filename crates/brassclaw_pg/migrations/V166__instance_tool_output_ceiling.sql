-- Optional instance-wide result ceiling, independent of component versions.
-- NULL preserves the previous behavior. No policy/revision/override is reset.
-- Unconstrained NUMERIC avoids rounding fractions before validation and carries
-- the full unsigned host byte representation without signed-BIGINT truncation.
ALTER TABLE brassclaw_instance_tool_settings
    ADD COLUMN max_output_bytes NUMERIC,
    ADD CONSTRAINT instance_tool_output_bytes CHECK (
        max_output_bytes IS NULL OR (
            max_output_bytes > 0
            AND max_output_bytes <= 18446744073709551615
            AND scale(max_output_bytes) = 0
        )
    );
