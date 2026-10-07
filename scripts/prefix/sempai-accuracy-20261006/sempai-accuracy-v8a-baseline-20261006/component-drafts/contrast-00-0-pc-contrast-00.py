def classify_slots(raw):
    if not isinstance(raw, list) or not 0 <= len(raw) <= 2:
        return {'ok': False, 'slots': []}
    for item in raw:
        if isinstance(item, bool) or not isinstance(item, int) or not 0 <= item <= 4:
            return {'ok': False, 'slots': []}
    return {'ok': True, 'slots': raw}
result = classify_slots(inputs.get('slots'))