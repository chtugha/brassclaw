slots = inputs.get('slots')
ok = type(slots) is list and 0 <= len(slots) <= 4
if ok:
    for slot in slots:
        if not (type(slot) is int and 2 <= slot <= 6):
            ok = False
            break
result = {'ok': ok, 'slots': slots if ok else []}