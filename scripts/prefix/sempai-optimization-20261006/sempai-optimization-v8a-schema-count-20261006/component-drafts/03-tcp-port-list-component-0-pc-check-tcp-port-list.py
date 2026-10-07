ports = inputs.get('ports')
valid = isinstance(ports, list) and len(ports) > 0
if valid:
    for p in ports:
        if not (type(p) is int and not isinstance(p, bool) and 1 <= p <= 65535):
            valid = False
            break
result = {'valid': valid, 'ports': ports if valid else []}