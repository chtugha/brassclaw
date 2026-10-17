-- Keep immutable workflow metadata at its existing bound. Typed invocation
-- arguments/answers use the existing 64 MiB execution/transport data ceiling,
-- rather than the unrelated 8 MiB component document limit. Never truncate.
-- Identity, fencing, hashes, attempt count and immutable answered effects remain.
ALTER TABLE brassclaw_monty_tool_invocations
    DROP CONSTRAINT brassclaw_monty_tool_invocations_arguments_bytes_check,
    DROP CONSTRAINT brassclaw_monty_tool_invocations_answer_bytes_check;
ALTER TABLE brassclaw_monty_tool_invocations
    ADD CONSTRAINT brassclaw_monty_tool_invocations_arguments_bytes_check
        CHECK (octet_length(arguments_bytes) BETWEEN 1 AND 67108864),
    ADD CONSTRAINT brassclaw_monty_tool_invocations_answer_bytes_check
        CHECK (octet_length(answer_bytes) BETWEEN 1 AND 67108864);
