# Raw candidate: missing/invalid timeout_ms returns an invalid result, not an exception.
timeout_ms = inputs.get("timeout_ms")
valid = isinstance(timeout_ms, int) and not isinstance(timeout_ms, bool) and 0 <= timeout_ms <= 7500
result = {"valid": valid, "timeout_ms": timeout_ms if valid else None}