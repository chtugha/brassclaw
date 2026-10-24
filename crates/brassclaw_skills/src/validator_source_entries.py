def decode_python_source_entries(inputs):
    import json
    entries = json.loads(inputs["entries_json"])
    if not isinstance(entries, list):
        raise ValueError("source entries must be a JSON list of ID/source pairs")
    sources = {}
    for entry in entries:
        if not isinstance(entry, list) or len(entry) != 2:
            raise ValueError("source entry must contain exactly an ID and source")
        identifier = entry[0]
        source = entry[1]
        if not isinstance(identifier, str) or not identifier or not isinstance(source, str):
            raise ValueError("source IDs must be nonempty strings and sources must be strings")
        if identifier in sources:
            raise ValueError("duplicate source ID")
        sources[identifier] = source
    return sources
