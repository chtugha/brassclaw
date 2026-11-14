# B2-02 Retained-Selection /2 — Package Handoff (rev 3)

Date: 2026-11-13.
Branch: `codex/b2-02-retained-selection-v2`.
Worktree: `/private/tmp/brassclaw-b2-02-retained-selection`.
Package author: package chat (Bob agent).

## Commits (chronological)

| Commit | Description |
|--------|-------------|
| `9bb386191` | Base — branch point from main |
| `80520a598` | feat(engine): initial /2 format implementation |
| `b43cc50bd` | docs: initial handoff report |
| `abd55c2c3` | fix(migration): rename V214 → V215 |
| `0c6ef93ec` | fix: strict_json + closed fields (review round 2) |
| `27267af66` | docs: handoff report rev 2 |
| `78147448b` | fix(b2-02): move /2 validation into brassclaw_skills::retained_selection_contract |

**Final implementation commit:** `78147448b`

## Changed files at HEAD

| File | Status | SHA-256 |
|------|--------|---------|
| `crates/brassclaw_skills/src/retained_selection_contract.rs` | **new** | `96d8853da516040f6eb3f819597b411d474d5fe4fa9443b6a8d08855d71b4eed` |
| `crates/brassclaw_skills/src/lib.rs` | modified | `2efaa5c4811faf4ff3cbdb9976033c6e216bfb5640a3a67f7302516660cb4fc6` |
| `crates/brassclaw_engine/src/memory/retained_selection_v2.rs` | modified | `406cce1b2762a5a2b2f1bb8c2ca21cf8372de75903d5fe44caefb0d0228fdfe7` |
| `crates/brassclaw_skills/src/review_selection_store.rs` | modified | `708a3e7a72cd95be5e846c917eff1e785560df95b8ffdc02195dd54f08eeaa12` |
| `crates/brassclaw_pg/migrations/V215__retained_selection_v2_format.sql` | new | `0fe8fe88471c6c326ff40fe337ab8ecd07e0ce63c49c63f63e013db3e0751be6` |
| `crates/brassclaw_engine/src/memory/mod.rs` | modified | `1e07758b106e4dbe9b872dd098bbb4b4701d403cff32ae597d5ef82c7e6ce947` |
| `crates/brassclaw_engine/src/memory/retained_instruction.rs` | modified | `9b28dce6910468b1032aef15b85fdd249d46b0529ec076164144a470bf121dbe` |

## Architecture

```
brassclaw_skills::retained_selection_contract   ← validation owner (new)
       ↑                        ↑
review_selection_store     retained_selection_v2  (Engine adapter: re-exports + builder wrapper)
(brassclaw_skills)         (brassclaw_engine)
```

No crate cycle: `brassclaw_engine` already depended on `brassclaw_skills`.
`brassclaw_skills` has no dependency on `brassclaw_engine`.

## Public API — `brassclaw_skills::retained_selection_contract`

| Symbol | Description |
|--------|-------------|
| `FORMAT_V1`, `FORMAT_V2` | Stable format-tag constants |
| `ApprovalProfile` | `pure_own` / `authored_function` |
| `OwnApprovalReference` | Closed 4-field record; private fields, public accessors; checksums as `[u8; 32]` |
| `FunctionalOccurrencePin` | Closed 5-field record; private fields, public accessors |
| `ParsedSelectionV2` | Fully validated `/2` record; private fields, public accessors |
| `SelectionVersion` | `V1` / `V2(ParsedSelectionV2)` |
| `parse_selection_version(bytes)` | Single version-dispatched entry point |
| `build_v2_bytes(...)` | Builder with round-trip validation; returns `(String, [u8; 32])` |
| `BASE_FIELDS` | The seven base field keys |
| `is_valid_python_identifier(name)` | Shared identifier validator |

## Validation contracts (all in `retained_selection_contract`)

- `strict_json` (DuplicateKeys::Reject + REVISION_LIMITS) at both parse entry and builder round-trip.
- `ComponentRevisionRef` deserialized via strict serde Visitor; `validated_component_ref()` additionally rejects nil UUID and version == 0 (serde Visitor accepts both).
- Checksums stored as `[u8; 32]` decoded from 64-char lowercase hex; `hex_encode()` produces the wire form.
- `closed_object()` enforces exact field counts: `OwnApprovalReference` (4), `FunctionalOccurrencePin` (5), outer document (8 base + declared optionals only).
- `association_approvals` as non-object → `Err`.
- `workflow_class`, `step_order`, `input_layout` validated in `parse_v2_inner`.
- Every `functional_approvals` step_id must appear in `step_order`; FA and AA step_ids are disjoint.
- `implementation.class_code` must equal 22.

## review_selection_store.rs

`verify_program_selection` calls `parse_selection_version` as its first action. For `/2` this performs the complete structural validation (all pin fields, nil/zero checks, step_id cross-reference, duplicate-key rejection) before any bytes are persisted. For `/1` the format tag is verified and the existing recipe/component graph resolution continues unchanged. No validation is deferred to a later Engine call.

## Tests

25 unit tests in `brassclaw_skills::retained_selection_contract::selection_contract_tests`.

**Native test selector (supervisor, requires compilation):**
```
cargo test -p brassclaw_skills selection_contract_tests
```

| Test | Coverage |
|------|----------|
| `v1_format_tag_returns_v1_variant` | /1 → `V1` |
| `unknown_format_tag_is_rejected` | unknown tag → `Err` |
| `non_json_is_rejected` | malformed bytes → `Err` |
| `duplicate_top_level_key_is_rejected` | duplicate outer key → `Err` via `strict_json` |
| `duplicate_nested_key_in_pin_is_rejected` | duplicate pin key → `Err` via `strict_json` |
| `valid_v2_round_trips_exactly` | build → parse → fields + checksum |
| `authored_function_profile_round_trips` | `authored_function` preserved |
| `model_input_binding_is_preserved` | optional field survives round-trip |
| `association_approvals_coexist_with_functional_approvals` | disjoint step_ids |
| `serialization_is_deterministic` | identical input → identical bytes |
| `nil_uuid_in_implementation_is_rejected` | nil UUID → `Err` (post-serde check) |
| `zero_version_in_implementation_is_rejected` | version 0 → `Err` (post-serde check) |
| `nil_uuid_in_own_approval_root_is_rejected` | nil UUID in root → `Err` |
| `zero_version_in_own_approval_root_is_rejected` | version 0 in root → `Err` |
| `non_python_code_class_in_implementation_is_rejected` | `class_code != 22` → `Err` |
| `step_id_not_in_step_order_is_rejected` | FA step_id absent from step_order → `Err` |
| `overlapping_step_ids_are_rejected` | same step_id in FA and AA → `Err` |
| `non_object_association_approvals_is_rejected` | array AA → `Err` |
| `empty_functional_approvals_is_rejected` | empty FA → `Err` |
| `missing_base_field_is_rejected` | each of 7 base fields removed → `Err` |
| `extra_top_level_field_is_rejected` | injected `"approved":true` → `Err` |
| `reserved_python_identifier_is_rejected` | 5 reserved words → `Err` |
| `own_approval_key_mismatch_is_rejected` | UUID key ≠ root.uuid → `Err` |
| `format_constants_are_distinct_and_stable` | constant stability |
| `hex_encode_produces_64_char_lowercase_hex` | encoding + SHA-256 self-consistency |

## Known gaps (unchanged from rev 2)

1. **B1-05/B1-06 prerequisite.** `build_v2_bytes` must only be called after B1 provides real trusted approval records.
2. **`retained_instruction.rs::retained_selection()` not yet updated** to call `build_v2_selection` when functional approvals are available. Wiring is B2-06.
3. **Catalogue readers** (`installed_monty_catalogue.rs`, `authored_monty_catalogue.rs`) still compare against `/1`; versioned comparison is B2-06.
4. **SQL migration V215** must be tested on a real PostgreSQL instance.
5. **No `execution_profile` extension** (N-01). Separate successor package.

## Contracts preserved

- All existing `/1` bytes, stored rows, and parsers are unchanged.
- `RetainedRecipeSelection::from_parts` is `pub(super)` — not reachable outside `memory::`.
- `parse_selection_version` returns `Err` on any unknown format tag.
- No approval, Tool permission, kernel grant or catalogue activation follows from parsing or emitting a `/2` record.
