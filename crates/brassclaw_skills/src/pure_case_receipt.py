def encode_original_pure_case_receipt(inputs):
    import json

    observation = json.loads(inputs["case_outcome_json"])
    if type(observation) is not dict:
        raise ValueError("original Pure observation must be an object")
    if observation.get("format") != "original-pure-invocation/1":
        raise ValueError("unsupported original Pure observation")
    # The native owner checks provenance and exact bytes before settling. This
    # encoder neither invents a pass flag nor discards candidate outcome data.
    return json.dumps({"format": "original-pure-case-receipt/1", "observation": observation})
