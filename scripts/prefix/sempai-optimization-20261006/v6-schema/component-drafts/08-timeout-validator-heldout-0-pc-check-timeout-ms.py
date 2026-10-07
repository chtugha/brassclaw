def check_timeout_ms(candidate):
    if type(candidate) is bool or not type(candidate) is int or not 0 <= candidate <= 7500:
        return {'valid': False, 'timeout_ms': None}
    return {'valid': True, 'timeout_ms': candidate}
result = check_timeout_ms(inputs.get('timeout_ms'))