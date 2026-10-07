ports = inputs.get('ports')
valid = isinstance(ports, list) and len(ports) > 0
if valid:
    for p in ports:
        if isinstance(p, bool) or not isinstance(p, int) or not 1 <= p <= 65535:
            valid = False
            break
result = {'valid': valid, 'ports': ports if valid else []}