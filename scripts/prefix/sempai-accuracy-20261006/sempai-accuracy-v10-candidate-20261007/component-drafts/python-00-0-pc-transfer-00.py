def inspect_offset(value):
    accepted = type(value) is int and not isinstance(value, bool) and -4 <= value <= 5
    return {'accepted': accepted, 'offset': value if accepted else None}
result = inspect_offset(inputs.get('offset'))