window = inputs.get('window')
if window is None and 'window' not in inputs:
    result = {'kind': 'missing', 'window': None}
elif window is None:
    result = {'kind': 'null', 'window': None}
elif type(window) is int and 3 <= window <= 12:
    result = {'kind': 'integer', 'window': window}
else:
    result = {'kind': 'invalid', 'window': None}