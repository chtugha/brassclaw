#!/usr/bin/env python3
"""H8.4 — delete the dormant Model A PK production path + obsolete tests.

Performs line-range deletions on crates/brassclaw_engine/src/executor/orchestrator.rs
with assertions on every boundary line. Aborts without writing if any assertion
fails. String replacements (param rename, phase_f7 rewrites, doc updates) are
done separately via the Edit tool on the post-deletion file.
"""
import sys

F = "crates/brassclaw_engine/src/executor/orchestrator.rs"
with open(F) as fh:
    lines = fh.readlines()  # lines[i] == 1-indexed line (i+1)

n = len(lines)
print(f"read {n} lines from {F}")

def L(idx1):  # 1-indexed accessor
    return lines[idx1 - 1]

def assert_contains(line_no, substr):
    assert 1 <= line_no <= n, f"line {line_no} OOB (n={n})"
    line = L(line_no)
    assert substr in line, (
        f"line {line_no}: expected {substr!r}\n  got {line!r}"
    )

def assert_blank(line_no):
    assert 1 <= line_no <= n, f"line {line_no} OOB (n={n})"
    line = L(line_no)
    assert line.strip() == "", (
        f"line {line_no}: expected blank\n  got {line!r}"
    )

def assert_strip(line_no, expected):
    assert 1 <= line_no <= n, f"line {line_no} OOB (n={n})"
    line = L(line_no)
    assert line.strip() == expected, (
        f"line {line_no}: expected stripped {expected!r}\n  got {line!r}"
    )

# Deletion ranges: (start, end) 1-indexed inclusive.
# For each: assert start-line content, end-line blank, before-line boundary,
# and after-line content (the line that remains at position `end+1`).
ranges = [
    # PD1: dispatch arm comment + __assemble_prior_knowledge__ arm + trailing blank
    {
        "start": 772, "end": 799,
        "start_sub": "// __assemble_prior_knowledge__(goal, token_budget, sender_class_code)",
        "before": ("blank", 771),
        "after": ("contains", 800, "// __fetch_component__"),
    },
    # PD3: handle_assemble_prior_knowledge doc + fn + trailing blank
    {
        "start": 2692, "end": 2927,
        "start_sub": "/// Handle `__assemble_prior_knowledge__",
        "before": ("blank", 2691),
        "after": ("contains", 2928, "/// Handle `__fetch_component__"),
    },
    # PD4: helper cluster (skill_provenance_for_items, assemble_from_component_items,
    #      assemble_component_strings, CLASS_CODE_*, doc_type_class_code,
    #      format_prior_knowledge_for_llm) + trailing blank
    {
        "start": 3672, "end": 3898,
        "start_sub": "/// Extract the skill-class (class 1\u20133) ids from `items`",
        "before": ("blank", 3671),
        "after": ("contains", 3899, "/// Handle `__check_budget__"),
    },
    # TD1a: run_python_step0 helpers (Step0Recording/kwargs_to_json/class_code_arg/
    #       run_python_step0) + section comment + trailing blank
    {
        "start": 5393, "end": 5607,
        "start_sub": "// \u2500\u2500 Phase G.8 \u2014 step-0 `run_loop` harness",
        "before": ("blank", 5392),
        "after": ("contains", 5608, "// \u2500\u2500 __regex_match__"),
    },
    # TD1b: G.8 step-0 dispatch unit tests (working_messages_has_user + 6 step0_* tests)
    #       + section comment + trailing blank
    {
        "start": 5657, "end": 6021,
        "start_sub": "// \u2500\u2500 Phase G.8 \u2014 \u00a70.9 v3 step-0 dispatch unit tests",
        "before": ("blank", 5656),
        "after": ("contains", 6023, "load_orchestrator_without_store"),
    },
    # TD2: format_prior_knowledge_for_llm test section (make_plan_doc/make_skill_doc +
    #       3 format_prior_knowledge tests + both_surfaces + raw_and_formatted_distinct)
    #       + section comment + trailing blank
    {
        "start": 9160, "end": 9372,
        "start_sub": "// \u2500\u2500 format_prior_knowledge_for_llm (Phase 6.1 / Phase 8 Step 8.1)",
        "before": ("blank", 9159),
        "after": ("contains", 9373, "// \u2500\u2500 Phase F.7: RetrievalSource arm tests"),
    },
    # TD3: phase_g1_active_skills_emitted_in_every_arm (doc + test + trailing blank)
    {
        "start": 9712, "end": 9783,
        "start_sub": "/// Phase G.1 (Q-G3) \u2014 every",
        "before": ("blank", 9711),
        "after": ("contains", 9784, "/// Phase G.2 \u2014"),
    },
]

# Verify all boundaries BEFORE deleting anything.
for r in ranges:
    s, e = r["start"], r["end"]
    assert_contains(s, r["start_sub"])
    assert_blank(e)
    kind = r["before"][0]
    if kind == "blank":
        assert_blank(r["before"][1])
    elif kind == "strip":
        assert_strip(r["before"][1], r["before"][2])
    akind = r["after"][0]
    if akind == "contains":
        assert_contains(r["after"][1], r["after"][2])
    print(f"  verified range {s}-{e}")

# Build keep-mask (delete the union of ranges).
delete = [False] * (n + 2)  # 1-indexed
for r in ranges:
    for i in range(r["start"], r["end"] + 1):
        assert not delete[i], f"overlap at line {i}"
        delete[i] = True

kept = [lines[i] for i in range(n) if not delete[i + 1]]
print(f"keeping {len(kept)} lines (deleted {n - len(kept)})")

with open(F, "w") as fh:
    fh.writelines(kept)
print(f"wrote {F}")
