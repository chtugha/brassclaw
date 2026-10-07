slots = inputs.get('slots')
ok = type(slots) is list and 0 <= len(slots) <= 3
if ok:
    for slot in slots:
        if not (type(slot) is int and 4 <= slot <= 8):
            ok = False
            break
result = {'ok': ok, 'slots': slots if ok else []}