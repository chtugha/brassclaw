def classify_lag(lag):
    valid = type(lag) is int and 5 <= lag <= 14
    return {'kind': 'integer' if valid else 'invalid', 'lag': lag if valid else None}
if 'lag' in inputs:
    result = classify_lag(inputs['lag'])
else:
    result = {'kind': 'missing', 'lag': None}