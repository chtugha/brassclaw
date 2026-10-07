def check_weighted_entries(candidate):
    if not isinstance(candidate, list) or len(candidate) > 3:
        return {'ok': False, 'entries': []}
    for item in candidate:
        if not isinstance(item, dict) or set(item.keys()) != {'label', 'weight'}:
            return {'ok': False, 'entries': []}
        label = item.get('label')
        weight = item.get('weight')
        if not isinstance(label, str) or not 1 <= len(label) <= 8:
            return {'ok': False, 'entries': []}
        if isinstance(weight, bool) or not isinstance(weight, int) or not 0 <= weight <= 1:
            return {'ok': False, 'entries': []}
    return {'ok': True, 'entries': candidate}
result = check_weighted_entries(inputs.get('entries'))