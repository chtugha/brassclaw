ttl = inputs.get('ttl')
if ttl is None:
    result = {'kind': 'null', 'ttl': None}
elif type(ttl) is int and 6 <= ttl <= 15:
    result = {'kind': 'integer', 'ttl': ttl}
else:
    result = {'kind': 'invalid', 'ttl': None}