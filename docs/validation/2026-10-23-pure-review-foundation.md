# Pure-review prerequisites and PostgreSQL transport ownership — 2026-10-23

Status: **passed scoped local acceptance**, based on common `main` at
`c52bdc2e0f561160c3b07602c6f9d87ee954f7cc` plus this checkpoint. This accepts
structural prerequisites and the named actual callers. Durable standalone pure
approval, class-wide Q1, human review routes, package qualification, activation
and full-plan completion remain open under
[validator Steps 3/8–10](../../validator_v3.md#step-9--wire-durable-q1-work-human-q2-and-approval-writers).

## Implemented scope

- `InspectedRetainedProgram::pure_component` exposes the exact inspected class-22
  closure, checks all root export signatures/call graphs and rejects legacy bodies
  or any host call, including private/dead/unselected dependency calls. It borrows
  source observations without copying executable bodies or creating an interpreter.
- The untrusted `pure-component-approval/1` declaration strictly parses complete
  exact root/closure references and distinct Q1/Q2/behavior identifiers. Selection
  comparison is not evidence or approval; Skill-association formats are unchanged.
- Existing sealed validator subjects/selections expose original submission
  ID/checksum separately from the validator's own retained manifest. Storage
  formats, activation and execution permission are unchanged.
- Real PostgreSQL ACK-cut transports now belong to optional backend test support.
  Product composition retains domain-specific byte selectors and actual outcome
  assertions. The backend forwards real bytes, supports explicit shutdown, denies
  remote upstreams/query address overrides and never invents SQL/coordinator results.
  Ordinary application builds do not enable that test listener.

See [the contract and exact remaining cutover](../reborn/pure-component-review.md)
for the future writer/reader requirements, including independent approval for
each unbound executable invocation rather than usage-closure UUID membership.

## Failed batches and repairs

The first batch passed the new declaration/source cases, original sealed-subject
checks and ordinary matched/unmatched global-root regression. Its architecture
suite then found two existing PostgreSQL listeners in product-composition test
files. Both complete protocol transports were moved to backend test support;
no architecture rule, file exclusion or binding-call disguise was introduced.

The second batch exposed a defect in the new transport's address check: the URL
library represents IPv4 hosts under the non-special PostgreSQL scheme as domain
text. Four native cases correctly failed before connection rather than skipping
their cut. Explicit literal parsing and a positive IPv4/IPv6 regression repaired
that defect. Remote names and query address overrides remain rejected. Both
failed archives are retained; their failed suites are not counted as passes.

## Actual execution and reuse

All Cargo operations ran serially in background `screen`, from
`/Volumes/SSDE/brassclaw`, under `/private/tmp/brassclaw-target-cargo.lock` with
the single target `/Users/ollama/brassclaw-target` and Rust **1.96.0**.
Incremental/dev/test debug output was disabled and `RUST_MIN_STACK=16777216`.
Pre-Cargo disk checks and selective disposable-cache cleanup preserved actual
executables before removal. No testmachine build or retired target2 was used.

The common application/worker build passed in run 2 (58.323 s), with application
version smoke. Run 3 reused that actual application: its only pre-validation
successor edit was the optional backend test transport, absent from the ordinary
application build. Run 3 rebuilt affected tests and worker (129.188 s) and repeated
the version smoke. The actual worker is retained beside the test executables.

The test build selected `brassclaw_monty_host`, `brassclaw_pg`,
`brassclaw_engine`, `brassclaw_skills`, `brassclaw_reborn_composition` and
`brassclaw_architecture` with `--tests --bins --locked`. Features were
`brassclaw_reborn_composition/skills-db`, `test-support`, `migrate-from-libsql`,
`bedrock` and `brassclaw_llm/testing`; the composition dev-dependency enabled
`brassclaw_pg/test-support`. Root build/lint additionally enabled
`skills-db,migrate-from-libsql,bedrock`. The immutable archive records exact fully
qualified arguments, profiles, complete source hashes and actual artifact paths.

**Run 3: 67 tests passed; zero failed or ignored.**

| Actual selector/group | Passed | Seconds |
| --- | ---: | ---: |
| Pure declaration contract | 5 | 0.378 |
| Existing Skill association contract | 5 | 0.263 |
| Actual validator source/preparation | 5 | 4.686 |
| Complete native subject/selection identity | 1 | 1.976 |
| PostgreSQL endpoint/transport guards | 8 | 0.372 |
| Native reservation, inbound and transcript atomicity/ACK loss | 10 | 19.817 |
| Native child, old catalogue, all three ACK cuts and same global VM | 1 | 36.549 |
| Dependency boundaries | 28 | 1.760 |
| Composition boundaries | 4 | 0.435 |

The separate ordinary matched/unmatched same-root regression passed in run 1
(one test, 69.573 s) and is reused for that unchanged production scope. It is not
added to the 67 newly executed tests. Run 3's child case independently exercises
the actual successor global service and the relocated wire transport.

Strict Clippy for root, PG, Engine, Skills, composition and architecture with
`--tests` and `-- -D warnings` passed (81.217 s). Repository formatting and
`git diff --check` passed. All 55 retained executable hashes were verified;
additional compiled test targets are not claimed as executed suites. The common
application/worker were restored to the canonical target with verified hashes.

| Actual retained artifact | SHA-256 |
| --- | --- |
| `bin-brassclaw` (reused run 2) | `a4c87dbc4a50b3af6a7bb687d2db6114e325fcf6c4033208a852aa44d654c6a3` |
| `bin-monty_worker` | `466f489f39a3c9e26018239bf79d96ef53d1787900f1136eb418c6786aa509af` |
| `test-brassclaw_reborn_composition` | `21ac487db733803c53719af918ec120b1f004c8bef7341a848651efd11b0fc62` |
| `test-brassclaw_pg` | `a2a02dd01e70ab24093bedf188211f234939b74acedc61803c4101fbabca6cf3` |

Archive: `target/accepted-evidence/pure-review-foundation-20261023/run-3/`.
`source.json` SHA-256:
`392f5cb57c827553802895d3142d13b77439aa7a440b3cbe2186ff3e0f844089`.
`accepted.json` SHA-256:
`006953e5005abe49ca41075194d9c932d381dfec9bb0d95d4dbab16ab691cfe2`.
Only this unembedded report, the unembedded status index, pure-review contract
and dated implementation journal are finalized afterward. The final applicability
receipt checks that all compiler/runtime inputs stay unchanged.

This is no full-workspace/platform/release, installed-instance, general crash
recovery or model-quality claim. The existing
[implementation ownership index](../implementation-status.md) retains every
remaining plan gate. Commit/push records this checkpoint without closing them.
