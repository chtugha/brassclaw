def classify_offset(value):
    if value is None:
        return {'accepted': False, 'offset': None}
    if type(value) is bool:
        return {'accepted': False, 'offset': None}
    if type(value) is not int:
        return {'accepted': False, 'offset': None}
    if not -4 <= value <= 5:
        return {'accepted': False, 'offset': None}
    return {'accepted': True, 'offset': value}
result = classify_offset(inputs.get('offset'))