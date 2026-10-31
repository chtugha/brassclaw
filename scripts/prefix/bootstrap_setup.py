"""Pure configuration-only bootstrap, selected and checked by the native owner.
No discovery/model/approval effect; endpoint and design acceptance stay unknown.
"""
def bootstrap_setup(inputs):
    import json
    data = json.loads(inputs["capture_bytes"])
    if sorted(data) != ["format", "job_id", "origin_mutation", "packet", "setup"]:
        raise ValueError("bootstrap capture fields")
    if data["format"] != "brassclaw-bootstrap-setup-capture/1":
        raise ValueError("bootstrap capture format")
    packet = data["packet"]
    if sorted(packet) != ["input_sha256", "payload", "request_id"] or packet["request_id"] != data["job_id"]:
        raise ValueError("bootstrap packet identity")
    payload = packet["payload"]
    setup = data["setup"]
    target = payload["target"]
    if setup["specification"] != target["specification"] or payload["executing_pair"] != target:
        raise ValueError("bootstrap original pair")
    if payload["task"] != "extract_bootstrap_facts" or payload["required_facts"] != [] or payload["eligible_options"] != []:
        raise ValueError("technical bootstrap cannot interpret documentation")
    names = ["additional_framing_tokens", "max_prefix_tokens", "max_prompt_tokens", "reserved_output_tokens"]
    if sorted(setup["fields"]) != names or sorted(payload["existing_values"]) != names:
        raise ValueError("bootstrap setup fields")
    for name in names:
        field = setup["fields"][name]
        old = payload["existing_values"][name]
        if sorted(field) != ["origin", "value"] or field["origin"] not in ["unset", "manual", "provider_configuration"]:
            raise ValueError("bootstrap setup origin")
        value = field["value"]
        if value is not None and (type(value) is not int or value < 0 or value > ((1 << 53) - 1)):
            raise ValueError("bootstrap setup value")
        if value == 0 and name in ["max_prompt_tokens", "reserved_output_tokens"]:
            raise ValueError("bootstrap positive setup value")
        if field["origin"] == "unset" and value is not None:
            raise ValueError("bootstrap unset value")
        if field["origin"] == "provider_configuration" and (value is None or name not in ["max_prefix_tokens", "max_prompt_tokens"]):
            raise ValueError("bootstrap configuration value")
        if old["value"] != value or old["protection"] != ("preserve" if field["origin"] == "manual" else "replaceable"):
            raise ValueError("bootstrap setup protection")
    sources = payload["sources"]
    if len(sources) != 1 or sources[0]["scope"] != "provider_configuration" or sources[0]["qualification"] != "configured-model-contract-only":
        raise ValueError("bootstrap configuration source")
    if any(item["status"] != "unknown" or item["value"] is not None for item in payload["checked_observations"]):
        raise ValueError("technical bootstrap has no endpoint observations")
    result = {
        "format": "brassclaw-bootstrap-setup-result/1",
        "job_id": data["job_id"],
        "origin_mutation": data["origin_mutation"],
        "target": target,
        "executing_pair": payload["executing_pair"],
        "setup": setup,
        "configuration": sources[0],
        "checked_observations": payload["checked_observations"],
        "readiness": {
            "scope": "configuration-prefill-only",
            "generate": "blocked",
            "missing": ["qualified-endpoint-discovery", "independent-design-acceptance-binding"],
            "design_acceptance": "unknown",
            "serving": "unmeasured"
        }
    }
    return json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
