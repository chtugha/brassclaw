def _validator_contract_name(name):
    return type(name) is str and bool(name) and name[0] in "abcdefghijklmnopqrstuvwxyz" and all(
        character in "abcdefghijklmnopqrstuvwxyz0123456789_" for character in name
    )


def _validator_contract_number(value):
    # These are JSON integer representation bounds, not adjustable task caps.
    if type(value) is int:
        return -9223372036854775808 <= value <= 18446744073709551615
    return type(value) is float and value == value and abs(value) != float("inf")


def _validator_contract_parts(value):
    text = str(value).lower()
    negative = text.startswith("-")
    if negative:
        text = text[1:]
    exponent = 0
    if "e" in text:
        text, power = text.split("e")
        exponent = int(power)
    decimals = len(text.split(".")[1]) if "." in text else 0
    digits = text.replace(".", "").lstrip("0")
    scale = decimals - exponent
    while digits.endswith("0"):
        digits = digits[:-1]
        scale -= 1
    if not digits:
        return (False, "0", 0)
    return (negative, digits, scale)


def _validator_contract_compare(left, right):
    # Match the native contract's normalized decimal comparison, including
    # adjacent u64 integers above 2**53; never coerce both operands to float.
    ln, ld, ls = _validator_contract_parts(left)
    rn, rd, rs = _validator_contract_parts(right)
    if ln != rn:
        return -1 if ln else 1
    if ld == "0" or rd == "0":
        return 0 if ld == rd else (-1 if ld == "0" else 1)
    magnitude_left = len(ld) - ls
    magnitude_right = len(rd) - rs
    if magnitude_left != magnitude_right:
        order = -1 if magnitude_left < magnitude_right else 1
    else:
        width = max(len(ld), len(rd))
        ltext = ld + "0" * (width - len(ld))
        rtext = rd + "0" * (width - len(rd))
        order = 0 if ltext == rtext else (-1 if ltext < rtext else 1)
    return -order if ln else order


def _validator_contract_default(schema, value):
    pending = [(schema, value)]
    while pending:
        current, supplied = pending.pop()
        kind = current["type"]
        if supplied is None:
            if kind == "null" or current.get("nullable", False):
                continue
            return False
        valid = (
            (kind == "string" and type(supplied) is str)
            or (kind == "boolean" and type(supplied) is bool)
            or (kind == "integer" and type(supplied) is int and _validator_contract_number(supplied))
            or (kind == "number" and _validator_contract_number(supplied))
            or (kind == "list" and type(supplied) is list)
            or (kind == "object" and type(supplied) is dict)
        )
        if not valid:
            return False
        for check in current.get("checks", []):
            name = check["kind"]
            if name == "min_length" and len(supplied) < check["value"]:
                return False
            if name == "min" and _validator_contract_compare(supplied, check["value"]) < 0:
                return False
            if name == "max" and _validator_contract_compare(supplied, check["value"]) > 0:
                return False
        if kind == "list":
            pending.extend((current["items"], item) for item in supplied)
        if kind == "object":
            fields = current["fields"]
            for name in fields:
                child = fields[name]
                if name in supplied:
                    pending.append((child, supplied[name]))
                elif "default" in child:
                    pending.append((child, child["default"]))
                elif child["required"]:
                    return False
            for name in supplied:
                if name not in fields:
                    if not current["allow_extra_fields"]:
                        return False
                    pending.append((current["extra_fields"], supplied[name]))
    return True


def check_recursive_contract(inputs):
    import json
    contract = json.loads(inputs["contract_json"])
    position = inputs["position"]
    errors = []
    if position not in ["input", "parameters", "result"]:
        errors.append("contract: unsupported review position")
    elif position != "result" and type(contract) is not dict:
        errors.append("contract: input mapping required")
    if errors:
        return {"rule_id": "recursive_contract", "ok": False, "errors": errors}
    defaults = position != "result"
    pending = [(contract, "value")] if not defaults else [(contract[name], position) for name in contract]
    if defaults and any(not _validator_contract_name(name) for name in contract):
        errors.append("contract: invalid local input name")
    deferred_defaults = []
    while pending:
        schema, location = pending.pop()
        if type(schema) is not dict:
            errors.append("schema: object required")
            continue
        kind = schema.get("type")
        if kind not in ["string", "integer", "number", "boolean", "null", "list", "object"]:
            errors.append("schema: invalid type")
            continue
        present = location in ["input", "parameters", "field"]
        allowed = {"type", "checks"}
        if kind != "null":
            allowed.add("nullable")
        if present:
            allowed.add("required")
            if defaults:
                allowed.add("default")
        if kind == "list":
            allowed.add("items")
        if kind == "object":
            allowed.update(["fields", "allow_extra_fields", "extra_fields"])
        if any(name not in allowed for name in schema):
            errors.append("schema: unknown or misplaced field")
        if "nullable" in schema and type(schema["nullable"]) is not bool:
            errors.append("schema: nullable must be boolean")
        if present and type(schema.get("required")) is not bool:
            errors.append("schema: required must be explicit boolean")
        if schema.get("required") is True and "default" in schema:
            errors.append("schema: required value forbids default")
        if location == "input" and schema.get("required") is False and "default" not in schema:
            errors.append("schema: optional top-level input needs default")
        checks = schema.get("checks", [])
        if "checks" not in schema and location in ["input", "parameters"]:
            errors.append("schema: explicit checks required")
        if type(checks) is not list:
            errors.append("schema: checks must be list")
            checks = []
        lower = None
        upper = None
        for check in checks:
            if type(check) is not dict or type(check.get("kind")) is not str:
                errors.append("schema: invalid check")
                continue
            name = check["kind"]
            fields = {"kind", "input"} if name == "max_field" else {"kind", "value"}
            if set(check) != fields:
                errors.append("schema: invalid check fields")
                continue
            if name == "max_field":
                target = contract.get(check["input"]) if type(check["input"]) is str and type(contract) is dict else None
                if location not in ["input", "parameters"] or kind not in ["integer", "number"] or not _validator_contract_name(check["input"]):
                    errors.append("schema: unsupported cross-input check")
                elif type(target) is not dict or target.get("type") not in ["integer", "number"] or target.get("nullable", False) or schema.get("nullable", False):
                    errors.append("schema: cross-input target must be declared numeric and non-null")
            elif name == "min_length":
                bound = check["value"]
                if kind != "string" or type(bound) is not int or not 0 <= bound <= 18446744073709551615:
                    errors.append("schema: invalid minimum string length")
            elif name in ["min", "max"]:
                bound = check["value"]
                if kind not in ["integer", "number"] or not _validator_contract_number(bound) or (kind == "integer" and type(bound) is not int):
                    errors.append("schema: numeric check type mismatch")
                elif name == "min" and (lower is None or _validator_contract_compare(bound, lower) > 0):
                    lower = bound
                elif name == "max" and (upper is None or _validator_contract_compare(bound, upper) < 0):
                    upper = bound
            else:
                errors.append("schema: unsupported check")
        if lower is not None and upper is not None and _validator_contract_compare(lower, upper) > 0:
            errors.append("schema: contradictory numeric bounds")
        if kind == "list":
            if "items" not in schema:
                errors.append("schema: list requires items")
            else:
                pending.append((schema["items"], "value"))
        if kind == "object":
            fields = schema.get("fields")
            if type(fields) is not dict:
                errors.append("schema: object requires fields")
            else:
                for name in fields:
                    if not name:
                        errors.append("schema: empty object field name")
                    pending.append((fields[name], "field"))
            extra = schema.get("allow_extra_fields")
            if type(extra) is not bool or (not extra and "extra_fields" in schema) or (extra and "extra_fields" not in schema):
                errors.append("schema: invalid extra-field contract")
            elif extra:
                pending.append((schema["extra_fields"], "value"))
        if "default" in schema:
            deferred_defaults.append((schema, schema["default"]))
    # Only interpret defaults after every nested schema passed structure checks.
    # No filling/coercion of a candidate or output value occurs in this rule.
    if not errors:
        for schema, value in deferred_defaults:
            if not _validator_contract_default(schema, value):
                errors.append("schema: default violates its recursive contract")
    return {"rule_id": "recursive_contract", "ok": not errors, "errors": errors}
