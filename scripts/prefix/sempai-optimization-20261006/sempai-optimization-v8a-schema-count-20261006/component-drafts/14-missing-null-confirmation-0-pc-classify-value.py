def classify_value(inputs):
    value = inputs.get('value')
    if 'value' not in inputs:
        result = {'state': 'missing', 'value': None}
    elif value is None:
        result = {'state': 'null', 'value': None}
    elif type(value) is int and not isinstance(value, bool) and 0 <= value <= 10:
        result = {'state': 'value', 'value': value}
    else:
        result = {'state': 'invalid', 'value': None}
    return result
result = classify_value(inputs)