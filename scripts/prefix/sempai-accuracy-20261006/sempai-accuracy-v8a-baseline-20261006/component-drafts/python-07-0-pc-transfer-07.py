def classify_window(raw):
    if 'window' not in inputs:
        result = {'kind': 'missing', 'window': None}
    else:
        window = inputs['window']
        if window is None:
            result = {'kind': 'null', 'window': None}
        elif type(window) is int and not isinstance(window, bool) and 3 <= window <= 12:
            result = {'kind': 'integer', 'window': window}
        else:
            result = {'kind': 'invalid', 'window': None}
    return result
result = classify_window(inputs.get('window'))