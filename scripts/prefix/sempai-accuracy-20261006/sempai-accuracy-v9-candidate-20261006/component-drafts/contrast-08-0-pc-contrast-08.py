records = inputs.get('records')
ok = type(records) is list and 0 <= len(records) <= 4
if ok:
    for item in records:
        if type(item) is not dict or set(item) != {'code', 'enabled'}:
            ok = False
            break
        if not (type(item['code']) is str and 2 <= len(item['code']) <= 5 and type(item['enabled']) is bool):
            ok = False
            break
result = {'ok': ok, 'records': records if ok else []}