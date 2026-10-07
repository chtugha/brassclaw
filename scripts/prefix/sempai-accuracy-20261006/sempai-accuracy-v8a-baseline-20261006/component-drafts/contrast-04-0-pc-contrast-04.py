def classify_slots(raw):
    if 'slots' not in raw:
        return {'ok': True, 'slots': []}
    slots = raw['slots']
    if not isinstance(slots, list) or not 0 <= len(slots) <= 3:
        return {'ok': False, 'slots': []}
    valid = True
    for item in slots:
        if isinstance(item, bool) or not isinstance(item, int) or not 4 <= item <= 8:
            valid = False
            break
    return {'ok': valid, 'slots': slots if valid else []}
result = classify_slots(inputs.get('slots'))