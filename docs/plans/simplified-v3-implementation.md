# Simplified v3 implementation record

This record distinguishes implemented changes from the binding target in
`simplified_v3.md`. It is not a declaration that the migration is complete.

## Phase 0 evidence

Run `python3 scripts/simplified_v3_inventory.py --output /tmp/simplified-v3-inventory.json`.
The JSON contains source locations and symbols, route and placeholder candidates,
settings-source candidates, test/CI files, and every Cargo feature declaration.
It does not read databases, secret files, or running-instance configuration.
Reference counts change as implementation proceeds; regenerate before review.
Consumer classification is explicitly unresolved until a semantic review verifies
all applicable uses. Filename or identifier matches alone never authorize deletion.

Initial scan: 21,478 reference lines, 739 route candidates, 314 placeholder
candidates, 396 test/CI files and 65 manifests. Candidates include comments and
tests; these counts are not counts of public runtime routes or actual defects.

## Binding security decisions and migration dispositions

The single-operator target is already authorized by the plan. WebUI login proves
operator access to this instance. It does not prove that a remote channel sender,
model output, recipe or extension has authority to execute host effects.

| Existing dimension | Target disposition | Required evidence before removal |
| --- | --- | --- |
| WebUI account, role and deployment mode | One owner authenticated by instance token | Invalid/absent token, session revocation, rotation, CSRF/Origin and external-channel negative tests |
| Tenant/User/Project authorization | Instance-level capability policy | Enumerate every comparison and SQL filter; distinguish authorization from data ownership/FKs |
| Project/Thread/Run/Message IDs | Retain relational identity | Stable transcript order, exact message input, reply routing, parent/child references, replay/dedup tests |
| RuntimeProfile selectors | One fixed security policy | Preserve actual sandbox, process, filesystem, network and secret gates; reject obsolete config explicitly |
| Capability grants and approvals | Instance policy plus concrete invocation approval | Fingerprint, expiry, revocation and stale-attempt checks before effects; no global approval reuse |
| Secret scope/AAD/path | Instance ID plus stable secret handle | Decrypt with original AAD before any key rewrite, re-encrypt and verify; ambiguity aborts migration |
| Actor/principal/channel identity | Operator identity or external provenance as appropriate | External OAuth identities and remote senders must never acquire operator rights implicitly |
| Monty conversation registry | One runtime-owned global service | Boot readiness, bounded admission, addressed cancellation, durable continuations and one-process ownership |

Token theft grants operator access, so token/session redaction, constant-time
verification, rotation and revocation remain mandatory. Browser XSS and CSRF must
not turn a logged-in browser into an unguarded host-effect path. Remote listening
remains explicit with loopback defaults; Origin/TLS/reverse-proxy requirements
survive removal of account management.

Prompt injection, untrusted Python/extension code and malicious MCP results stay
below the capability/kernel boundary. SSRF and subprocess environments remain
controlled by brokered network and secret access, sandboxing and resource limits.
Backups must preserve old keys until encrypted data and relational migrations
have been verified. Unknown external-effect outcomes after a crash require
reconciliation, not automatic replay.

## Implemented prerequisites

- Runtime component initialization moved from WebUI construction into
  `component_boot.rs`, called by `build_reborn_runtime` before worker/trigger
  startup. Migration proof is still `BootedDb`. Required seeding and queue
  recovery failures now abort boot; integrity failures remain fatal. All prompt
  bodies load before any process-wide prompt initialization. Each loaded body
  and its checksum are verified together against the compiled seed, so an edit
  after the earlier integrity scan cannot install an unverified prompt.
- The Monty driver resolves the exact `msg:<message_id>` accepted reference via
  `SessionThreadService::submitted_user_message`. The durable message must be
  User/Submitted, belong to the expected thread/turn/run, and have content.
  Missing/invalid references, lookup errors and linkage mismatches fail before
  effect-executor construction or VM driving. There is no latest-message or
  empty-string fallback.
- A test against the pinned Monty v0.0.16 proves that one VM can suspend A,
  complete B and then resume A without replaying either external wait.

## Remaining dependencies before production cutover

No phase has been declared complete. The global service is not yet connected to
production and the driver still owns per-conversation sessions. The owner/auth,
scope/policy, edition, settings/UI and persisted-data migrations remain open.
The inventory still requires per-consumer classification, actual route mappings
and a settings catalogue with defaults/ranges/restart semantics.

Monty's upstream `FunctionCall::resume_pending` and incremental
`ResolveFutures::resume` support async interleaving. `host.run_program` currently
fully awaits `execute_code`, so it needs a retained nested continuation keyed to
task/attempt. Durable blocked-run transitions must release the worker claim;
keeping a parent waiting on a worker-owned oneshot is insufficient.

The root `LimitedTracker` currently counts lifetime allocations and wall time.
Its `FunctionCall` offers `tracker_mut`, while `ResolveFutures` does not. A global
runtime needs bounded live-heap accounting and separately enforced task budgets,
excluding idle/approval time. The continuation test does not prove these limits.
Do not remove resource limits or reset global live-memory accounting to conceal
the mismatch.

The existing PostgreSQL integration rig requires Docker/testcontainers. This
environment has neither a Docker CLI nor `psql`; no live PostgreSQL boot,
migration, restore or cutover test has been verified. Skipped tests cannot satisfy
the plan's acceptance criteria. Existing secret and instance data have not been
rewritten, migrated or deleted.

## Verification

- `cargo test -p brassclaw_reborn_config --all-targets`: 63 passed.
- `cargo test -p brassclaw_threads --all-targets`: all passed, including the
  eight new exact-input contract tests.
- `cargo test -p brassclaw_engine --test global_monty_continuation`: one passed
  against the pinned upstream VM; this is an API prerequisite test only.
- `cargo test -p brassclaw_architecture --all-targets`: 29 passed.
- `cargo check -p brassclaw_reborn_composition --features skills-db`: passed.
- Strict `cargo clippy` for composition, threads and engine with `--all-targets`,
  `skills-db` and `-D warnings`: passed. Two pre-existing expression-style
  warnings and one unused test-only helper were cleaned up during validation.
- Changed Rust files pass `rustfmt --check`; `git diff --check` passes.

Commands use the NVMe target directory required by the repository. These are
targeted checks, not the full workspace, PostgreSQL or browser acceptance suite.

An additional `--no-default-features` composition check failed in existing
Postgres feature boundaries (`system_seed.rs` imports and the factory's
Postgres-only fields/types). The production configuration mandates Postgres and
passes the checks above. The feature cleanup in Phase 4 must explicitly resolve
or retire this unsupported configuration; it has not been counted as passing.
