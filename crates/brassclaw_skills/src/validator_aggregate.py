def aggregate_validator_checks(inputs):
    required = inputs["required_rule_ids"]
    checks = inputs["checks"]
    if type(required) is not list or type(checks) is not list or not required:
        raise ValueError("validator_rule_coverage_invalid")
    required_ids = set()
    for rule_id in required:
        if type(rule_id) is not str or not rule_id or rule_id in required_ids:
            raise ValueError("validator_required_rule_invalid")
        required_ids.add(rule_id)
    seen = set()
    errors = []
    for check in checks:
        if type(check) is not dict or set(check) != {"rule_id", "ok", "errors"}:
            raise ValueError("validator_rule_result_invalid")
        rule_id = check["rule_id"]
        ok = check["ok"]
        diagnostics = check["errors"]
        if type(rule_id) is not str or rule_id not in required_ids or rule_id in seen:
            raise ValueError("validator_rule_identity_invalid")
        if type(ok) is not bool or type(diagnostics) is not list:
            raise ValueError("validator_rule_result_invalid")
        if (ok and diagnostics) or (not ok and not diagnostics):
            raise ValueError("validator_rule_result_contradictory")
        for diagnostic in diagnostics:
            if type(diagnostic) is not str or not diagnostic:
                raise ValueError("validator_diagnostic_invalid")
        seen.add(rule_id)
        if not ok:
            errors.append({"rule_id": rule_id, "errors": diagnostics[:]})
    if seen != required_ids:
        raise ValueError("validator_rule_coverage_incomplete")
    return {"pass": len(errors) == 0, "errors": errors}
