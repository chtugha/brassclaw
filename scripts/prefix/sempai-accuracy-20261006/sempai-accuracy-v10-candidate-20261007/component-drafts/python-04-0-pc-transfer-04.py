ttl = inputs.get('ttl')
accepted = type(ttl) is int and not type(ttl) is bool and 0 <= ttl <= 9
result = {'accepted': accepted, 'ttl': ttl if accepted else None}