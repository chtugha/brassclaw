mode = inputs.get("mode")
accepted = type(mode) is str and mode in ("eco", "boost")
result = {"accepted": accepted, "mode": mode if accepted else None}