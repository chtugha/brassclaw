def _validator_skill_call_targets_result(errors):
    return {"rule_id": "skill_call_targets", "ok": not errors, "errors": errors}


def check_review_skill_call_targets(inputs):
    import json
    graph, interface = inputs["graph"], inputs["interface"]
    if graph["rule_id"] != "python_graph" or interface["rule_id"] != "skill_interface":
        raise ValueError("actual selected graph and Skill interface rule results required")
    if not graph["ok"] or not interface["ok"]:
        return _validator_skill_call_targets_result(["skill_call_targets: valid original graph and Skill interface required"])
    subject, entries = _validator_review_subject(inputs)
    inventory = json.loads(inputs["inventory_json"])
    profile = inputs["profile"]
    if subject["format"] != "component-validator-subject/2" or inventory["format"] != "python-source-inventory/1" or profile["format"] != "retained-python-profile/1":
        raise ValueError("original function subject/native inventory and selected profile required")
    pair = [subject["candidate"], subject["function_partner"]["component"]]
    skill, code = None, None
    for entry in pair:
        if entry["reference"]["class_code"] in [1, 2, 3]:
            skill = entry
        elif entry["reference"]["class_code"] == 22:
            code = entry
    declaration = skill["revision"]["document"]["interface"]
    association = json.loads(skill["revision"]["association"])
    callable_name = association["callable"]
    if type(callable_name) is not str or not callable_name.startswith("host."):
        raise ValueError("retained Skill host callable required")
    records, scopes = {}, {}
    constants = set()
    for entry in entries:
        if entry["reference"]["class_code"] != 22:
            continue
        identity = entry["reference"]["uuid"]
        inspection = inventory["sources"][identity]
        if inspection["kind"] != "inspected" or inspection["structure"]["preload"] is None:
            raise ValueError("complete original native preload observations required")
        records[identity] = inspection["structure"]
        for name, scope in inspection["structure"]["preload"]["scopes"].items():
            if name in scopes:
                raise ValueError("original graph must have distinct implementation symbols")
            scopes[name] = scope
        constants.update(inspection["structure"]["preload"]["constants"])
    if set(records) != set(inventory["sources"]) or code["reference"]["uuid"] not in records:
        raise ValueError("complete original Python inventory required")
    needed, errors, visited = _validator_python_reachable_functions(
        scopes, [entry["symbol"] for entry in declaration["exports"].values()],
        set(profile["builtin_names"]), constants,
    )
    if visited != len(needed):
        errors.append("skill_call_targets: recursive function graph requires unsupported retained invocation qualification")
    # Attribute each observed site to its actual enclosing top-level function.
    # Sorted disjoint ranges give one linear scan after sorting, with no
    # function-count x call-count search or candidate-controlled depth cap.
    for facts in records.values():
        ranges = sorted([(scope["start"], scope["end"], name)
                         for name, scope in facts["preload"]["scopes"].items()])
        sites = sorted([(site["start"], site["end"], site["attribute"])
                        for site in facts["direct_host_calls"]])
        position = 0
        for start, end, attribute in sites:
            while position < len(ranges) and ranges[position][1] <= start:
                position += 1
            if position == len(ranges) or start < ranges[position][0] or end > ranges[position][1]:
                raise ValueError("native Host site lacks an enclosing retained function")
            name = ranges[position][2]
            if name not in needed:
                continue
            if scopes[name]["nested_functions"]:
                errors.append("function " + name + ": Tool-calling closure requires unsupported invocation qualification")
            if "host." + attribute != callable_name:
                errors.append("function " + name + ": observed Host target differs from the declared Skill usage")
    # No cardinality, argument/result variance, invocation or semantic verdict.
    # Zero/multiple matching sites do not establish a complete Skill approval.
    return _validator_skill_call_targets_result(errors)
