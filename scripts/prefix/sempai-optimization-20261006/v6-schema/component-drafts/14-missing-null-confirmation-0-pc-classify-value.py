value = inputs.get('value')
if 'value' not in inputs:
    result = {'state': 'missing', 'value': None}
elif value is None:
    result = {'state': 'null', 'value': None}
elif type(value) is int and not 0 <= value <= 10:
    result = {'state': 'invalid', 'value': None}
else:
    result = {'state': 'value', 'value': value}