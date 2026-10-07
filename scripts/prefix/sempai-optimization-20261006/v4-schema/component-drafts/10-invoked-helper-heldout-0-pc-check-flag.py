def check_flag(raw):
    return raw is True or raw is False
result = {'valid': False, 'flag': None}
if check_flag(inputs.get('flag')):
    result = {'valid': True, 'flag': inputs['flag']}