# Raw candidate: missing and null are distinct invalid outcomes; no coercion.
timeout_ms = inputs.get("timeout_ms")
valid = isinstance(timeout_ms, int) and not isinstance(timeout_ms, bool) and 0 <= timeout_ms <= 7500
result = {"valid": valid, "timeout_ms": timeout_ms if valid else None}