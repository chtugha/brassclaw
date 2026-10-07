operation_state = inputs.get('operation_state')
outcome = inputs.get('outcome')
attempt = inputs.get('attempt')
max_attempts = inputs.get('max_attempts')
read_only = inputs.get('read_only')
dedup_verified = inputs.get('dedup_verified')
valid = (
    type(operation_state) is str and operation_state in ('completed', 'unknown', 'not_started')
    and type(outcome) is str and outcome in ('retryable', 'terminal')
    and type(attempt) is int and not isinstance(attempt, bool) and attempt >= 1
    and type(max_attempts) is int and not isinstance(max_attempts, bool) and max_attempts >= 1
    and type(read_only) is bool
    and type(dedup_verified) is bool
)
retry = False
if valid:
    retry = (outcome == 'retryable' and attempt < max_attempts and operation_state != 'completed' and (read_only or dedup_verified))
result = {'retry': retry}