channels = inputs.get("channels")
valid = isinstance(channels, list) and all(isinstance(item, int) and not isinstance(item, bool) and 0 <= item <= 255 for item in channels)
result = {"ok": valid, "channels": channels if valid else []}