quota = inputs.get('quota')
if quota is None:
    result = {'kind': 'null', 'quota': None}
elif type(quota) is int and not quota is True and 4 <= quota <= 13:
    result = {'kind': 'integer', 'quota': quota}
else:
    result = {'kind': 'invalid', 'quota': None}