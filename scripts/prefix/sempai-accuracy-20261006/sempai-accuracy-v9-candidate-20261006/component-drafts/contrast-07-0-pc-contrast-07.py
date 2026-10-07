records = inputs.get('records')
valid = type(records) is list and 1 <= len(records) <= 3
if valid:
    for record in records:
        if type(record) is not dict or set(record) != {'code', 'enabled'}:
            valid = False
            break
        if not (type(record['code']) is str and 2 <= len(record['code']) <= 5 and type(record['enabled']) is bool):
            valid = False
            break
result = {'ok': valid, 'records': records if valid else []}