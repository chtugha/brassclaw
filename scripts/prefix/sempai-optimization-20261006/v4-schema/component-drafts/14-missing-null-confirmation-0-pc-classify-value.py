value = inputs.get('value')
if value is None:
    result = {'state': 'null', 'value': None}
elif isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 10:
    result = {'state': 'invalid', 'value': None}
else:
    result = {'state': 'value', 'value': value}