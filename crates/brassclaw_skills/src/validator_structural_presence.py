def check_structural_presence(inputs):
    errors = []
    for name in ["name", "description", "content"]:
        if not inputs[name].strip():
            errors.append(name + " is empty")
    return {"rule_id": "structural_presence", "ok": not errors, "errors": errors}
