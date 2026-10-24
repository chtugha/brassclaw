def _validator_python_graph_result(errors):
    return {"rule_id": "python_graph", "ok": not errors, "errors": errors}


def check_review_python_graph(inputs):
    import json
    definitions = inputs["definitions"]
    contracts = inputs["contracts"]
    if definitions["rule_id"] != "python_definitions" or contracts["rule_id"] != "python_contracts":
        raise ValueError("original prerequisite rule results required")
    # These are actual prior result edges in the pinned private Recipe, not
    # candidate flags. Preserve their full diagnostics in the separate rules.
    if not definitions["ok"] or not contracts["ok"]:
        return _validator_python_graph_result(["graph: matching definitions and valid contracts required"])
    profile = inputs["profile"]
    if profile["format"] != "retained-python-profile/1":
        raise ValueError("selected native retained profile required")
    builtins = set(profile["builtin_names"])
    imports = set(profile["import_names"])
    if not builtins or not imports or len(builtins) != len(profile["builtin_names"]) or len(imports) != len(profile["import_names"]):
        raise ValueError("complete distinct native profile names required")
    subject = json.loads(inputs["subject_json"])
    inventory = json.loads(inputs["inventory_json"])
    if subject.get("format") != "component-validator-subject/1" or inventory.get("format") != "python-source-inventory/1":
        raise ValueError("complete original subject and native inventory required")
    records = {}
    indices = {}
    for index, entry in enumerate([subject["candidate"]] + subject["dependencies"]):
        if entry["reference"]["class_code"] == 22:
            identity = entry["reference"]["uuid"]
            if identity in records:
                raise ValueError("distinct selected Python identities required")
            records[identity] = entry["revision"]
            indices[identity] = index
    root = subject["candidate"]["reference"]["uuid"]
    if root not in records or set(records) != set(inventory["sources"]):
        raise ValueError("complete original Python graph and native inventory required")
    errors = []
    dependencies = {}
    owners = {}
    public_names = set()
    for identity, revision in records.items():
        prefix = "component " + str(indices[identity]) + ": "
        dependencies[identity] = revision["dependencies"]
        if any(dependency not in records for dependency in dependencies[identity]):
            errors.append(prefix + "preload.dependencies: selected Python dependency missing or wrong class")
        declaration = revision["document"]["preload"]
        symbols = [entry["symbol"] for entry in declaration["exports"].values()]
        symbols.extend(declaration["private_functions"])
        symbols.extend(declaration["constants"])
        for symbol in symbols:
            if symbol in owners:
                errors.append(prefix + "preload: conflicting implementation symbol")
            else:
                owners[symbol] = identity
        for name, export in declaration["exports"].items():
            if name in public_names:
                errors.append(prefix + "preload.exports: conflicting public export")
            public_names.add(name)
            if not export["mapping"] and set(export["parameters"]) != set(revision["document"]["input_contract"]):
                errors.append(prefix + "preload.exports: positional signature differs from consumer inputs")
        inspection = inventory["sources"][identity]
        if inspection["kind"] != "inspected" or inspection["structure"]["preload"] is None:
            raise ValueError("matching completed native definition checks required")
        facts = inspection["structure"]
        if facts["relative_imports"] or any(name not in imports for name in facts["imports"]):
            errors.append(prefix + "source.imports: unsupported retained host profile")
        if facts["reserved_name_references"] or facts["host_value_references"] != 0:
            errors.append(prefix + "source: forbidden intrinsic or dynamic host reference")
    if errors:
        return _validator_python_graph_result(errors)
    # Kahn's worklist checks cycles without recursion or quadratic transitive
    # symbol-set copies. The actual loader still owns its exact UUID tie order.
    degrees = {identity: len(items) for identity, items in dependencies.items()}
    consumers = {identity: [] for identity in records}
    for identity, items in dependencies.items():
        for dependency in items:
            consumers[dependency].append(identity)
    ready = [identity for identity in records if degrees[identity] == 0]
    visited_count = 0
    while ready:
        identity = ready.pop()
        visited_count += 1
        for consumer in consumers[identity]:
            degrees[consumer] -= 1
            if degrees[consumer] == 0:
                ready.append(consumer)
    if visited_count != len(records):
        return _validator_python_graph_result(["graph: cyclic Python dependency graph"])
    reachable = set()
    pending = [root]
    while pending:
        identity = pending.pop()
        if identity not in reachable:
            reachable.add(identity)
            pending.extend(dependencies[identity])
    if reachable != set(records):
        return _validator_python_graph_result(["graph: selected Python component is outside the candidate closure"])
    for identity in records:
        needed = set()
        prefix = "component " + str(indices[identity]) + ": "
        for name in inventory["sources"][identity]["structure"]["preload"]["references"]:
            if name in owners:
                needed.add(owners[name])
            elif name not in builtins:
                errors.append(prefix + "source.references: undeclared retained symbol")
        # Resolve only actual referenced owners; do not materialize every
        # ancestor's complete transitive symbol set in a long dependency chain.
        remaining = len(needed)
        pending = [identity]
        reached = set()
        while pending and remaining:
            dependency = pending.pop()
            if dependency not in reached:
                reached.add(dependency)
                if dependency in needed:
                    remaining -= 1
                pending.extend(dependencies[dependency])
        if remaining:
            errors.append(prefix + "source.references: symbol belongs to an undeclared dependency")
    return _validator_python_graph_result(errors)
