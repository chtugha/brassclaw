def _validator_python_pure_calls_result(errors):
    return {"rule_id": "python_pure_calls", "ok": not errors, "errors": errors}


def check_review_python_pure_calls(inputs):
    import json
    graph = inputs["graph"]
    if graph["rule_id"] != "python_graph":
        raise ValueError("actual selected graph rule required")
    if not graph["ok"]:
        return _validator_python_pure_calls_result(["pure_calls: valid original graph required"])
    subject, entries = _validator_review_subject(inputs)
    inventory = json.loads(inputs["inventory_json"])
    profile = inputs["profile"]
    if subject["format"] != "component-validator-subject/1" or inventory["format"] != "python-source-inventory/1" or profile["format"] != "retained-python-profile/1":
        raise ValueError("original subject/native inventory and selected profile required")
    records = {}
    scopes = {}
    constants = set()
    errors = []
    for index, entry in enumerate(entries):
        if entry["reference"]["class_code"] != 22:
            continue
        identity = entry["reference"]["uuid"]
        if identity in records:
            raise ValueError("distinct selected Python identities required")
        records[identity] = entry["revision"]
        inspection = inventory["sources"][identity]
        if inspection["kind"] != "inspected" or inspection["structure"]["preload"] is None:
            raise ValueError("complete original preload observations required")
        facts = inspection["structure"]
        if facts["direct_host_calls"]:
            errors.append("component " + str(index) + ": pure closure contains a host call, including private or unselected code")
        for name, scope in facts["preload"]["scopes"].items():
            if name in scopes:
                raise ValueError("original graph must have distinct implementation symbols")
            scopes[name] = scope
        constants.update(facts["preload"]["constants"])
    root = subject["candidate"]["reference"]["uuid"]
    if root not in records or set(records) != set(inventory["sources"]):
        raise ValueError("complete selected Python closure required")
    exports = records[root]["document"]["preload"]["exports"]
    if not exports:
        errors.append("candidate.preload.exports: pure approval requires a public entry point")
    if errors:
        return _validator_python_pure_calls_result(errors)
    # All public root exports belong to the reviewed interface. Walk actual
    # call and function-value edges, as the retained preflight does, including
    # exports other than the currently selected default.
    needed = {}
    pending = [entry["symbol"] for entry in exports.values()]
    builtins = set(profile["builtin_names"])
    while pending:
        name = pending.pop()
        if name in needed:
            continue
        if name not in scopes:
            raise ValueError("declared public implementation lacks native scope")
        scope = scopes[name]
        dependencies = set()
        for target in scope["calls"]:
            if target in scopes:
                dependencies.add(target)
            elif target in constants or target not in builtins:
                errors.append("function " + name + ": unsupported retained global callable " + target)
        for target in scope["value_references"]:
            if target in scopes:
                dependencies.add(target)
        needed[name] = dependencies
        pending.extend(dependencies)
    # Iterative cycle detection has no Python recursion/depth cap. A recursive
    # function-value edge is unsupported by current retained qualification too.
    degrees = {name: len(dependencies) for name, dependencies in needed.items()}
    consumers = {name: [] for name in needed}
    for name, dependencies in needed.items():
        for dependency in dependencies:
            consumers[dependency].append(name)
    ready = [name for name in needed if degrees[name] == 0]
    visited = 0
    while ready:
        name = ready.pop()
        visited += 1
        for consumer in consumers[name]:
            degrees[consumer] -= 1
            if degrees[consumer] == 0:
                ready.append(consumer)
    if visited != len(needed):
        errors.append("pure_calls: recursive function graph requires unsupported retained invocation qualification")
    return _validator_python_pure_calls_result(errors)
