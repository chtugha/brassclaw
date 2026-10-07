def check_mode(mode):
    accepted = type(mode) is str and mode in ('eco', 'boost')
    return {'accepted': accepted, 'mode': mode if accepted else None}
result = check_mode(inputs.get('mode'))