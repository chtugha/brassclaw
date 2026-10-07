def check_flag(candidate):
    valid = type(candidate) is bool
    return {'valid': valid, 'flag': candidate if valid else None}
result = check_flag(inputs.get('flag'))