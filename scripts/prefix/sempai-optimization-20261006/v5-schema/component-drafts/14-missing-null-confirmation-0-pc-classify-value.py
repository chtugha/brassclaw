value = inputs.get('value')
if value is None:
    result = {'state': 'null' if isinstance(value, type(None)) else 'missing', 'value': None}
else:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 10:
        result = {'state': 'invalid', 'value': None}
    else:
        result = {'state': 'value', 'value': value}