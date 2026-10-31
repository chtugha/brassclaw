def _validator_skill_interface_equal(left, right):
    pending = [(left, right)]
    while pending:
        first, second = pending.pop()
        if type(first) is not type(second):
            return False
        if type(first) is dict:
            if set(first) != set(second):
                return False
            pending.extend((first[name], second[name]) for name in first)
        elif type(first) is list:
            if len(first) != len(second):
                return False
            pending.extend(zip(first, second))
        elif first != second:
            return False
    return True


def check_review_skill_interface(inputs):
    import json
    subject, entries = _validator_review_subject(inputs)
    if subject["format"] != "component-validator-subject/2":
        raise ValueError("complete Skill/implementation function subject required")
    pair = [subject["candidate"], subject["function_partner"]["component"]]
    skill, code = None, None
    for entry in pair:
        if entry["reference"]["class_code"] in [1, 2, 3]:
            skill = entry
        elif entry["reference"]["class_code"] == 22:
            code = entry
    document = skill["revision"]["document"]
    association = json.loads(skill["revision"]["association"])
    interface = document.get("interface")
    errors = []
    if type(document.get("body")) is not str or not document["body"].strip():
        errors.append("skill.body: nonblank usage prose required")
    fields = {"format", "python_code_uuid", "exports", "inputs", "result", "failure", "dependencies", "private_symbols"}
    if type(interface) is not dict or set(interface) != fields:
        return {"rule_id": "skill_interface", "ok": False, "errors": errors + ["skill.interface: complete closed declaration required"]}
    if interface["format"] != "skill-interface/1" or interface["python_code_uuid"] != code["reference"]["uuid"]:
        errors.append("skill.interface: format or implementing identity differs")
    for field in ["inputs", "result", "failure"]:
        if not _validator_skill_interface_equal(interface[field], association[field]):
            errors.append("skill.interface." + field + ": differs from the exact usage association")
    # Reuse grammar checks; this neither infers semantic agreement nor requires
    # every compatible implementation schema to equal the usage schema.
    for field, position in [("inputs", "input"), ("result", "result")]:
        check = check_recursive_contract({"contract_json": json.dumps(interface[field]), "position": position})
        for diagnostic in check["errors"]:
            errors.append("skill.interface." + field + ": " + diagnostic)
    preload = code["revision"]["document"].get("preload")
    if type(preload) is not dict or type(preload.get("exports")) is not dict or type(preload.get("private_functions")) is not dict or type(preload.get("constants")) is not list or type(preload.get("dependencies")) is not list:
        errors.append("implementation.preload: complete declaration required")
    else:
        exports = interface["exports"]
        if type(exports) is not dict or not exports:
            errors.append("skill.interface.exports: nonempty public interface required")
        else:
            for name, entry in exports.items():
                if type(name) is not str or not _validator_preload_export_shape(entry) or name not in preload["exports"] or not _validator_skill_interface_equal(entry, preload["exports"][name]):
                    errors.append("skill.interface.exports: name or exact declared signature differs")
        for field, available in [("dependencies", preload["dependencies"]), ("private_symbols", list(preload["private_functions"]) + preload["constants"])]:
            values = interface[field]
            if type(values) is not list or any(type(value) is not str or not value for value in values):
                errors.append("skill.interface." + field + ": distinct named declarations required")
            elif len(set(values)) != len(values) or any(value not in available for value in values):
                errors.append("skill.interface." + field + ": undeclared or duplicate reference")
    # Actual code exports/loads/calls, schema variance, reachable private closure,
    # isolated behavior, effects/errors/retries and prose meaning are other rules.
    # Unchanged context is never a second own approval. No success grants Q1/Q2.
    return {"rule_id": "skill_interface", "ok": not errors, "errors": errors}
