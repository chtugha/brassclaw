def _validator_review_declaration_shape(value):
    required = {"format", "exports", "private_functions", "constants", "imports", "dependencies"}
    if type(value) is not dict:
        return False
    if any(field not in value for field in required) or any(field not in required and field != "default_export" for field in value):
        return False
    if type(value["format"]) is not str or type(value["exports"]) is not dict or type(value["private_functions"]) is not dict:
        return False
    for name, entry in value["exports"].items():
        if type(name) is not str or type(entry) is not dict or set(entry) != {"symbol", "parameters", "mapping"}:
            return False
        if type(entry["symbol"]) is not str or type(entry["mapping"]) is not bool or type(entry["parameters"]) is not list:
            return False
        if any(type(parameter) is not str for parameter in entry["parameters"]):
            return False
    for name, parameters in value["private_functions"].items():
        if type(name) is not str or type(parameters) is not list or any(type(parameter) is not str for parameter in parameters):
            return False
    for field in ["constants", "imports", "dependencies"]:
        if type(value[field]) is not list or any(type(item) is not str for item in value[field]):
            return False
    return value.get("default_export") is None or type(value["default_export"]) is str


def check_review_python_definitions(inputs):
    import json
    subject = json.loads(inputs["subject_json"])
    inventory = json.loads(inputs["inventory_json"])
    if type(subject) is not dict or subject.get("format") != "component-validator-subject/1":
        raise ValueError("complete retained validator subject required")
    candidate = subject.get("candidate")
    dependencies = subject.get("dependencies")
    if type(candidate) is not dict or type(dependencies) is not list:
        raise ValueError("complete retained validator subject required")
    if type(candidate.get("reference")) is not dict or type(candidate["reference"].get("class_code")) is not int or candidate["reference"]["class_code"] != 22:
        raise ValueError("PythonCode review subject required")
    if type(inventory) is not dict or set(inventory) != {"format", "preparation_revision", "worker_checksum", "sources"} or inventory["format"] != "python-source-inventory/1" or type(inventory["sources"]) is not dict:
        raise ValueError("complete native source inventory required")
    # The native reader and trusted observer establish source/worker integrity
    # and original input/result custody. This pure rule never authenticates a
    # caller JSON report, computes replacement facts or executes candidate code.
    records = []
    seen = set()
    for index, entry in enumerate([candidate] + dependencies):
        if type(entry) is not dict or type(entry.get("reference")) is not dict or type(entry.get("revision")) is not dict:
            raise ValueError("complete retained revision required")
        reference = entry["reference"]
        revision = entry["revision"]
        identity = reference.get("uuid")
        code = reference.get("class_code")
        if type(identity) is not str or not identity or identity in seen or type(code) is not int:
            raise ValueError("distinct retained component identities required")
        if type(revision.get("class_code")) is not int or revision["class_code"] != code or revision.get("uuid") != identity:
            raise ValueError("retained revision identity differs")
        seen.add(identity)
        if code == 22:
            if type(revision.get("document")) is not dict or type(revision.get("dependencies")) is not list:
                raise ValueError("complete retained document required")
            records.append((index, identity, revision))
    if set(inventory["sources"]) != set(record[1] for record in records):
        raise ValueError("native inventory differs from complete selected Python graph")
    errors = []
    for index, identity, revision in records:
        prefix = "component " + str(index) + ": "
        inspection = inventory["sources"][identity]
        if type(inspection) is not dict:
            raise ValueError("native source inspection required")
        if inspection.get("kind") == "compile_rejected":
            if set(inspection) != {"kind", "source_checksum", "category"} or inspection["category"] not in ["syntax", "unsupported", "import"]:
                raise ValueError("native compiler rejection required")
            errors.append(prefix + "source rejected by the selected compiler profile: " + inspection["category"])
            continue
        if set(inspection) != {"kind", "structure"} or inspection["kind"] != "inspected" or type(inspection["structure"]) is not dict:
            raise ValueError("complete native source observations required")
        declaration = revision["document"].get("preload")
        if not _validator_review_declaration_shape(declaration):
            errors.append(prefix + "preload: closed typed declaration required")
            continue
        if len(set(declaration["dependencies"])) != len(declaration["dependencies"]) or set(declaration["dependencies"]) != set(revision["dependencies"]):
            errors.append(prefix + "preload.dependencies: retained dependency declaration differs")
        check = check_preload_definition({"declaration": declaration, "observations": inspection["structure"]})
        for diagnostic in check["errors"]:
            errors.append(prefix + diagnostic)
    return {"rule_id": "python_definitions", "ok": not errors, "errors": errors}
