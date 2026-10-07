def classify_lag(raw):
    if 'lag' not in inputs:
        result = {'accepted': False, 'lag': None}
    else:
        lag = inputs['lag']
        if type(lag) is bool or not type(lag) is int or not -1 <= lag <= 8:
            result = {'accepted': False, 'lag': None}
        else:
            result = {'accepted': True, 'lag': lag}
result = classify_lag(inputs.get('lag'))