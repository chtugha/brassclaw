if 'ttl' not in inputs:
    result = {'kind': 'missing', 'ttl': None}
elif inputs['ttl'] is None:
    result = {'kind': 'null', 'ttl': None}
elif type(inputs['ttl']) is int and 6 <= inputs['ttl'] <= 15:
    result = {'kind': 'integer', 'ttl': inputs['ttl']}
else:
    result = {'kind': 'invalid', 'ttl': None}