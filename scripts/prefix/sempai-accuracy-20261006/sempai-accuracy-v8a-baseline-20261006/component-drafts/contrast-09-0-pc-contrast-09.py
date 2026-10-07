def classify_records(records):
    if not (type(records) is list and 1 <= len(records) <= 2):
        return {'ok': False, 'records': []}
    for rec in records:
        if not (type(rec) is dict and set(rec.keys()) == {'code', 'enabled'}
                and type(rec['code']) is str and 2 <= len(rec['code']) <= 5
                and type(rec['enabled']) is bool):
            return {'ok': False, 'records': []}
    return {'ok': True, 'records': records}
result = classify_records(inputs.get('records'))