def prepare_post_turn_request(inputs):
    import json
    claim = json.loads(inputs["claim_bytes"])
    if sorted(claim) != ["attempt_ref", "bundle_bytes", "bundle_checksum", "evidence_complete", "format", "prefix_bytes", "prefix_checksum", "selection_bytes"]:
        raise ValueError("invalid review claim")
    if claim["format"] != "completed-turn-review-claim/1" or claim["evidence_complete"] is not True:
        raise ValueError("incomplete review evidence")
    for name in ["attempt_ref", "bundle_bytes", "bundle_checksum", "prefix_bytes", "prefix_checksum", "selection_bytes"]:
        if not isinstance(claim[name], str) or not claim[name]:
            raise ValueError("missing pinned review evidence")
    instructions = "__POST_TURN_INSTRUCTIONS__"
    return json.dumps({
        "format": "completed-turn-sempai-request/1",
        "attempt_ref": claim["attempt_ref"],
        "bundle_checksum": claim["bundle_checksum"],
        "prefix_checksum": claim["prefix_checksum"],
        "selection_bytes": claim["selection_bytes"],
        "messages": [
            {"role": "system", "content": claim["prefix_bytes"]},
            {"role": "system", "content": instructions},
            {"role": "user", "content": claim["bundle_bytes"]}
        ],
        "tools": []
    })
