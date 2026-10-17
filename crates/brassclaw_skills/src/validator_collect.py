def collect_validator_checks(inputs):
    results = []
    for name in inputs:
        check = inputs[name]
        if check["rule_id"] != name:
            raise ValueError("validator_rule_identity_invalid")
        results.append(check)
    return results
