# Target typed-input interface; binding validates all required inputs first and never raises on invalid input.
ports = inputs["ports"]
if not isinstance(ports, list) or len(ports) == 0:
    result = {"valid": False, "ports": []}
else:
    valid = True
    for p in ports:
        if isinstance(p, bool) or not isinstance(p, int) or not (1 <= p <= 65535):
            valid = False
            break
    result = {"valid": valid, "ports": ports if valid else []}
