def check_timeout_ms():
    raw = inputs.get("timeout_ms")
    valid = isinstance(raw, int) and not isinstance(raw, bool) and 0 <= raw <= 7500
    result = {"valid": valid, "timeout_ms": raw if valid else None}
