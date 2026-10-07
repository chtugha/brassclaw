def classify_entry(item):
    if not isinstance(item, dict):
        return False
    if 'label' not in item or 'weight' not in item:
        return False
    if len(item) != 2:
        return False
    label = item.get('label')
    weight = item.get('weight')
    if not (type(label) is str and 1 <= len(label) <= 8):
        return False
    if not (type(weight) is int and not isinstance(weight, bool) and 0 <= weight <= 1):
        return False
    return True

def check_weighted_entries(entries):
    if not isinstance(entries, list) or len(entries) > 3:
        return {'ok': False, 'entries': []}
    valid = True
    for item in entries:
        if not classify_entry(item):
            valid = False
            break
    result = {'ok': valid, 'entries': entries if valid else []}
    return result
result = check_weighted_entries(inputs.get('entries'))