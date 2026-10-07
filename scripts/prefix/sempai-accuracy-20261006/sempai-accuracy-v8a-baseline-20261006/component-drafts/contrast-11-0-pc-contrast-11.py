def classify_records(records):
    if not isinstance(records, list) or len(records) == 0:
        return {'ok': False, 'records': []}
    for item in records:
        if not isinstance(item, dict) or len(item) == 0:
            return {'ok': False, 'records': []}
        if 'code' not in item or 'enabled' not in item or len(item) != 2:
            return {'ok': False, 'records': []}
        code = item['code']
        enabled = item['enabled']
        if not (type(code) is str and 2 <= len(code) <= 5):
            return {'ok': False, 'records': []}
        if not (type(enabled) is bool):
            return {'ok': False, 'records': []}
    return {'ok': True, 'records': records}
result = classify_records(inputs.get('records'))