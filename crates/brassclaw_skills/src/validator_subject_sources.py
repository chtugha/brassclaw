def extract_review_python_sources(inputs):
    subject, entries = _validator_review_subject(inputs)
    sources = {}
    for entry in entries:
        if entry["reference"]["class_code"] == 22:
            document = entry["revision"]["document"]
            if type(document.get("content")) is not str:
                raise ValueError("complete PythonCode source string required")
            sources[entry["reference"]["uuid"]] = document["content"]
    return sources
