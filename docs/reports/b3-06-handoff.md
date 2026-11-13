# B3-06 Durable Q1-Work Design — Handoff Report

**Date:** 2026-11-13
**Branch:** `codex/b3-06-durable-q1-work-wt` (worktree at `/Volumes/SSDE/brassclaw-b3-06`)
**Commits (oldest → newest):**
- `9169746ce` — promote `#[cfg(test)]`-gated preparation chain to production
- `02798b1a0` — selection-retention path, Phase 1/2 split, initial tests *(superseded below)*
- `8f46752d6` — coherent snapshot, commit custody, honest fault tests *(superseded below)*
- `6655c63e0` — report update (outdated)
- `ad1ea1b90` — scope correction, deadline custody, AckCut test, defer-only reconciler
- `ec039b38e` — fix three reviewer findings (write_result skip, test duplication, AckCut Err acceptance)

---

## Summary

Six rounds of work completed the durable Q1-work path, correctly scoped
B1-07/B2-04 responsibilities, and addressed all three review findings from
the first post-commit inspection:

1. Promote the `#[cfg(test)]`-gated preparation chain to production.
2. Add the Phase 1/2 split in the reconciler, initial tests.
3. Fix coherent revision snapshot and commit custody.
4. Final scope correction: remove `retain_validator_selection_for_submission`;
   reconciler defers when no selection is present; `bounded_for` deadline on
   write tx; AckCut test.
5. Fix three reviewer findings from first post-commit inspection (see below).

---

## What Was Implemented

### Core change — commit `9169746ce`
The validator-original preparation path was `#[cfg(test)]`-only. Five guards removed:

- `private_task_catalogue.rs` — `ValidatorOriginal`, `prepare_private_validator`,
  `prepare_fresh_private_validator`, `read_private_validator_original`
- `global_monty_driver.rs` — `GlobalTaskPortsFactory::prepare_private_validator` default
- `global_task_factory.rs` — `MontyCatalogueProvider` trait default + `OwnedGlobalTaskFactory` impl
- `installed_monty_catalogue.rs` — `MontyCatalogueProvider` impl

### Reconciler Phase 1/Phase 2 split — commit `02798b1a0` → finalised in `ad1ea1b90`
`reconcile_originals` reads state under a RepeatableRead snapshot (Phase 1,
inside `tokio::time::timeout`), then executes the resulting action outside the
timeout (Phase 2). This ensures the write-tx deadline is owned by
`prepare_private_validator` via `bounded_for`, not the outer reconciler deadline.

### B1-07/B2-04 scope boundary — commit `ad1ea1b90`
`retain_validator_selection_for_submission` was removed from
`MontyCatalogueProvider`, `OwnedGlobalTaskFactory`, and `InstalledMontyCatalogue`.
Creating a validator selection from draft heads in production belongs to
B1-07/B2-04. The reconciler now:
- When `selection_preparation_present == false` → logs a debug message and
  takes `Action::None` (defer to next wake cycle).
- When `selection_preparation_present == true` → takes `Action::PrepareOriginal`
  (calls `prepare_private_validator`).

### Deadline custody — commit `ad1ea1b90`
`prepare_fresh_private_validator` delegates to
`prepare_fresh_private_validator_with_write_pool(original, &self.pool)`.
That method wraps the entire write transaction (advisory lock + INSERT +
definition seal + COMMIT) inside `bootstrap_evidence::io::bounded_for`, matching
the existing prefix-source pattern. The post-COMMIT readback also runs inside
`bounded_for`. `TransactionClient` custody (`acquire` + `recycle_after_commit`)
ensures uncertain connections are never returned to the pool.

### Test-only write-pool injection — commit `ad1ea1b90`
`#[cfg(test)] prepare_private_validator_with_write_pool` on `InstalledMontyCatalogue`
uses a caller-supplied pool for the write connection while reading and doing the
final readback through `self.pool`. Required for `AckCutTransport` fault injection.

### Reviewer findings — commit `ec039b38e`

Three issues identified in the first post-commit inspection of `ad1ea1b90`:

**Finding 1 — `write_result?` skipped the bounded readback on write errors.**
The inner readback closure opened with `write_result?`, which short-circuited
before `store.get_private_work_state()` whenever the write returned any error,
including an unknown-commit error from a lost COMMIT ACK. That meant the
readback — the only mechanism for resolving a genuine unknown-commit — was
silently bypassed in exactly the case it was designed for.

Fix: introduce a local `enum WriteOutcome { Committed, UnknownCommit(err),
DefiniteFailure(err) }`. Every pre-COMMIT error (advisory lock, acquire,
transaction start, INSERT fault, pre-checks) is mapped to `DefiniteFailure`
immediately at the point it arises — no INSERT occurred, skip the readback.
Only the `tx.commit()` call itself can produce `UnknownCommit`. The
`bounded_for` timeout (outer deadline) is treated as `UnknownCommit` via
`unwrap_or_else`. Both `Committed` and `UnknownCommit` now unconditionally
execute the bounded readback on `self.pool`. The wake is fired only on
`Committed`.

**Finding 2 — test entry point duplicated the production flow.**
`#[cfg(test)] prepare_private_validator_with_write_pool` contained the same
~60-line read-only setup (subject / selection / actor / root in one
RepeatableRead snapshot) as `prepare_private_validator`. Any future change
to the read logic had to be applied in both places.

Fix: extract the shared read-only setup into a new private method
`read_validator_original`. Both `prepare_private_validator` and
`prepare_private_validator_with_write_pool` now call `read_validator_original`
and delegate to `prepare_fresh_private_validator_with_write_pool`. The
production path and the test injection point are now identical in structure —
only the write-pool parameter differs.

**Finding 3 — AckCut test accepted any error from the first call.**
The test comment said "The result may be Ok or Err (readback timed out —
extremely unlikely)" and then only asserted `count == 1`. This allowed the
test to pass even when `prepare_private_validator_with_write_pool` returned
`Err` for a reason unrelated to the COMMIT ACK loss — including a case where
no INSERT occurred at all.

Fix: add `assert!(result.is_ok(), ...)` and `assert_eq!(work_id)` immediately
after the first call. With `WriteOutcome::UnknownCommit` always running the
bounded readback on `self.pool`, the readback must find the committed row and
return `Ok`. An `Err` is now a genuine test failure. The test docstring is
updated to explain the assertion.

---

## Files Changed

| File | Net changes |
|------|-------------|
| `private_task_catalogue.rs` | 4 `#[cfg(test)]` guards removed; `read_validator_original` extracted (shared setup); `prepare_private_validator` and `#[cfg(test)] prepare_private_validator_with_write_pool` delegate to it; write tx uses `WriteOutcome` enum for precise commit/unknown/failure classification; `bounded_for` deadline on both write and readback; readback always runs for `Committed` and `UnknownCommit` |
| `global_task_factory.rs` | `retain_validator_selection_for_submission` removed from trait default + `OwnedGlobalTaskFactory` impl; `prepare_private_validator` ungated |
| `installed_monty_catalogue.rs` | `retain_validator_selection_for_submission` removed from `MontyCatalogueProvider` impl; `prepare_private_validator` ungated |
| `global_monty_driver.rs` | `prepare_private_validator` default ungated (commit `9169746ce`) |
| `review_work_reconciliation.rs` | Phase 1/2 split; `RetainSelection` action removed; `!selection_preparation_present` → `Action::None` with debug log |
| `runtime/tests/private_delivery_tests.rs` | 4 B3-06 acceptance tests (Cases 1, 2, 3a, 3b, 3c) |
| `runtime/tests/validator_retained_q1_tests.rs` | `queued_structural_original_with_selection` test helper |

---

## Tests

| Test | What it proves |
|------|----------------|
| `reconciler_defers_when_qualified_selection_absent` | Reconciler takes no action (no `private_work` row) when `selection_preparation_present == false` |
| `reconciler_creates_private_work_without_wake_when_selection_present` | Fresh reconciler (no prior wake) creates `private_work` row when a qualified selection is pre-seeded |
| `prepare_private_validator_concurrent_winner_returns_existing_original_without_replay` | Advisory-lock idempotency: second call returns same immutable state; no duplicate row |
| `prepare_private_validator_commit_abort_retries_cleanly_without_duplicate` | BEFORE INSERT trigger fault causes real rollback; sequence confirms trigger fired; zero rows after fault; exactly one row after retry |
| `prepare_private_validator_pg_commit_ack_loss_reads_back_without_replay` | `AckCutTransport` intercepts the COMMIT ACK for the validator-original write tx (matched on `brassclaw-validator-original:` advisory-lock parameter prefix); uncertain connection is detached; bounded readback on `self.pool` resolves the committed row; exactly one row; `cut_count == 1` |

---

## Validation Status

### Format
```bash
cargo fmt -p brassclaw_reborn_composition   # applied clean, no diff
```

### Joint acceptance (required before merge)

All changed production modules are gated behind
`#[cfg(all(feature = "postgres", feature = "skills-db"))]`.

```bash
# Clippy with features:
cargo clippy -p brassclaw_reborn_composition \
  --features postgres,skills-db --all-targets -- -D warnings

# Integration tests (require live PostgreSQL):
cargo test -p brassclaw_reborn_composition \
  --features postgres,skills-db,root-llm-provider \
  reconciler_defers_when_qualified_selection_absent

cargo test -p brassclaw_reborn_composition \
  --features postgres,skills-db,root-llm-provider \
  reconciler_creates_private_work_without_wake

cargo test -p brassclaw_reborn_composition \
  --features postgres,skills-db,root-llm-provider \
  prepare_private_validator_concurrent_winner

cargo test -p brassclaw_reborn_composition \
  --features postgres,skills-db,root-llm-provider \
  prepare_private_validator_commit_abort

cargo test -p brassclaw_reborn_composition \
  --features postgres,skills-db,root-llm-provider \
  prepare_private_validator_pg_commit_ack_loss
```

---

## Open Points

1. **Joint acceptance clippy + integration tests** — `--features postgres,skills-db`
   clippy and the five new tests must pass before merge. No compilation was run
   as directed; compilation and test execution are the orchestrator's
   responsibility.

2. **`run_q1_validation_retained` remains `#[cfg(test)]`** — correct. Production
   path is reconciler-driven: selection created by B1-07/B2-04 → reconciler
   calls `prepare_private_validator`. `run_q1_validation_retained` is the
   synchronous test path that also awaits native settlement.

3. **`ValidatorTaskDriver::submit` remains `#[cfg(test)]`** — correct.

4. **Build lock** — `/private/tmp/brassclaw-target-cargo.lock` must only be
   released via `flock(LOCK_UN)` / fd close, never deleted.

5. **Merge path** — fast-forward `main` ← `codex/b3-06-durable-q1-work-wt`
   after joint acceptance. Changes to build scripts, call scripts, and
   documentation remain the orchestrator's responsibility.

---

## Second-round review findings — commit `6c24e5e33`

Five findings from the second post-commit inspection of `ec039b38e`:

**F1 — Type conflict:** `bounded_for` requires `Future<Output = Result<T, RebornBuildError>>`. The `WriteOutcome` enum made the closure return `Result<WriteOutcome, WriteOutcome>`, which does not satisfy that bound. Fix: remove `WriteOutcome` entirely. Classification is carried through a `Cell<bool> commit_reached` flag declared outside the closure and set synchronously just before `tx.commit()`. The closure type is `Result<(), RebornBuildError>` again. After `bounded_for` returns, the flag distinguishes pre-COMMIT failures (skip readback) from post-COMMIT unknown outcomes (run readback).

**F2 — `result` used after `unwrap()`:** `result.unwrap()` on line N moved out of `result`; a later format `{result:?}` in the count assert was a use-after-move. Fix: use a single `.expect("...")` that binds the state into `state_a`; remove all subsequent references to `result`.

**F3 — Unbounded reads:** `read_validator_original` and `read_private_validator_original` used raw `self.pool.get()` without a deadline. Both now wrap their entire RepeatableRead SQL block in `bounded_for` with stage names `"private-validator-read-original"` and `"private-validator-verify-original"` respectively. The native codec/inspector in `read_private_validator_original` runs outside the closure (no PG connection held) and is not deadline-bounded.

**F4 — Shared recovery path:** Production and test-injection callers share `read_validator_original` + `prepare_fresh_private_validator_with_write_pool`. Their recovery `match` blocks are structurally identical. Extracting them into a shared helper would require a closure or trait over the pool parameter and would add complexity without benefit given only two callers. The acknowledged duplication is intentional.

**F5 — ACK test checksum + pool saturation:** Added `state_a.request_checksum` non-empty assertion; moved `cut_count == 1` to immediately after the first call; added `fault_pool.get()` with zero timeout to prove the capacity-1 pool is saturated (uncertain connection detached, not recycled); renamed `state → state_b` for the second call and asserted `state_a.request_checksum == state_b.request_checksum` to prove both calls resolved the same immutable row.

**Report clarification:** "error before COMMIT" means the INSERT transaction was not committed (the server rolled it back or it never reached COMMIT). A trigger fault can reach and attempt an INSERT but still abort; that is still a not-committed case and the readback is not needed.

---

## Commit Details

```
9169746ce  B3-06: promote durable Q1-work preparation to production path
02798b1a0  B3-06: add production selection-retention path, fix timeout custody, add tests
f5493ff04  B3-06: update handoff report (feature-gate note, correct test commands, lock policy)
8f46752d6  B3-06: fix coherent snapshot, commit custody, and honest fault tests
6655c63e0  B3-06: update report (coherent snapshot, commit custody, fault test)
ad1ea1b90  B3-06: scope, deadline custody, AckCut test, and defer-only reconciler
ec039b38e  B3-06: fix three reviewer findings from ad1ea1b90
6b918828f  B3-06: update report for reviewer findings (ec039b38e)
6c24e5e33  B3-06: fix second-round review findings from ec039b38e
2b907d3da  B3-06: update report for second-round findings (6c24e5e33)
e4b6adedb  B3-06: fix third-round review findings from 6c24e5e33
ec5a821af  B3-06: update report for third-round findings (e4b6adedb)
d94aaa8e1  B3-06: fix fourth-round review findings from e4b6adedb
07e0a4656  B3-06: update report for fourth-round findings (d94aaa8e1)
f97bd0e3d  B3-06: fix fifth-round review findings from d94aaa8e1
583d9191e  B3-06: update report for fifth-round findings (f97bd0e3d)
530a6855f  B3-06: fix Clippy failures from first compilation run
```

---

## Clippy failures from first compilation run — commit `530a6855f`

Four issues found by exit-101 Clippy run after `f97bd0e3d`:

**1 — `PreparedValidatorReview::prepare()` was `#[cfg(test)]`:**
Called by the production path `prepare_fresh_private_validator` → `PreparedValidatorReview::prepare()`. Removing the gate is the minimal correct fix. All identity/worker/limit/input checks are preserved — `prepare()` calls `inspect()` internally which verifies subject/selection consistency, worker bytes, limit constraints and program inspection.

**2 — `ConfirmDelivery` branch returned `Result<Option<Value>, _>` instead of `Result<(), _>`:**
`confirm_original_with_source` returns `Result<Option<Value>, AgentLoopDriverError>`. The `PrepareOriginal` and `None` arms return `Result<(), _>`. Added `.map(|_| ())` to the `ConfirmDelivery` branch. Error propagation and the shared confirmation API unchanged.

**3 — Factory/Provider `prepare_private_validator` methods had only test consumers:**
The production reconciler calls `self.catalogue.prepare_private_validator(id)` directly on the concrete `Arc<InstalledMontyCatalogue>`, not through any trait. The trait-level methods have only test consumers (via `ValidatorTaskDriver → GlobalMontyDriver → GlobalPrivateCompletion`). Re-added `#[cfg(test)]` to: `GlobalTaskPortsFactory::prepare_private_validator` (trait default), `MontyCatalogueProvider::prepare_private_validator` (trait default and `OwnedGlobalTaskFactory` impl), and `MontyCatalogueProvider for Arc<InstalledMontyCatalogue>` impl. The concrete `InstalledMontyCatalogue::prepare_private_validator` stays ungated.

**4 — `prepare_private_validator_with_write_pool` now unused:**
Removed the test-only write-pool injection method. Merged `prepare_fresh_private_validator_with_write_pool` back into `prepare_fresh_private_validator` (single method, no `write_pool` parameter). The write transaction now uses `&self.pool` directly, which is the proxy pool when Case 3c builds the runtime with `proxy_pool`.

---

## Fifth-round review findings — commit `f97bd0e3d`

**Issue 1 — Worker race in all five new B3-06 tests:**
All five tests called `start()` and immediately proceeded to seed fixtures and call `prepare_private_validator()` without stopping the background worker. The worker can race to prepare the original before the test-controlled action runs, causing spurious success, wrong row counts, or unintended trigger firing.

Fix: add `crate::runtime::test_pg::stop_worker_for_state_fixture(&runtime).await` immediately after `start()` in Cases 1–3c. For Case 3c, this runs after runtime start and before fixture seeding, which is also before transport arming.

Add `count_before == 0` pre-action assertions in Cases 3a, 3b, and 3c to make a silent race into a visible test failure.

**Issue 2 — Shutdown order in Case 3c:**
Fixed from `proxy_pool.close() → transport.shutdown() → runtime.shutdown()` to `runtime.shutdown() → proxy_pool.close() → transport.shutdown()`. The runtime must fully shut down (all background tasks complete) before the pool they use is closed.

**PID assertion clarification (note, no code change):**
`recovery_pid != first_pid` proves the pool issued a new PG session after the detach freed the slot. It does not prove the old backend has OS-terminated (non-deterministic), only that a new session was established — consistent with the reviewer's clarification.

---

## Fourth-round review findings — commit `d94aaa8e1`

**F3 (complete) — Full recovery path via shared proxy pool:**
The previous fix was only partial. The test still used `prepare_private_validator_with_write_pool()` to inject the fault pool for the write only, while the readback ran through a separate catalogue pool. This proved recovery over a split path, not over the same pool as required.

New approach: build the runtime with the AckCutTransport proxy as `installed_catalogue.pool` (capacity 1). Both write and readback run through the same pool — the production path. The runtime is built via `RebornBuildInput::postgres_with_reborn_home(name, proxy_pool.clone(), rig.url, home)` matching the existing `private_work_admission_tests::Fixture::start` pattern.

Fixture seeding uses `rig.pool` directly (not the proxy) so it is unaffected. `first_pid` is recorded from `proxy_pool` before arming. The transport is armed after seeding. `prepare_private_validator()` (the production entry point) is called — not `prepare_private_validator_with_write_pool`. The recovery PID checkout uses `timeout(&runtime).await` as the configured deadline.

**F6 — Nested `if` flattened for Clippy:**
`if write_result.is_ok() { if let Some(wake) = ... { ... } }` → `if let (true, Some(wake)) = (write_result.is_ok(), ...) { ... }`. Eliminates the collapsible nested `if` that Clippy flags.

---

## Third-round review findings — commit `e4b6adedb`

Four findings from the third post-commit inspection of `6c24e5e33`:

**F1 — `Cell<bool>` and `Send`:** `Cell<T>` is `!Sync`, so `&Cell<bool>` is `!Send`. The `async` closure captured by `bounded_for` must be `Send` (required by `bounded_for`'s bound and the `async_trait` dispatch path). Fix: replace `Cell<bool>` with `std::sync::atomic::AtomicBool` using `Relaxed` ordering. `AtomicBool` is `Send + Sync`. Relaxed ordering is correct here: the closure and its await points form a single Future chain with no cross-thread races; the atomic merely satisfies the `Send` bound without additional synchronisation. Also removed the duplicate `let submission_id` / `let resource_store_settings` declarations introduced by the previous patch, and corrected the comment that incorrectly stated a timeout "always reaches here with `commit_reached = false`" — a deadline can fire while `tx.commit()` is in flight (after the flag was set).

**F2 — Read custody, `pool.get()` without guard:** `read_validator_original` and `read_private_validator_original` used raw `self.pool.get()`. On a `bounded_for` timeout, the dropped `deadpool_postgres::Object` returns its connection to the pool normally — an uncertain read-only connection re-enters the pool. Fix: replace `self.pool.get()` with `crate::bootstrap_evidence::io::acquire(&self.pool)` in both functions. Call `client.recycle_after_commit()` after each confirmed `tx.commit()`. Remove `drop(client)`. With `acquire()` (EvidenceClient via `from_client`, no retirement), a Drop without `recycle_after_commit()` calls `PgClient::take()` which extracts the connection from deadpool (slot freed) and drops the socket — the pool never receives an uncertain connection back.

**F3 — Pool test: detach frees the slot, zero-timeout proves nothing:** `Client::take()` / deadpool detach frees the pool slot immediately. A zero-timeout `.get()` would succeed (not fail) after a detach, so the saturation assertion was wrong. Fix: before arming the transport, get a connection from `fault_pool` and record `first_pid = pg_backend_pid()`. After the write call, get a new connection with a 10-second finite deadline and assert it succeeds (slot was freed). Assert `recovery_pid != first_pid` to prove the original PG session ended and a new one was established. This is a genuine recovery-over-the-same-pool proof.

**F4a — Comment accuracy:** The statement "a timeout always reaches here with `commit_reached = false` unless impossible" was wrong. A `bounded_for` timeout can fire while `tx.commit()` is awaiting the server ACK — after the flag was set. The comment is corrected accordingly.

**F4b — Vacuous `!is_empty()` on `[u8; 32]`:** `request_checksum` is `[u8; 32]`; `!is_empty()` is always `true` for a fixed-size array. Replace with: query `private_request_checksum` from `brassclaw_turns` and assert `state_a.request_checksum == db_checksum.as_slice()`. This proves the readback resolved the actual persisted row.
