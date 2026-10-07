def check_mode(mode):
    accepted = isinstance(mode, str) and mode in ('eco', 'boost')
    return {'accepted': accepted, 'mode': mode if accepted else None}
result = check_mode(inputs.get('mode'))