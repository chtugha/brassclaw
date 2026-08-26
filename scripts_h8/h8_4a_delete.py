"""H8.4a — delete the obsolete active_skills provenance mechanism.

Line-range deletions across 5 files with assertions on every boundary.
Aborts WITHOUT writing anything if any assertion fails (safe across re-runs).
Surgical single-line edits (imports/re-exports/doc-comments) are done separately
via the Edit tool on the post-deletion files.
"""
import sys

FILES = {
    "crates/brassclaw_engine/src/executor/orchestrator.rs": [
        # D1: __set_active_skills__ dispatch arm (comment + arm + trailing blank)
        {"start": 761, "end": 763,
         "start_sub": "// __set_active_skills__(skills)",
         "end_sub": None,  # blank
         "before": ("blank", 760),
         "after": ("contains", 764, "// __validate_component__")},
        # D2: "skill_activated" event match arm (compact, no trailing blank)
        {"start": 2471, "end": 2479,
         "start_sub": '"skill_activated" => {',
         "end_sub": "}",  # arm close
         "before": ("strip", 2470, "}"),
         "after": ("contains", 2480, '"budget_warning" => {')},
        # D3: handle_set_active_skills fn + doc + trailing blank
        {"start": 3887, "end": 3911,
         "start_sub": "/// Handle `__set_active_skills__(skills)`.",
         "end_sub": None,  # blank
         "before": ("blank", 3886),
         "after": ("contains", 3912, "/// Handle `__validate_component__")},
    ],
    "crates/brassclaw_engine/src/types/thread.rs": [
        # D5+D6: ActiveSkillProvenance struct + doc + blank + const + blank
        {"start": 194, "end": 207,
         "start_sub": "/// Provenance for a skill that was active during thread execution.",
         "end_sub": None,  # blank
         "before": ("blank", 193),
         "after": ("contains", 208, "// ── Thread ─")},
        # D7: set_active_skills + active_skills fns + doc + trailing blank
        {"start": 350, "end": 379,
         "start_sub": "/// Persist active skill provenance in thread metadata.",
         "end_sub": None,  # blank
         "before": ("blank", 349),
         "after": ("contains", 380, "/// Transition to a new state, recording an event.")},
        # D8: active_skill_provenance_roundtrips_through_metadata test + preceding blank
        {"start": 685, "end": 700,
         "start_sub": None,  # blank at 685
         "end_sub": "}",
         "before": ("strip", 684, "}"),
         "after": ("strip", 701, "}")},
    ],
    "crates/brassclaw_engine/src/executor/db_skill_loader.rs": [
        # D10: fetch_skill_provenance_by_ids fn + doc + trailing blank
        {"start": 98, "end": 178,
         "start_sub": "/// Fetch the active-skill provenance (`doc_id`, `name`, `version`) for a set",
         "end_sub": None,  # blank
         "before": ("blank", 97),
         "after": ("contains", 179, "// ---")},
    ],
    "crates/brassclaw_engine/orchestrator/default.py": [
        # D12: _set_active_skills_from_matched_ids def + 2 trailing blanks
        {"start": 992, "end": 1023,
         "start_sub": "def _set_active_skills_from_matched_ids(matched_component_ids, state, active_skills):",
         "end_sub": None,  # blank
         "before": ("blank", 991),
         "after": ("contains", 1024, "def _parse_orchestrator_channel_steps(orchestrator_content):")},
    ],
    "tests/engine_v2_skill_codeact.rs": [
        # D14+D15: pg_rig module + doc + blank + skill_codeact test + doc + trailing blank
        {"start": 666, "end": 874,
         "start_sub": "/// Per-test Postgres-16 testcontainer rig (v3 Phase H4.8).",
         "end_sub": None,  # blank
         "before": ("blank", 665),
         "after": ("contains", 875, "/// Verify that non-matching goals don't activate skills")},
    ],
}


def check_file(path, ranges):
    with open(path) as fh:
        lines = fh.readlines()
    n = len(lines)

    def L(i):
        return lines[i - 1]

    def assert_contains(i, sub):
        assert 1 <= i <= n, f"{path}: line {i} OOB (n={n})"
        assert sub in L(i), f"{path}:{i}: expected {sub!r}\n  got {L(i)!r}"

    def assert_blank(i):
        assert 1 <= i <= n, f"{path}: line {i} OOB (n={n})"
        assert L(i).strip() == "", f"{path}:{i}: expected blank\n  got {L(i)!r}"

    def assert_strip(i, expected):
        assert 1 <= i <= n, f"{path}: line {i} OOB (n={n})"
        assert L(i).strip() == expected, f"{path}:{i}: expected stripped {expected!r}\n  got {L(i)!r}"

    delete = [False] * (n + 2)
    for r in ranges:
        s, e = r["start"], r["end"]
        if r["start_sub"] is not None:
            assert_contains(s, r["start_sub"])
        else:
            assert_blank(s)
        if r["end_sub"] is None:
            assert_blank(e)
        else:
            assert_strip(e, r["end_sub"])
        kind = r["before"][0]
        if kind == "blank":
            assert_blank(r["before"][1])
        elif kind == "strip":
            assert_strip(r["before"][1], r["before"][2])
        akind = r["after"][0]
        if akind == "contains":
            assert_contains(r["after"][1], r["after"][2])
        elif akind == "strip":
            assert_strip(r["after"][1], r["after"][2])
        for i in range(s, e + 1):
            assert not delete[i], f"{path}: overlap at line {i}"
            delete[i] = True
        print(f"  {path}: verified range {s}-{e}")
    kept = [lines[i] for i in range(n) if not delete[i + 1]]
    return kept


print("H8.4a deletion — verifying all boundaries before writing...")
results = {}
for path, ranges in FILES.items():
    results[path] = check_file(path, ranges)

for path, kept in results.items():
    with open(path, "w") as fh:
        fh.writelines(kept)
    print(f"  wrote {path} ({len(kept)} lines)")
print("H8.4a deletion complete.")
