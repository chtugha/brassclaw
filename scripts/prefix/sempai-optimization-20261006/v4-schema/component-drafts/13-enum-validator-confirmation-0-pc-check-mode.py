mode = inputs.get("mode")
accepted = isinstance(mode, str) and mode in ("eco", "boost")
result = {"accepted": accepted, "mode": mode if accepted else None}