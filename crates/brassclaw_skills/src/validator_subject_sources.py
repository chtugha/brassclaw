def extract_review_python_sources(inputs):
    import json
    subject = json.loads(inputs["subject_json"])
    if not isinstance(subject, dict) or subject.get("format") != "component-validator-subject/1":
        raise ValueError("complete retained validator subject required")
    if set(subject) != {
        "format", "submission_id", "actor", "subject_bytes", "subject_checksum",
        "candidate", "dependencies", "base_comparison",
    } or not isinstance(subject["dependencies"], list):
        raise ValueError("complete retained validator subject required")
    sources = {}
    seen = set()
    # The comparison base is deliberately not an executable dependency. The
    # sealed store reader verifies exact bytes, hashes and the selected graph;
    # this pure transformation neither establishes nor grants that provenance.
    for entry in [subject["candidate"]] + subject["dependencies"]:
        if not isinstance(entry, dict) or set(entry) != {"reference", "revision_bytes", "revision"}:
            raise ValueError("complete retained revision required")
        reference = entry["reference"]
        revision = entry["revision"]
        if not isinstance(reference, dict) or not isinstance(revision, dict) or not isinstance(entry["revision_bytes"], str):
            raise ValueError("complete retained revision required")
        if set(reference) != {"uuid", "class_code", "version", "checksum"} or revision.get("format") != "component-revision/1":
            raise ValueError("complete retained revision required")
        identity = reference.get("uuid")
        class_code = reference.get("class_code")
        if not isinstance(identity, str) or not identity or identity in seen:
            raise ValueError("distinct retained component identities required")
        if type(class_code) is not int or type(revision.get("class_code")) is not int or revision.get("class_code") != class_code or revision.get("uuid") != identity:
            raise ValueError("retained revision identity differs")
        seen.add(identity)
        if class_code == 22:
            document = revision.get("document")
            if not isinstance(document, dict) or not isinstance(document.get("content"), str):
                raise ValueError("complete PythonCode source string required")
            sources[identity] = document["content"]
    return sources
