records = inputs.get('records')
ok = False
if type(records) is list and 1 <= len(records) <= 2:
    for record in records:
        if type(record) is not dict or set(record) != {'code', 'enabled'}:
            ok = False
            break
        if not (type(record['code']) is str and 2 <= len(record['code']) <= 5 and type(record['enabled']) is bool):
            ok = False
            break
result = {'ok': ok, 'records': records if ok else []}