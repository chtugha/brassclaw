value = inputs.get("value")
if value is None:
    result = {"state": "null", "value": None}
else:
    if type(value) is int and 0 <= value <= 10:
        result = {"state": "value", "value": value}
    else:
        result = {"state": "invalid", "value": None}
if "value" not in inputs:
    result = {"state": "missing", "value": None}