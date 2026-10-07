slots = inputs.get('slots')
ok = type(slots) is list and 1 <= len(slots) <= 4
if ok:
    for slot in slots:
        if not (type(slot) is int and 5 <= slot <= 9):
            ok = False
            break
result = {'ok': ok, 'slots': slots if ok else []}