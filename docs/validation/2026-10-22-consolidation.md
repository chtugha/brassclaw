# Combined source validation — 2026-10-22

Status: **passed scoped combined-tree acceptance**, completed 2026-10-23
(Europe/Berlin). This records consolidation, not completion of the v3 plans.

## Common source and retired builds

The authoritative source/index is the repository root on `main`, based on
`580a7f73e`. Both development workstreams are retained. The latest prefix-owned
overlay (`prefix_compiler_components.rs`) exactly matched the common tree.
The two mutable build mirrors matched their retained inventories before
retirement (9,338 runtime inputs; 3,862 prefix inputs). Differences were inspected
as historical baselines, committed successors or newer common-tree work; no old
projection was copied over the root. Both historical local branches were already
ancestors of `main`; no cherry-pick or force push was needed.

The mutable `catalogue-policy-workspace` and `prefix-target2-build-source`, old
repository Cargo cache and `/Users/ollama/brassclaw-target2` were retired after
actual source/worker/executable hash checks. Immutable historical source/binary/
receipt archives remain evidence, not alternate development/build trees. A scan
found one active host target, `/Users/ollama/brassclaw-target`; repository `target`
contains evidence without a Cargo cache. Retirement/audit receipts remain under
`target/accepted-evidence/consolidation-20261022/` and the named temporary audit
records on this development host.

## Failures repaired before acceptance

The initial common build encountered generated asset paths from the retired
mirror. Discarding the stale Cargo cache and compiling the actual root resolved
that failure. Failed build receipts are retained rather than counted as passes.

The new complete-parent fixture first mixed a legacy pure body with preloaded
exports. It now declares a pure completion function. Subsequent real execution
exposed inconsistent service/runtime tenant identity, then agent/project/thread
owner identity, in that fixture. The private parent now uses a coherent real
claim/profile/scope and the existing trusted internal admission/result-reference
contract. Production settlement validation was not relaxed. Its child runs in
the instance VM, retains the captured old catalogue after coherent publication,
returns the original full reply and permits a later ordinary approved chat on
the same VM. This is a closed review, not parent Recipe activation.

The tenant mismatch also exposed an actual PostgreSQL thread-store gap: ensure
could write a scope from another service namespace before a later reader rejected
it. Ensure and snapshot writes now reject that identity before persistence. The
real PostgreSQL regression verifies that the rejected thread creates no row.

Pregenerated Oniguruma bindings now agree with the generator's exclusion of
function-pointer equality. Native-model and HTTP tests share one stateless
loopback HTTP module instead of loading the same source twice. These repair the
actual compiler/Clippy diagnostics; no warning or rule was disabled. Temporary
investigation diagnostics were removed.

## Exact execution scope

All checks ran serially from `/Volumes/SSDE/brassclaw` in background `screen`,
under `/private/tmp/brassclaw-target-cargo.lock`, using Rust **1.96.0** and the
single NVMe target. Incremental compilation and dev/test debug information were
disabled. Disk checks/necessary cache cleanup preceded Cargo; actual executables
were copied and hash-verified before Cargo exited or cleanup could remove them.
The real worker is retained beside the test executables as `monty_worker`.

The root `brassclaw` application and worker build passed
(46.201 s). Affected composition,
Threads, Turns and host-runtime test targets compiled
(114.520 s). Common features were
`brassclaw_reborn_composition/skills-db`, `test-support`, `migrate-from-libsql`,
`bedrock` and `brassclaw_llm/testing`; root build/lint also enabled
`skills-db,migrate-from-libsql,bedrock`. Exact fully qualified argument vectors,
profiles, toolchain, source-file inventory and artifacts are in the immutable
`run-10/source.json`, per-case logs and `run-10/accepted.json` archive.

**503 tests passed; zero failed or ignored.**

| Executed selector/group | Passed | Seconds |
| --- | ---: | ---: |
| `complete-parent` | 1 | 35.824 |
| `native-child-failure-ack` | 1 | 35.901 |
| `atomic-pg-transcript-admission-upgrade` | 15 | 27.854 |
| `native-answer-no-replay` | 1 | 31.134 |
| `ordinary-global-match-no-match` | 1 | 67.604 |
| `prefix-http` | 3 | 3.034 |
| `prefix-source-storage` | 1 | 1.148 |
| `provider-model-input-limits` | 1 | 2.015 |
| `prefix-budget-monty` | 1 | 0.047 |
| `threads-units` | 40 | 0.379 |
| `memory-transcript` | 43 | 0.386 |
| `filesystem-transcript` | 15 | 0.371 |
| `turns-units` | 86 | 0.267 |
| `memory-turn-coordinator` | 146 | 1.099 |
| `filesystem-turn-coordinator` | 8 | 0.484 |
| `native-builtins` | 140 | 18.662 |

Affected/root Clippy with `--tests`, those features and `-- -D warnings` passed
(71.504 s). Repository formatting and
`git diff --check` passed. All 69 retained executable hashes
were verified. Compiled additional targets are not claimed as executed tests;
the complete composition/workspace suite and platform matrix were not run.

| Actual retained artifact | SHA-256 |
| --- | --- |
| `bin-brassclaw` | `95c7f31eda8113ceb9eb58d75fa8533a0f4e54f0cce63a2ea734f180a45b81c2` |
| `bin-monty_worker` | `b1fefa0f6e0742611100cca391a3a63fcaa1d6ec61663400879ed11ae7185fa9` |
| `test-brassclaw_reborn_composition` | `e44f2310a97c206ae98e87e21388ef88ea3d898be7887faa2059b05af2d03dc9` |

The immutable source inventory SHA-256 is
`0367df61de76646fc0d49a0742cd2a4347bb95c35be8d3a5d68b79fa797c3e1e`;
the acceptance receipt SHA-256 is
`7dc71967d576b22f4329b224b00bb0d8f75ab0633333a68a55d35c9272e07ddb`.
Only this unembedded validation record and the unembedded dated implementation
journal are updated after acceptance; runtime/compiler inputs stay unchanged.

## Remaining boundaries

This accepts the named real PostgreSQL/global/parent-child/native-result custody,
selected prefix/provider/budget and shared backend callers on the combined tree.
It does not qualify or activate installation-owned public parent workflows or
complete the five legacy subagent migrations, full validator/library/pure approval,
final prefix serving/request guards, general crash/reconciliation recovery,
remaining authority/cap/compatibility cutover, platform/release matrix, live-model
quality or fresh installed-instance acceptance. Those retain their exact owners
in [the implementation/status index](../implementation-status.md). A commit/push
records this checkpoint; it is not a full-plan or deployment certificate.
