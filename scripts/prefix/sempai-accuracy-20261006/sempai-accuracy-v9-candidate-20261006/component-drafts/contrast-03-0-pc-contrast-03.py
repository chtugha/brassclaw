slots = inputs.get('slots')
ok = type(slots) is list and 1 <= len(slots) <= 2
if ok:
    for slot in slots:
        if not (type(slot) is int and 3 <= slot <= 7):
            ok = False
            break
result = {'ok': ok, 'slots': slots if ok else []}