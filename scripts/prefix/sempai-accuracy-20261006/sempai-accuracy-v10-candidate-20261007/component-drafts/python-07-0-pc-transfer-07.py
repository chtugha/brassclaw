window = inputs.get('window')
if window is None:
    result = {'kind': 'null', 'window': None}
elif type(window) is int and 3 <= window <= 12:
    result = {'kind': 'integer', 'window': window}
else:
    result = {'kind': 'invalid', 'window': None}