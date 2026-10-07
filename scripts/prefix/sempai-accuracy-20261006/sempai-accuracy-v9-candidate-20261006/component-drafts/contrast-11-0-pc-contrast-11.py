records = inputs.get('records')
ok = type(records) is list and 1 <= len(records) <= 4
if ok:
    for rec in records:
        if type(rec) is not dict or set(rec) != {'code', 'enabled'}:
            ok = False
            break
        if not (type(rec['code']) is str and 2 <= len(rec['code']) <= 5 and type(rec['enabled']) is bool):
            ok = False
            break
result = {'ok': ok, 'records': records if ok else []}