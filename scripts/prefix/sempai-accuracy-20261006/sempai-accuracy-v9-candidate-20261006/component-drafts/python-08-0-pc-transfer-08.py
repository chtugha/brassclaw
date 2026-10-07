quota = inputs.get('quota')
valid = type(quota) is int and 4 <= quota <= 12
result = {'kind': 'integer' if valid else 'invalid', 'quota': quota if valid else None}