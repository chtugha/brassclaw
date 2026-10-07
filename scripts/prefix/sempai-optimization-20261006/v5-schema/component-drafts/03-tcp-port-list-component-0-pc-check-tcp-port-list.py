def check_tcp_port_list(candidate):
    if not isinstance(candidate, list) or len(candidate) == 0:
        return {'valid': False, 'ports': []}
    valid = True
    for port in candidate:
        if isinstance(port, bool) or not isinstance(port, int) or not 1 <= port <= 65535:
            valid = False
            break
    result = {'valid': valid, 'ports': candidate if valid else []}
    return result
result = check_tcp_port_list(inputs.get('ports'))