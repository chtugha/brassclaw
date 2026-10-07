ports = inputs.get('ports')
valid = isinstance(ports, list) and len(ports) > 0
if valid:
    for port in ports:
        if isinstance(port, bool) or not isinstance(port, int) or not 1 <= port <= 65535:
            valid = False
            break
result = {'valid': valid, 'ports': ports if valid else []}