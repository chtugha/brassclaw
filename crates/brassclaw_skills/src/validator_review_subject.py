def _validator_review_entry(entry):
    if type(entry) is not dict or set(entry) != {"reference", "revision_bytes", "revision"}:
        raise ValueError("complete retained revision required")
    reference = entry["reference"]
    revision = entry["revision"]
    if type(reference) is not dict or set(reference) != {"uuid", "class_code", "version", "checksum"}:
        raise ValueError("complete retained reference required")
    if type(revision) is not dict or type(entry["revision_bytes"]) is not str:
        raise ValueError("complete retained revision required")
    identity = reference["uuid"]
    code = reference["class_code"]
    if type(identity) is not str or not identity or type(code) is not int or not (0 <= code <= 10 or 12 <= code <= 23 or code == 50):
        raise ValueError("retained identity and supported integer class required")
    if type(reference["version"]) is not int or reference["version"] < 1 or reference["version"] > 9223372036854775807:
        raise ValueError("positive retained version required")
    checksum = reference["checksum"]
    if type(checksum) is not str or len(checksum) != 64 or any(c not in "0123456789abcdef" for c in checksum):
        raise ValueError("retained checksum required")
    if revision.get("format") != "component-revision/1" or type(revision.get("class_code")) is not int:
        raise ValueError("complete retained revision required")
    if revision["class_code"] != code or revision.get("uuid") != identity:
        raise ValueError("retained revision identity differs")
    if type(revision.get("document")) is not dict or type(revision.get("dependencies")) is not list:
        raise ValueError("complete retained document required")
    return identity


def _validator_review_base(base, candidate):
    if base is not None:
        _validator_review_entry(base)
        first, second = base["reference"], candidate["reference"]
        if first["uuid"] != second["uuid"] or first["class_code"] != second["class_code"] or first["version"] >= second["version"]:
            raise ValueError("comparison base differs from retained candidate lineage")


def _validator_review_subject(inputs):
    import json
    subject = json.loads(inputs["subject_json"])
    fields = {"format", "submission_id", "actor", "subject_bytes", "subject_checksum", "candidate", "dependencies", "base_comparison"}
    if type(subject) is not dict:
        raise ValueError("complete retained validator subject required")
    kind = subject.get("format")
    if kind == "component-validator-subject/2":
        fields.add("function_partner")
    elif kind != "component-validator-subject/1":
        raise ValueError("complete retained validator subject required")
    if set(subject) != fields or type(subject["dependencies"]) is not list:
        raise ValueError("complete retained validator subject required")
    candidate = subject["candidate"]
    _validator_review_entry(candidate)
    _validator_review_base(subject["base_comparison"], candidate)
    entries = [candidate]
    if kind == "component-validator-subject/2":
        partner = subject["function_partner"]
        if type(partner) is not dict or set(partner) != {"role", "component", "base_comparison"} or partner["role"] not in ["changed", "context"]:
            raise ValueError("complete function partner required")
        other = partner["component"]
        _validator_review_entry(other)
        primary_code = candidate["reference"]["class_code"]
        other_code = other["reference"]["class_code"]
        if primary_code in [1, 2, 3] and other_code == 22:
            skill, code = candidate, other
        elif primary_code == 22 and other_code in [1, 2, 3]:
            skill, code = other, candidate
        else:
            raise ValueError("one Skill and implementing PythonCode required")
        association = skill["revision"].get("association")
        if type(association) is not str:
            raise ValueError("retained Skill association required")
        association = json.loads(association)
        if type(association) is not dict or association.get("format") != "skill-association/1" or association.get("skill_uuid") != skill["reference"]["uuid"] or association.get("python_code_uuid") != code["reference"]["uuid"]:
            raise ValueError("function partner differs from Skill association")
        if partner["role"] == "context" and partner["base_comparison"] is not None:
            raise ValueError("unchanged context is not a changed review root")
        _validator_review_base(partner["base_comparison"], other)
        entries.append(other)
    entries.extend(subject["dependencies"])
    seen = set()
    for entry in entries:
        identity = _validator_review_entry(entry)
        if identity in seen:
            raise ValueError("distinct retained component identities required")
        seen.add(identity)
    # Bases remain comparison data, never executable dependencies or new roots.
    # These shape checks do not authenticate bytes, checksums or prior approval.
    return subject, entries
