channels = inputs.get("channels")
if not isinstance(channels, list):
    result = {"ok": False, "channels": []}
else:
    valid = all(isinstance(item, int) and not isinstance(item, bool) and 0 <= item <= 255 for item in channels)
    if valid:
        result = {"ok": True, "channels": channels}
    else:
        result = {"ok": False, "channels": []}
result = result