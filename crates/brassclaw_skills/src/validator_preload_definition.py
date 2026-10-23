def _validator_preload_identifier(name):
    if not name or name[0] not in "_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ":
        return False
    for character in name:
        if character not in "_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789":
            return False
    return name not in [
        "host", "inputs", "result", "exec", "eval", "open", "__import__",
        "False", "None", "True", "and", "as", "assert", "async", "await",
        "break", "class", "continue", "def", "del", "elif", "else", "except",
        "finally", "for", "from", "global", "if", "import", "in", "is",
        "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try",
        "while", "with", "yield",
    ]


def check_preload_definition(inputs):
    declaration = inputs["declaration"]
    observations = inputs["observations"]
    errors = []
    if declaration is None:
        return {"rule_id": "preload_definition", "ok": False, "errors": ["preload: declaration is missing"]}
    if declaration["format"] != "python-preload/2":
        errors.append("preload.format: unsupported declaration")
    functions = {}
    symbols = set()
    exports = declaration["exports"]
    for name in sorted(exports):
        entry = exports[name]
        symbol = entry["symbol"]
        parameters = entry["parameters"]
        if not _validator_preload_identifier(name) or name.startswith("_"):
            errors.append("preload.exports: invalid public name")
        if not _validator_preload_identifier(symbol) or symbol in symbols:
            errors.append("preload.exports: invalid or conflicting implementation symbol")
        if len(set(parameters)) != len(parameters) or any(
            parameter != "inputs" and not _validator_preload_identifier(parameter)
            for parameter in parameters
        ):
            errors.append("preload.exports: invalid parameter names")
        if entry["mapping"] and parameters != ["inputs"]:
            errors.append("preload.exports: mapping export requires exactly inputs")
        symbols.add(symbol)
        functions[symbol] = parameters
    for name in sorted(declaration["private_functions"]):
        parameters = declaration["private_functions"][name]
        if not _validator_preload_identifier(name) or name in symbols:
            errors.append("preload.private_functions: invalid or conflicting symbol")
        if len(set(parameters)) != len(parameters) or any(
            parameter != "inputs" and not _validator_preload_identifier(parameter)
            for parameter in parameters
        ):
            errors.append("preload.private_functions: invalid parameter names")
        symbols.add(name)
        functions[name] = parameters
    for name in declaration["constants"]:
        if not _validator_preload_identifier(name) or name in symbols:
            errors.append("preload.constants: invalid or conflicting symbol")
        symbols.add(name)
    if len(set(declaration["imports"])) != len(declaration["imports"]):
        errors.append("preload.imports: duplicate module")
    default = declaration.get("default_export")
    if default is not None and default not in exports:
        errors.append("preload.default_export: unknown public export")
    observed = observations["preload"]
    if observed is None:
        errors.append("source.preload: definitions cannot be loaded without effects under the selected profile")
    else:
        if observed["functions"] != functions:
            errors.append("source.preload.functions: declared signatures differ from parsed definitions")
        if set(observed["constants"]) != set(declaration["constants"]):
            errors.append("source.preload.constants: declarations differ from parsed definitions")
    if set(observations["imports"]) != set(declaration["imports"]):
        errors.append("source.imports: declarations differ from parsed modules")
    return {"rule_id": "preload_definition", "ok": not errors, "errors": errors}
