def classify_records(records):
    valid = isinstance(records, list) and len(records) <= 3
    if valid:
        for item in records:
            if not (type(item) is dict and set(item.keys()) == {'code', 'enabled'}
                    and type(item['code']) is str and 2 <= len(item['code']) <= 5
                    and type(item['enabled']) is bool):
                valid = False
                break
    if valid:
        result = {'ok': True, 'records': records}
    else:
        result = {'ok': False, 'records': []}
    return result
result = classify_records(inputs.get('records'))