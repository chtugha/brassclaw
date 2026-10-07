def classify_offset(value):
    if 'offset' not in inputs:
        return {'kind': 'missing', 'offset': None}
    if value is None:
        return {'kind': 'null', 'offset': None}
    valid = type(value) is int and 2 <= value <= 11
    return {'kind': 'integer', 'offset': value} if valid else {'kind': 'invalid', 'offset': None}
result = classify_offset(inputs.get('offset'))