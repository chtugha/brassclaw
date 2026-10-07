def classify_window(raw):
    if 'window' not in inputs:
        result = {'accepted': False, 'window': None}
    else:
        window = inputs['window']
        if type(window) is bool or not type(window) is int or not -3 <= window <= 6:
            result = {'accepted': False, 'window': None}
        else:
            result = {'accepted': True, 'window': window}
    return result
result = classify_window(inputs.get('window'))