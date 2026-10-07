channels = inputs.get("channels")
valid = isinstance(channels, list) and len(channels) > 0
if valid:
    for item in channels:
        if isinstance(item, bool) or not isinstance(item, int) or not 0 <= item <= 255:
            valid = False
            break
result = {"ok": valid, "channels": channels if valid else []}