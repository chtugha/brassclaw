# D01 private Composition Engine bridge retirement

Source-only handoff on base `1e7f664fe42dc67f083623eca818b88a86017a60`.
Native acceptance is pending with the supervisor. No Cargo, tests, database or
Modal execution was performed by this package chat. Standalone rustfmt and
`git diff --check` passed; these do not establish compilation or runtime behavior.

The private `PgThreadEngineStore`/`PgOrchestratorLookup` construction has been
removed with its sole-dependent effect adapter, per-run builder and LLM guard.
Composition supplies `orchestrator_lookup: None`; ordinary work retains the
existing `GlobalMontyDriver` and supervised global owner. The public generic
compatibility port, facade and fail-closed AgentLoop behavior remain. No active
Recipe is withdrawn or reclassified as No-Match. There is no replacement Engine
Thread mapping, synthesized UUID, lifecycle field, counter or timestamp.

The unregistered `memory_doc_libsql_store.rs` source was removed. The separately
registered `migrate-from-libsql` importer, `libsql` feature alias, V016 migration,
WAL-consistent snapshot/backup path and reduction-rules importer remain intact.
`PgCompositionPort` loses only its unused Store field/constructor parameter;
all five callers are adjusted. No historical database bytes, approvals, retained
artifacts, component revisions or existing task selections are rewritten.

## Retired test inventory

[Exact names and original source hashes](2026-11-06-d01-private-engine-bridge-retirement.json)
record 47 retired compiled tests: the original 44 plus three separately reviewed
tests of the sole-dependent LLM guard. The three test functions in the previously
unregistered LibSQL file are listed separately and were never compiled. None of
these removed cases is reported as passing. They tested the retired adapter's
mapping, fabricated Engine identity/defaults, effect translation or construction;
their removal does not certify broader Engine codec or suppression migration.

## Native selectors awaiting supervisor execution

All selectors below are relative to the `brassclaw_reborn_composition` library
test binary. The runtime cases require `postgres` + `skills-db`; importer checks
also require `migrate-from-libsql`. The package's `skills-db` feature implies
`postgres`. Default and `skills-db,migrate-from-libsql,test-support` compilation/
lint coverage remains for the supervisor; no all-features or AWS claim is made.

- `runtime::tests::native_global_runtime_retains_one_root_across_match_and_no_match`:
  actual application boot/worker/Monty path; opaque conversation, original scope,
  thread record and creation timestamp survive matched and unmatched turns. Its
  existing checks retain one root, captured catalogue and correlated outcomes.
- `runtime::tests::model_host_compatibility::global_driver_hands_opaque_admitted_tasks_to_one_existing_service`:
  real PostgreSQL admission, host and global owner; opaque agent/project/owner,
  explicit owner distinct from actor, Some(project) and None, JSON and unparsed
  metadata strings, title, creator and goal. A fresh service reads the same
  originals after execution; the actual row retains creation/deletion fields,
  allows legitimate update-time advancement and stays stable across rereads.
  The contained matcher fixture now applies the installed catalogue's existing
  `default` namespace for absent projects while the actual admitted scope remains
  None. This is not a new production identity conversion.
- `runtime::tests::model_host_compatibility::native_worker_without_monty_fails_original_run_without_model_fallback`:
  genuine admitted PostgreSQL run/claim, native relinquish, real worker/state and
  real host factory without a Monty driver. Expects terminal `driver_unavailable`
  on the original identity, unchanged transcript, no provider/policy work, no
  Monty admission and no queued retry. The exit-evidence adapter is the existing
  distrust-by-default adapter; it cannot manufacture successful exit evidence.
- `runtime::tests::provider_boot_tests::native_legacy_sqlite_import_captures_wal_and_rolls_back_all_tables`:
  preserved actual legacy import/WAL/rollback coverage.
- `reduction_rules_store::tests::native_rulesets_import_preserve_and_replace_without_resurrection`:
  preserved reduction-rules import coverage.
- `pg_composition_port::tests`: five native and five pure mapping cases remain;
  the five native constructors use the reduced signature.

The existing AgentLoop cases `tier_zero_stage_fails_closed_when_no_bridge_wired`,
`tier_zero_stage_fails_closed_when_bridge_returns_none` and
`failed_selected_tier_zero_recipe_never_reaches_model_or_prompt` remain separate
generic contract coverage. They do not substitute for native worker acceptance.

This slice does not complete all B2-09 work, Engine codec/deprecation/suppression
migration, retained native loading/recovery, component activation, deployment or
full-v3 acceptance. B02's private Work/Prepare APIs and tests are unchanged.

## Remaining private composer and seed-contract gate

The repository source search finds no supported production consumer of the
private `PgCompositionPort`: its only constructors are its five native tests,
and no production `with_component_port` wiring is present. Removing its unused
Store parameter does not retire this separate private composer. Its blanket
`allow(dead_code)` (documented as waiting for C.5/C.6) and empty-on-DB-failure
`list_skills` behavior remain unchanged. Generic public Engine `ComponentPort`
and retrieval documentation also still describes that historical wiring.

The packaged requirement at
[`seed_builtin_host.rs`](../../crates/brassclaw_reborn_composition/src/seed_builtin_host.rs)
still says “Composition port (PgCompositionPort) wired into the engine.” Its
adjacent error contract also describes degradation to Non-Matching-Mode. These
bytes are not evidence of a live consumer or of allowed v3 failure routing.
They were not edited or requalified under this cleanup.

B2-09's owning follow-up must establish safe retirement or migration of that
composer and align the seed contract, consumer documentation and applicable
qualification. It must preserve genuine generic consumers, retained originals
and historical evidence; it cannot reinterpret already-approved component bytes
or fabricate a replacement consumer. This gate remains open after D01.
