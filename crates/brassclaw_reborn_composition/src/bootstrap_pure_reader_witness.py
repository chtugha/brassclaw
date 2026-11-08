def observe_private_review_reader(inputs):
    import json
    operation = inputs["operation"]
    if operation == "entry":
        value = _validator_review_entry(json.loads(inputs["entry_json"]))
        if type(value) is not str:
            raise ValueError("private_reader_entry_return_type")
        observed = {"native_kind": "string", "value": value}
    elif operation == "base":
        value = _validator_review_base(json.loads(inputs["base_json"]), json.loads(inputs["entry_json"]))
        if value is not None:
            raise ValueError("private_reader_base_return_type")
        observed = {"native_kind": "none"}
    elif operation == "subject":
        value = _validator_review_subject({"subject_json": inputs["subject_json"]})
        # The private tuple is observed before serializing its separately tagged
        # members. It never masquerades as a public JSON list result.
        if type(value) is not tuple or len(value) != 2 or type(value[0]) is not dict or type(value[1]) is not list:
            raise ValueError("private_reader_subject_return_type")
        observed = {"native_kind": "tuple2", "first_kind": "dict", "second_kind": "list", "subject": value[0], "entries": value[1]}
    else:
        raise ValueError("private_reader_operation_invalid")
    return json.dumps({"format": "private-review-reader-observation/1", "operation": operation, "return": observed})
