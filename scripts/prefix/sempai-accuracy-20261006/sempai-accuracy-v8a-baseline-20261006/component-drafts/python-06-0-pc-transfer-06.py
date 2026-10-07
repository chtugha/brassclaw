def classify_offset(raw):
    if 'offset' not in inputs:
        result = {'kind': 'missing', 'offset': None}
    else:
        offset = inputs['offset']
        if offset is None:
            result = {'kind': 'null', 'offset': None}
        elif type(offset) is int and not type(offset) is bool and 2 <= offset <= 11:
            result = {'kind': 'integer', 'offset': offset}
        else:
            result = {'kind': 'invalid', 'offset': None}
    return result
result = classify_offset(inputs.get('offset'))