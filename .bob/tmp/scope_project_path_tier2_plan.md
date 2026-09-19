# Scope / Project / Path Security Simplification — Tier 2 Plan

> **Status**: Reviewed against the current codebase; corrected plan ready for implementation
> **Priority**: Correctness and authority-preserving concurrency before simplification
> **Scope**: Capability-lease persistence and per-user mount alias verification

## Review outcome

The earlier plan was not safe to implement as written. In particular:

- Removing the authorization-layer fallback while only stripping indexed fields in `LocalFilesystem::put` would break every local `revoke`, `claim`, and `consume`, because `LocalFilesystem` also rejects `CasExpectation::Version(_)`.
- Removing mutation locks would make the existing local fallback race-prone because that fallback downgrades versioned writes to `CasExpectation::Any`.
- Read-time lease-index compaction could overwrite a concurrently issued lease and could permanently discard paths after a transient read failure.
- The proposed PostgreSQL JSON patch used lowercase status names inside the serialized JSON, but serde expects enum variant names such as `"Claimed"` and `"Consumed"`.
- The proposed PostgreSQL update omitted `user_id` from its write predicate and did not preserve the full `consume` contract.
- The proposed alias test did not compile because `ResourceScope::local_default` requires arguments, duplicated existing mount-view coverage, and could not prove that all consumer stores were represented.

The corrected work is below. Rejected changes are recorded explicitly so they are not reintroduced during implementation.

## Ground-truth invariants

1. Capability leases are authority-bearing records. `claim` and `consume` must be atomic across all supported deployment shapes.
2. A backend that cannot honor `CasExpectation::Version` must not be described as providing cross-process CAS safety.
3. PostgreSQL lease operations must scope reads and writes by `id`, `tenant_id`, and `user_id`.
4. The SQL `status` column uses lowercase values, while the JSONB `grant.status` field uses serde's Rust variant names (`Active`, `Claimed`, `Consumed`, `Revoked`). Both representations must remain synchronized.
5. `consume` must run `ensure_consumable`, decrement `max_invocations`, and preserve the fingerprinted/claimed transition behavior implemented by the in-memory and filesystem stores.
6. Lease-index maintenance must not turn transient read failures or concurrent writes into permanent loss of discoverability.
7. Production persistence is PostgreSQL. Filesystem-backed stores still require correct contracts because they are used by tests and filesystem composition paths.

## 2A — Fix PostgreSQL lease mutation semantics and atomicity

### Files

- `./crates/brassclaw_authorization/src/pg_store.rs`
- `./crates/brassclaw_authorization/src/lib.rs` only if mutation logic is extracted for backend parity
- `./crates/brassclaw_authorization/tests/` for PostgreSQL integration coverage

### Problems to fix together

`PgCapabilityLeaseStore` currently has several coupled correctness defects:

1. `transition_status` reads and writes on separate pooled connections, so concurrent claimers can both pass the precondition.
2. Its `UPDATE` predicate omits `user_id`, even though the read is user-scoped.
3. `consume` sets every lease directly to `Consumed`; it does not decrement multi-use grants and does not call `ensure_consumable`.
4. The current `claim` performs one read for `ensure_claimable` and a second read in `transition_status`, widening the race window.
5. `issue` uses `ON CONFLICT (id) DO NOTHING` but returns the submitted lease even when no row was inserted.

### Implementation

Replace the generic read-then-update flow with one transaction on one client:

1. Acquire a mutable pooled client and begin a PostgreSQL transaction.
2. Select the row with `FOR UPDATE` using all authority boundaries:

```sql
SELECT "grant"
  FROM brassclaw_capability_leases
 WHERE id = $1
   AND tenant_id = $2
   AND user_id = $3
 FOR UPDATE
```

3. Deserialize the lease and verify that the embedded `ResourceScope` has the same owner as the requested scope. Return `UnknownLease` for rows outside the caller's scope.
4. Apply the operation-specific Rust mutation while the row lock is held:
   - `revoke`: preserve the current idempotent behavior and set `Revoked`.
   - `claim`: call `ensure_claimable` with the supplied fingerprint, then set `Claimed`.
   - `consume`: call `ensure_consumable`, decrement `max_invocations`, and apply exactly the same status transitions as the in-memory and filesystem stores.
5. Serialize the complete updated lease and update both representations in one statement, again including `user_id`:

```sql
UPDATE brassclaw_capability_leases
   SET status = $1,
       "grant" = $2,
       updated_at = now()
 WHERE id = $3
   AND tenant_id = $4
   AND user_id = $5
```

6. Require exactly one affected row, then commit. A zero-row update after the locked read is a persistence/invariant error, not success.

Prefer extracting small shared mutation helpers for `claim` and `consume` if doing so removes the existing backend drift without changing public API. Do not duplicate the multi-use transition logic a third time.

Do not use the earlier `jsonb_set` proposal. Re-serializing the locked lease is less error-prone and keeps the lowercase SQL status separate from the serde JSON enum representation.

For `issue`, replace `ON CONFLICT DO NOTHING` success semantics with one of these explicit outcomes:

- a plain `INSERT` that maps a duplicate key to a persistence error; or
- retain `ON CONFLICT DO NOTHING`, inspect the affected-row count, and return an error when it is zero.

A newly generated UUID collision is unlikely, but returning an unpersisted authority record is an invalid contract.

### Required tests

Add real PostgreSQL integration tests that drive `CapabilityLeaseStore`, not only private helpers:

- two concurrent claims of the same active fingerprinted lease: exactly one succeeds;
- two concurrent consumes of a one-shot lease: exactly one succeeds;
- a two-invocation lease remains `Active` with one invocation after the first consume and becomes `Consumed` with zero after the second;
- consuming a revoked, consumed, expired, exhausted, or unclaimed fingerprinted lease returns the same typed errors as the in-memory store;
- a scope with a different user cannot claim, consume, revoke, or read the lease;
- duplicate `issue` does not report success;
- the SQL status column and serialized JSON status remain synchronized after every transition.

## 2B — Remove unsafe CAS degradation before simplifying filesystem lease writes

### Files

- `./crates/brassclaw_authorization/src/lib.rs`
- `./crates/brassclaw_filesystem/src/local.rs`
- relevant filesystem and authorization tests

### Current behavior

`write_lease_raw` retries every `FilesystemError::Unsupported` with a projection-free entry and `CasExpectation::Any`. This conflates two independent unsupported capabilities:

- indexed projections;
- versioned CAS.

The retry keeps local byte-only storage working, but a versioned mutation silently loses cross-process compare-and-swap protection. The per-owner lock only serializes callers sharing the same store instance; it does not protect another process or another `FilesystemCapabilityLeaseStore` instance over the same root.

### Correct approach

Do not globally teach `LocalFilesystem::put` to silently discard `Entry.indexed`. The root filesystem contract deliberately rejects record-shaped entries on byte-only mounts, and changing it would silently weaken every caller, not just authorization.

First make the capability-store contract explicit and safe:

1. Separate projection fallback from CAS fallback. An `Unsupported` result must not automatically downgrade a `Version` expectation to `Any`.
2. Preserve indexed entries on capable backends.
3. For byte-only backends, construct the opaque entry explicitly in the capability store or in an explicitly named byte-only adapter; do not silently mutate the generic filesystem contract.
4. For mutation operations, either:
   - provide genuine atomic versioned CAS for the local backend, including cross-process behavior and monotonically changing versions; or
   - fail closed when the configured backend cannot provide the CAS floor required by `RootFilesystem` for authority-bearing stores.
5. Audit composition wiring before choosing the second option so supported local startup is not broken. If a filesystem production path is still supported, it must be wired to a CAS-capable backend or rejected at construction/startup with a clear error.

Do not remove the current fallback until the replacement passes backend-contract tests. The final state must never silently turn a versioned authority mutation into an unconditional write.

### Required tests

- a stale versioned lease write fails with `VersionMismatch` on every backend accepted by `FilesystemCapabilityLeaseStore`;
- two store instances over the same backing root cannot both consume or claim the same one-shot lease;
- projection fallback, if retained in an explicit adapter, does not alter the CAS expectation;
- unsupported capability-store backends fail at construction/startup or on the first authority mutation without persisting a partial transition;
- indexed projections remain queryable on `InMemoryBackend` and PostgreSQL-backed filesystems.

## 2C — Retain mutation locking and bound idle lock entries safely

### File

- `./crates/brassclaw_authorization/src/lib.rs`

### Correction

Do not remove the lock from `revoke`, `claim`, or `consume` while any supported backend can downgrade versioned writes. The previous 2E contradicted 2A and would create double-claim/double-consume races in local execution.

The map-growth concern is valid: the current `HashMap<CapabilityLeaseOwnerKey, Arc<Mutex<()>>>` retains one strong reference per owner forever. Fix retention without weakening serialization:

- store `Weak<tokio::sync::Mutex<()>>` values instead of permanent `Arc` values;
- while holding the map mutex, remove dead entries, upgrade the current owner's weak entry when possible, or insert a downgraded reference to a new lock;
- return a strong `Arc` to the operation and hold its guard for the complete read/validate/write sequence.

This bounds idle entries while preserving the current lock coverage. It does not claim cross-process safety; that remains the CAS responsibility from 2B.

### Required tests

- concurrent operations for the same owner share one live lock;
- different owners do not block each other;
- dead owner entries are pruned after operations release their strong references;
- claim/consume race tests continue to pass.

## 2D — Keep the scan fallback until index durability is proven

### File

- `./crates/brassclaw_authorization/src/lib.rs`

### Correction

Do not delete `scan_lease_paths` merely because normal `issue` writes the index first. The owner index is a derived discovery structure and is not transactionally committed with the lease file. The current code also uses unconditional index writes on byte-only storage, and locks are neither cross-process nor shared across separate store instances.

The scan remains a recovery path when the index is absent. It is not on the normal indexed hot path, as existing tests already verify by asserting zero `list_dir` calls after indexed issuance.

Before removal, prove all of the following:

1. index and lease creation are atomic on every supported backend, or the index can be rebuilt deterministically;
2. no supported upgrade can contain pre-index leases;
3. concurrent issuers cannot lose index entries;
4. an absent or corrupt index has a documented recovery path;
5. migration/recovery tests cover persisted data from an older store version.

If scan cost needs bounding, add explicit directory-entry limits and return a typed persistence error when exceeded. Do not silently return an incomplete authority set.

## 2E — Reject read-time lease-index compaction

### File

- `./crates/brassclaw_authorization/src/lib.rs`

### Correction

Do not implement the earlier `leases_for_scope` compaction proposal.

It is incorrect for three reasons:

1. consumed and revoked lease files are not deleted, so the proposed logic does not compact the monotonic index described by the plan;
2. any transient read or deserialization error would be treated as a stale path and permanently removed from the index;
3. a concurrent `issue` can append a path after the read, then the compaction write can overwrite the index with an older snapshot and erase that new path.

If retention is required, design it as an explicit lifecycle operation:

- define which terminal leases may be deleted and after what retention period;
- update/delete the lease and index atomically where transactions exist;
- otherwise use versioned CAS with retry for the index;
- never compact based on arbitrary read failures;
- retain the scan/rebuild path until migration and crash consistency are covered.

This lifecycle work is separate from the security simplification and should not be bundled without a retention requirement.

## 2F — Add accurate mount-alias regression coverage

### File

- `./crates/brassclaw_reborn_composition/src/lib.rs`

### Current coverage

The existing `mount_view_tests` module already iterates over `PER_USER_ALIASES` and verifies tenant/user rewriting. A second test that only compares `MountView.mounts` with a duplicated literal list does not prove that all consumer stores are represented; it only freezes the current mount output.

The earlier snippet also called `ResourceScope::local_default()` with no arguments, but the function requires `UserId` and `InvocationId` and returns `Result`.

### Implementation

1. Audit filesystem-backed stores wired by composition and produce the canonical expected per-user alias set in the test.
2. Add a focused test in the existing test module that compares `PER_USER_ALIASES` directly with that canonical expected set. Reuse `sample_scope()` for mount-view behavior; do not change helper visibility and do not add public API.
3. Keep the existing rewrite test as the caller-level assertion that each listed alias resolves under `/tenants/<tenant>/users/<user>`.
4. Add a full mount-set test only if it separately verifies the tenant-shared and three read-only system grants and their permissions.
5. Correct `Vec::with_capacity(PER_USER_ALIASES.len() + 2)` to `PER_USER_ALIASES.len() + 4`, because the function appends one tenant-shared grant and three system grants.

The test is a deliberate maintenance tripwire, not automatic discovery of consumers. Any new filesystem-backed consumer must update both composition wiring and the canonical alias assertion in the same change.

## Implementation order

1. **2A — PostgreSQL atomicity and semantic parity**: production authority path; highest correctness and security impact.
2. **2F — alias audit and regression test**: additive and low risk; correct the capacity at the same time.
3. **2B — filesystem CAS contract**: decide and implement genuine CAS or fail-closed capability checks before removing fallback behavior.
4. **2C — weak-reference lock registry**: retain all lock coverage while bounding idle memory.
5. **2D — index durability analysis**: keep scan recovery unless the required proofs and migration tests exist.
6. **2E — no implementation** unless a separate retention requirement is approved with atomic index maintenance.

## Verification

Before every cargo build, test, check, or clippy invocation:

```bash
df -h /Users/ollama/brassclaw-target
```

If available space is below 15 GB or capacity is above 90%, clean first:

```bash
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clean
```

Then run:

```bash
cargo fmt --check
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test -p brassclaw_authorization
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clippy -p brassclaw_authorization --all-targets -- -D warnings
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test -p brassclaw_reborn_composition --all-features
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clippy -p brassclaw_reborn_composition --all-targets --all-features -- -D warnings
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test -p brassclaw_filesystem
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clippy -p brassclaw_filesystem --all-targets -- -D warnings
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test -p brassclaw_authorization --features integration
```

Use the repository's configured PostgreSQL integration-test setup. Do not treat skipped PostgreSQL tests as verification of 2A.

## Explicit non-goals

- Do not remove `ResourceScope`, `ScopedPath`, or `MountView`.
- Do not weaken tenant/user path rewriting, mount permissions, or indexed tenant projections on capable backends.
- Do not add public mount introspection solely for tests.
- Do not suppress deprecations, warnings, persistence errors, CAS failures, or integration-test failures.
- Do not claim filesystem cross-process safety unless a test with independent store instances/processes proves it.
