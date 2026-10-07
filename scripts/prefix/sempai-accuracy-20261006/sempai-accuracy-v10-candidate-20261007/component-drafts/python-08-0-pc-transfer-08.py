quota = inputs.get('quota')
if quota is None:
    result = {'kind': 'missing', 'quota': None}
elif type(quota) is int and 4 <= quota <= 12:
    result = {'kind': 'integer', 'quota': quota}
else:
    result = {'kind': 'invalid', 'quota': None}