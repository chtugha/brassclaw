def inspect_lag(value):
    accepted = type(value) is int and -1 <= value <= 8
    return {'accepted': accepted, 'lag': value if accepted else None}
result = inspect_lag(inputs.get('lag'))