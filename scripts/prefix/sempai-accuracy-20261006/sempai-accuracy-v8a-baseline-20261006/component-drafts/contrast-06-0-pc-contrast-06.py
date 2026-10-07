def classify_record(item):
    if not isinstance(item, dict):
        return False
    if set(item.keys()) != {'code', 'enabled'}:
        return False
    code = item.get('code')
    if not (type(code) is str and 2 <= len(code) <= 5):
        return False
    enabled = item.get('enabled')
    if not (type(enabled) is bool):
        return False
    return True

records = inputs.get('records')
if not isinstance(records, list):
    result = {'ok': False, 'records': []}
else:
    ok = all(classify_record(r) for r in records)
    result = {'ok': ok, 'records': records if ok else []}