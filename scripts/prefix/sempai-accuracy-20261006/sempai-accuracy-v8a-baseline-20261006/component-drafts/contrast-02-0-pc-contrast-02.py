def classify_slots(raw):
    if not isinstance(raw, list) or not all(isinstance(x, int) and not isinstance(x, bool) and 2 <= x <= 6 for x in raw) or not 0 <= len(raw) <= 4:
        return {'ok': False, 'slots': []}
    return {'ok': True, 'slots': raw}
result = classify_slots(inputs.get('slots'))