def classify_quota(quota):
    if type(quota) is bool or not type(quota) is int or not -2 <= quota <= 7:
        result = {'accepted': False, 'quota': None}
    else:
        result = {'accepted': True, 'quota': quota}
result = classify_quota(inputs.get('quota'))