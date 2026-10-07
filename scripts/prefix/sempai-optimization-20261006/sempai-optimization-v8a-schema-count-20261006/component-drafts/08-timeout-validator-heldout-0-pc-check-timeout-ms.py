def check_timeout_ms(inputs):
    timeout_ms = inputs.get('timeout_ms')
    valid = type(timeout_ms) is int and not isinstance(timeout_ms, bool) and 0 <= timeout_ms <= 7500
    result = {'valid': valid, 'timeout_ms': timeout_ms if valid else None}
result = check_timeout_ms(inputs)