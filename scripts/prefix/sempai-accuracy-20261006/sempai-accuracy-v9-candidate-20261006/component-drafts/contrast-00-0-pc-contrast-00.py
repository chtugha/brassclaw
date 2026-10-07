slots = inputs.get('slots')
valid = type(slots) is list and 0 <= len(slots) <= 2
if valid:
    for slot in slots:
        if not (type(slot) is int and 0 <= slot <= 4):
            valid = False
            break
result = {'ok': valid, 'slots': slots if valid else []}