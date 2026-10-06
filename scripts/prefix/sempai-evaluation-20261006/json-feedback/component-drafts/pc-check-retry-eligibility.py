operation_state = inputs["operation_state"]
outcome = inputs["outcome"]
attempt = inputs["attempt"]
max_attempts = inputs["max_attempts"]
read_only = inputs["read_only"]
dedup_verified = inputs["dedup_verified"]

retry = (
    outcome == "retryable"
    and operation_state != "completed"
    and isinstance(attempt, int) and not isinstance(attempt, bool)
    and isinstance(max_attempts, int) and not isinstance(max_attempts, bool)
    and 0 < attempt < max_attempts
    and (read_only is True or dedup_verified is True)
)

result = {"retry": bool(retry)}