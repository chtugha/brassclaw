def is_exact_flag(value):
    return isinstance(value, bool)
result = {"valid": is_exact_flag(inputs.get("flag")), "flag": inputs.get("flag") if is_exact_flag(inputs.get("flag")) else None}