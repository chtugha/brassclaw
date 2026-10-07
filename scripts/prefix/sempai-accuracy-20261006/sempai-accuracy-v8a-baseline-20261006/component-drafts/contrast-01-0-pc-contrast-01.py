def classify_slots(raw):
    if not isinstance(raw, list) or len(raw) < 1 or len(raw) > 3:
        return {'ok': False, 'slots': []}
    ok = True
    for item in raw:
        if type(item) is bool or not type(item) is int or not 1 <= item <= 5:
            ok = False
            break
    if not ok:
        return {'ok': False, 'slots': []}
    return {'ok': True, 'slots': raw}
result = classify_slots(inputs.get('slots'))