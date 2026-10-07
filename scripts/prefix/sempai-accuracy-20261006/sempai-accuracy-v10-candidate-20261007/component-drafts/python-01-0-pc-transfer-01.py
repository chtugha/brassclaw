window = inputs.get('window')
accepted = type(window) is int and not isinstance(window, bool) and -3 <= window <= 6
result = {'accepted': accepted, 'window': window if accepted else None}