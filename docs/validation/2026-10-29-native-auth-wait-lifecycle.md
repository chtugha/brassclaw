# Native auth waiting: scoped lifecycle and persistence acceptance

29 October 2026. Applied infrastructure prerequisite; **not** ordinary native
authentication continuation, task-family credential capture, K3 or full-plan
completion. No Recipe/Tool activation, Rust-loop fallback, release or deployment.

## Implemented boundary

The worker-only `TurnRunTransitionPort::block_native_auth` preserves an original
live runner/lease, native invocation reference and claim count while the run is
BlockedAuth. It creates no loop checkpoint. A versioned private-field wait record
distinguishes this custody from legacy queue/checkpoint suspension. Unsupported
port implementations reject. The future native producer must independently
verify its actual committed occurrence and host preflight; constructing metadata
or copying IDs/boundary labels grants no dispatch authority.

Auth resume requires the original actor/scope/gate/source/reply references,
BlockedAuthGate precondition and unexpired original claim. It returns Running
directly, with no ordinary worker queue/wake or incremented attempt. Native
resume idempotency retains the full original metadata request across reload;
altered inputs conflict. Malformed native snapshot/replay records reject. The
existing PostgreSQL, filesystem, lifecycle, driver-box and event wrappers forward
the contract. Known snapshot constructors carry the additive optional field.

BlockedAuth heartbeat renews custody without changing its public event cursor,
so a delayed auth prompt does not become stale solely through heartbeat. A
CancelRequested run cannot renew its acknowledgement deadline. Original runner
acknowledgement remains two-phase. Native cancellation lease expiry records
Failed/native_cancel_ack_expired, and native runner failure remains Failed;
neither fabricates a VM cancellation acknowledgement. Stale claims cannot resume
or dispatch, and native custody cannot become legacy checkpoint/requeue state.
Unmarked legacy state retains its existing behavior.

## Actual checks

258 distinct Rust cases, zero ignored:

| Cases | Observation |
| --- | --- |
| 86 | Existing Turns library units. |
| 13 | New native custody, exact resume/replay, malformed original, scope/lease/boundary rejection, lifecycle/no-worker-wake, cancellation and no-fabricated-ack cases. |
| 146 | Existing coordinator contracts, excluding those 13 new cases. |
| 11 | Filesystem contracts, including versioned record-owner reopen/resume and actual local byte-owner rejection preserving the original native snapshot. |
| 1 | Actual native PostgreSQL store reopen, heartbeat, original resume, replay/conflict and bounded cancellation-expiry lifecycle. |
| 1 | Existing real PostgreSQL legacy-budget migration test executed with its optional migration feature enabled. |

The Turns library/coordinator and corrected filesystem targets compiled and ran
in R1/R2. R3 verifies/reuses their exact unchanged images, source deltas and logs
(256 cases), and executes the two PostgreSQL cases from its current linked
Composition test image. No ignored/failed run is counted as success. Formatting
passes. Strict `--lib --tests` lint/type checks pass for Turns, LoopSupport,
Reborn and Composition with skills-db/test-support, **without** the optional
migration feature. Reborn/Composition snapshot-literal consumers compile.

All Cargo commands were serial, direct, in the common checkout and inherited
canonical target under `/private/tmp/brassclaw-target-cargo.lock`, using Screen.
No private build tree, target override, cache clean, remote compilation or warning
suppression. The two failed histories remain preserved:

- R1's new filesystem fixture incorrectly assumed LocalFilesystem supports
  versioned replacement. Its existing rejection was correct. The repaired
  positive case uses the existing record owner; a separate real local case
  verifies rejection and unchanged original snapshot/custody.
- R2's minimal skills-db build exposed an existing migration-only test calling
  a feature-disabled module. That single test now has the same
  migrate-from-libsql condition as its module and neighbouring migration tests.
  It actually passes enabled; the feature-disabled configuration also passes
  strict checks. No `ignore`, blanket cfg or warning allowance was introduced.

## Exact evidence

Terminal scoped receipt:
`/private/tmp/brassclaw-native-auth-wait-validation-20261029/lower-3/partial.json`
SHA256 `4078ac6cd16c0c2762d456b84bc4aff63800d02ec34b7c659d6df9128857c2fc`.

Its source inventory has 2,709 files across 58 local dependency packages,
SHA256 `46f7b6fd386b5de5050c97541bc66b9d85580caa60eaa4315eb648eb2e93cee5`.
The actual Composition test image SHA256 is
`84c101944d3c2f90653b3d270633da4cbe99ac487f3728660fc52e94e9882451`.
Its actual features include postgres/root-llm-provider/skills-db/test-support and
migrate-from-libsql. Enabled migration execution and disabled migration strict
commands/logs are both retained in that receipt.

The applied package and test repairs are recorded under
`/private/tmp/brassclaw-native-auth-wait-20261029-_l6u9ydb`:
original16-path applied manifest, original failed fixtures, current-applied.json
and repair history. Do not replay the historical proposed source. The current
17-path source is represented by Root81
`current-root-inputs-native-wait-r3.json`, SHA256
`de9f1d8ca06e6c104545cf6011db3d63f4dc1d80649088e41896388399dc2950`.
Sources were rehashed unchanged after completion. Screen exited0, own
runners1951/1957/2313 are actually gone and the common lock independently free.

## Remaining integration

Wire the actual native producer to wait after its original journal retention
and before kernel dispatch/resource reservation/material staging, then invoke
that original retained runtime once. Keep normal effect freshness requiring
Running; a separate wait check must prove original native custody without
granting dispatch while BlockedAuth. Revalidate live Tool policy after the wait.

Capture exact original task-family account/secret generations and explicit
absence; bind them explicitly to retained runtime/services. Only verified own-flow
or conditional refresh lineage may advance that family. Verified children retain
the original source with their own claim fences. Do not lose bindings through
task-local context or fall back to today's global selection. Native historical
gate resolution, real auth-flow/UI/callback/global-Monty/final-use cases remain
open. These neutral store tests do not establish those behaviours or platform/
deployment support. See the source-checked private follow-up notes and the
existing Simplified/Composition owners.
