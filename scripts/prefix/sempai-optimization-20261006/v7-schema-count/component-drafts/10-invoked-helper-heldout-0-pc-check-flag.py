def check_flag(candidate):
    return {'valid': type(candidate) is bool, 'flag': candidate if type(candidate) is bool else None}
result = check_flag(inputs.get('flag'))