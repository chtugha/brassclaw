def check_review_python_contracts(inputs):
    import json
    subject = json.loads(inputs["subject_json"])
    if type(subject) is not dict or subject.get("format") != "component-validator-subject/1":
        raise ValueError("complete retained validator subject required")
    candidate = subject.get("candidate")
    dependencies = subject.get("dependencies")
    if type(candidate) is not dict or type(dependencies) is not list:
        raise ValueError("complete retained validator subject required")
    root_reference = candidate.get("reference")
    if type(root_reference) is not dict or type(root_reference.get("class_code")) is not int or root_reference["class_code"] != 22:
        raise ValueError("PythonCode review subject required")
    seen = set()
    errors = []
    # Compare only the selected candidate/dependency graph. The comparison base
    # is not a newly proposed executable dependency. Original integrity and the
    # validator's fixed class/position selection belong to the trusted owner.
    for index, entry in enumerate([candidate] + dependencies):
        if type(entry) is not dict or type(entry.get("reference")) is not dict or type(entry.get("revision")) is not dict:
            raise ValueError("complete retained revision required")
        reference = entry["reference"]
        revision = entry["revision"]
        identity = reference.get("uuid")
        class_code = reference.get("class_code")
        if type(identity) is not str or not identity or identity in seen:
            raise ValueError("distinct retained component identities required")
        if type(class_code) is not int or type(revision.get("class_code")) is not int or revision.get("class_code") != class_code or revision.get("uuid") != identity:
            raise ValueError("retained revision identity differs")
        seen.add(identity)
        if class_code != 22:
            continue
        document = revision.get("document")
        if type(document) is not dict:
            raise ValueError("complete retained document required")
        prefix = "component " + str(index) + ": "
        preload = document.get("preload")
        if type(preload) is not dict or type(preload.get("exports")) is not dict:
            errors.append(prefix + "preload export declaration required")
            continue
        public = bool(preload["exports"])
        # Private helper-only libraries need no fabricated public boundary.
        # Every contract that is present is still checked; public exports need
        # both contracts. Definition/call/semantic checks remain separate rules.
        for field, position in [("input_contract", "input"), ("result_contract", "result")]:
            if field not in document:
                if public:
                    errors.append(prefix + field + " required for public exports")
                continue
            check = check_recursive_contract({
                "contract_json": json.dumps(document[field]), "position": position,
            })
            for diagnostic in check["errors"]:
                errors.append(prefix + field + ": " + diagnostic)
    return {"rule_id": "python_contracts", "ok": not errors, "errors": errors}
