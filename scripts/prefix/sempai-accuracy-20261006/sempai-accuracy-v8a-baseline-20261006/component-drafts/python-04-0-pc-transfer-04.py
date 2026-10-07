def classify_ttl(raw):
    if 'ttl' not in raw:
        result = {'accepted': False, 'ttl': None}
    else:
        ttl = raw['ttl']
        if type(ttl) is bool or not type(ttl) is int or not 0 <= ttl <= 9:
            result = {'accepted': False, 'ttl': None}
        else:
            result = {'accepted': True, 'ttl': ttl}
    return result
result = classify_ttl(inputs.get('raw'))