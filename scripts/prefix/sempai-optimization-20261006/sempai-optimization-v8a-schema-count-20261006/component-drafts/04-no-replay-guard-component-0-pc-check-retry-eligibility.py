def classify_state(state):
    return type(state) is str and state in ('completed', 'unknown', 'not_started')

def classify_outcome(outcome):
    return type(outcome) is str and outcome in ('retryable', 'terminal')

def classify_attempt(attempt):
    return type(attempt) is int and not isinstance(attempt, bool) and attempt >= 1

def classify_max(max_attempts):
    return type(max_attempts) is int and not isinstance(max_attempts, bool) and max_attempts >= 1

def classify_read_only(flag):
    return type(flag) is bool

def classify_dedup(flag):
    return type(flag) is bool

def check_retry_eligibility(operation_state, outcome, attempt, max_attempts, read_only, dedup_verified):
    state_ok = classify_state(operation_state)
    outcome_ok = classify_outcome(outcome)
    attempt_ok = classify_attempt(attempt)
    max_ok = classify_max(max_attempts)
    read_only_ok = classify_read_only(read_only)
    dedup_ok = classify_dedup(dedup_verified)
    valid = state_ok and outcome_ok and attempt_ok and max_ok and read_only_ok and dedup_ok
    if not valid:
        return {'retry': False}
    eligible = (outcome == 'retryable' and attempt < max_attempts and operation_state != 'completed' and (read_only or dedup_verified))
    return {'retry': eligible}

state = inputs.get('operation_state')
outcome = inputs.get('outcome')
attempt = inputs.get('attempt')
max_attempts = inputs.get('max_attempts')
read_only = inputs.get('read_only')
dedup_verified = inputs.get('dedup_verified')
result = check_retry_eligibility(state, outcome, attempt, max_attempts, read_only, dedup_verified)