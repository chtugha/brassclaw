def encode_python_structure_results(inputs):
    import json
    return json.dumps({
        "format": "validator-step-results/1",
        "results": {
            "0:1": inputs["sources"],
            "0:3": inputs["inventory_json"],
            "0:4": inputs["definitions"],
            "0:5": inputs["contracts"],
            "0:6": inputs["checks"],
            "0:7": inputs["aggregation"],
        },
    })
