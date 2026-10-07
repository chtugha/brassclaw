entries = inputs.get('entries')
valid = isinstance(entries, list) and 0 <= len(entries) <= 3
if valid:
    for item in entries:
        if not isinstance(item, dict) or set(item.keys()) != {'label', 'weight'}:
            valid = False
            break
        label = item.get('label')
        weight = item.get('weight')
        if not (type(label) is str and 1 <= len(label) <= 8):
            valid = False
            break
        if not (type(weight) is int and not isinstance(weight, bool) and 0 <= weight <= 1):
            valid = False
            break
result = {'ok': valid, 'entries': entries if valid else []}