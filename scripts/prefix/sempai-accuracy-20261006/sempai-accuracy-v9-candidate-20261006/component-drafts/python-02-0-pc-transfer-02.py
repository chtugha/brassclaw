quota = inputs.get('quota')
accepted = type(quota) is int and not type(quota) is bool and -2 <= quota <= 7
result = {'accepted': accepted, 'quota': quota if accepted else None}