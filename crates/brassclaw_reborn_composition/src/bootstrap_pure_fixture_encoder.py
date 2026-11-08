def encode_pure_fixture_result(inputs):
    import json
    kinds = ["text", "sources", "rule", "checks", "aggregation"]
    kind = inputs["kind"]
    step = inputs["step_id"]
    if kind not in kinds or type(step) is not str or not step:
        raise ValueError("pure_fixture_identity_invalid")
    if any(inputs[name] is not None for name in kinds if name != kind):
        raise ValueError("pure_fixture_extra_result")
    value = inputs[kind]
    if value is None:
        raise ValueError("pure_fixture_result_missing")
    # The producer has already crossed the native JSON/result-contract boundary.
    # Never coerce a tuple, bool, missing field or failed producer into this shape.
    if kind == "text" and type(value) is not str:
        raise ValueError("pure_fixture_result_invalid")
    if kind == "sources" and (type(value) is not dict or any(type(k) is not str or type(v) is not str for k, v in value.items())):
        raise ValueError("pure_fixture_result_invalid")
    if kind in ["rule", "checks"]:
        rows = [value] if kind == "rule" else value
        if type(rows) is not list:
            raise ValueError("pure_fixture_result_invalid")
        for row in rows:
            if type(row) is not dict or set(row) != {"rule_id", "ok", "errors"} or type(row["rule_id"]) is not str or type(row["ok"]) is not bool or type(row["errors"]) is not list or any(type(item) is not str for item in row["errors"]):
                raise ValueError("pure_fixture_result_invalid")
    if kind == "aggregation":
        if type(value) is not dict or set(value) != {"pass", "errors"} or type(value["pass"]) is not bool or type(value["errors"]) is not list:
            raise ValueError("pure_fixture_result_invalid")
        for row in value["errors"]:
            if type(row) is not dict or set(row) != {"rule_id", "errors"} or type(row["rule_id"]) is not str or type(row["errors"]) is not list or any(type(item) is not str for item in row["errors"]):
                raise ValueError("pure_fixture_result_invalid")
    return json.dumps({"format": "validator-step-results/1", "results": {step: value}})
