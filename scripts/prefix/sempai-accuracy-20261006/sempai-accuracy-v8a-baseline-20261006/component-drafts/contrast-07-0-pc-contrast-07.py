def classify_records(records):
    if not (type(records) is list and 1 <= len(records) <= 3):
        result = {'ok': False, 'records': []}
    else:
        valid = True
        for rec in records:
            if not (type(rec) is dict and set(rec.keys()) == {'code', 'enabled'}
                    and type(rec['code']) is str and 2 <= len(rec['code']) <= 5
                    and type(rec['enabled']) is bool):
                valid = False
                break
        if not valid:
            result = {'ok': False, 'records': []}
        else:
            result = {'ok': True, 'records': records}
    return result
result = classify_records(inputs.get('records'))