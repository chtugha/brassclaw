def classify_slots(raw):
    valid = isinstance(raw, list) and 1 <= len(raw) <= 2
    if valid:
        for item in raw:
            if type(item) is bool or not type(item) is int or not 3 <= item <= 7:
                valid = False
                break
    return {'ok': valid, 'slots': raw if valid else []}
result = classify_slots(inputs.get('slots'))