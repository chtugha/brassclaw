def check_timeout_ms(inputs):
    timeout_ms = inputs.get('timeout_ms')
    valid = type(timeout_ms) is int and not isinstance(timeout_ms, bool) and 0 <= timeout_ms <= 7500
    if valid:
        result = {'valid': True, 'timeout_ms': timeout_ms}
    else:
        result = {'valid': False, 'timeout_ms': None}
    return result
result = check_timeout_ms(inputs)