def encode_structural_checker_cases(inputs):
    import json
    expected = json.loads(inputs["expected_json"])
    if type(expected) is not list or not expected:
        raise ValueError("checker_fixture_matrix_invalid")
    results = {}
    seen = set()
    for case in expected:
        if type(case) is not dict or set(case) != {"slot", "step", "expected"}:
            raise ValueError("checker_fixture_matrix_invalid")
        slot = case["slot"]
        step = case["step"]
        if type(slot) is not str or type(step) is not str or not step or slot in seen or step in results:
            raise ValueError("checker_fixture_identity_invalid")
        seen.add(slot)
        actual = inputs[slot]
        wanted = case["expected"]
        for value in [actual, wanted]:
            if type(value) is not dict or set(value) != {"rule_id", "ok", "errors"}:
                raise ValueError("checker_fixture_result_invalid")
            if type(value["rule_id"]) is not str or not value["rule_id"] or type(value["ok"]) is not bool or type(value["errors"]) is not list:
                raise ValueError("checker_fixture_result_invalid")
            if value["ok"] == bool(value["errors"]):
                raise ValueError("checker_fixture_result_contradictory")
            for diagnostic in value["errors"]:
                if type(diagnostic) is not str or not diagnostic:
                    raise ValueError("checker_fixture_diagnostic_invalid")
        if actual != wanted:
            raise ValueError("checker_fixture_outcome_differs")
        results[step] = actual
    for name in inputs:
        if name != "expected_json" and name not in seen and inputs[name] is not None:
            raise ValueError("checker_fixture_coverage_differs")
    return json.dumps({"format": "validator-step-results/1", "results": results})
