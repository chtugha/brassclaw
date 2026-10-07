def check_flag(candidate):
    is_exact_bool = type(candidate) is bool
    return {'valid': is_exact_bool, 'flag': candidate if is_exact_bool else None}
result = check_flag(inputs.get('flag'))