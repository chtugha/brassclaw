-- Preserve the admitted typed-value policy with the effect intent. Historical
-- records retain the previously enforced profile. Settings edits never change
-- an in-flight or completed invocation's bounds; the existing immutable trigger
-- covers these columns without granting dispatch/replay rights.
ALTER TABLE brassclaw_monty_tool_invocations
    ADD COLUMN data_max_depth BIGINT NOT NULL DEFAULT 64 CHECK (data_max_depth > 0),
    ADD COLUMN data_max_nodes BIGINT NOT NULL DEFAULT 262144 CHECK (data_max_nodes > 0),
    ADD COLUMN data_max_bytes BIGINT NOT NULL DEFAULT 67108864 CHECK (data_max_bytes > 0),
    DROP CONSTRAINT brassclaw_monty_tool_invocations_arguments_bytes_check,
    DROP CONSTRAINT brassclaw_monty_tool_invocations_answer_bytes_check;
ALTER TABLE brassclaw_monty_tool_invocations
    ADD CONSTRAINT brassclaw_monty_tool_invocations_arguments_bytes_check
        CHECK (octet_length(arguments_bytes) BETWEEN 1 AND data_max_bytes),
    ADD CONSTRAINT brassclaw_monty_tool_invocations_answer_bytes_check
        CHECK (octet_length(answer_bytes) BETWEEN 1 AND data_max_bytes);
