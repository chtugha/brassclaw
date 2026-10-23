# Retained validator observation limits — 2026-10-23

Status: **passed scoped acceptance** on common `main`, based on
`172f8c08522f499743f476ded61d87a99590339d` plus this checkpoint.

`RetainedValidatorCoverage::observe` now verifies each result fingerprint with
the corresponding actual `RetainedStepObservation::data_limits()`. It formerly
used the default 262,144-node ceiling even when the executor had accepted the
result under larger acknowledged settings. The checksum algorithm/format is
unchanged; this repairs the verification bounds, not component approval.

The actual Monty regression generates **300,000 diagnostic strings**, collects
them and executes the pinned aggregator. Step one captures 1,048,576 nodes;
later steps capture 600,000 after a confirmed revision change. Before observation,
another acknowledged change reduces the live profile to 2,048 nodes/16 KiB. The
VM identity stays unchanged. Complete diagnostics still verify against the original
per-step limits and preserve the negative aggregate (`pass=false`). Altered data
is rejected. The independently smaller report profile rejects the whole report;
no data is clipped and `q1_qualification` remains false.

The existing fixture also checks incomplete/missing/swapped/wrong-selection
observations, private required-rule coverage and real PostgreSQL sealed
subject/validator-selection retention. Separate actual native workflow and Tool
usage cases retain their per-step bounds and exact negative expectations. These
are constrained execution/consumer observations, not ordinary activated Q1 routes,
semantic review, general standalone pure evidence, human Q2 or component activation.

## Execution

All Cargo work ran serially in background `screen`, under
`/private/tmp/brassclaw-target-cargo.lock`, from `/Volumes/SSDE/brassclaw` with
only `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target`, Rust **1.96.0**, disabled
incremental/dev/test debug output and `RUST_MIN_STACK=16777216`. Pre-Cargo disk
checks and necessary selective cache cleanup preserved actual executables before
removal. No remote, alternate target or mutable compilation mirror was used.

Ordinary application/worker build passed (49.670 s), with version smoke.
Composition/worker test targets rebuilt (114.094 s). Features were
`brassclaw_reborn_composition/skills-db`, `test-support`, `migrate-from-libsql`,
`bedrock` and `brassclaw_llm/testing`; root build/lint additionally enabled
`skills-db,migrate-from-libsql,bedrock`.

**Seven tests passed; zero failed or ignored. All were executed on this batch.**

| Actual selector/group | Passed | Seconds |
| --- | ---: | ---: |
| `validator_component_tests::`, including complete large diagnostics | 5 | 9.633 |
| Existing actual workflow per-step limits/full handoff | 1 | 3.632 |
| Existing actual Tool usage large answer/negative observations | 1 | 5.451 |

Strict root/Engine/composition Clippy with `--tests -- -D warnings` passed
(71.696 s), as did repository format and whitespace checks. All 39 retained
executable hashes were verified; other compiled targets are not claimed as
executed suites. The actual ordinary application/worker were restored to the
canonical target and their hashes rechecked.

| Actual artifact | SHA-256 |
| --- | --- |
| Application | `5077170012a408b39cc0b695b31d72e96e35e630c3395c00063f67b49c56b291` |
| Worker | `861223cad68d9e964fadb3e1efa74c56f55b44949b078b1a710de79ce9db1e39` |
| Composition tests | `af339536c9fed5d5358828b18cd0299129fbee955bf0eda073f1e8dc329703a4` |

Archive: `target/accepted-evidence/validator-observation-limits-20261023/run-1/`.
Exact commands/profiles, complete source hashes and actual artifacts are retained.

- `source.json` SHA-256: `a9c6ff798877b37a67881e43f7ba19a8972d0ce937995509c13e50de93f6e88a`.
- `accepted.json` SHA-256: `7808eb585ca7e1854fd08f38bb69eb68acb0556156f94a61b7c7e32d34125ea1`.

Only this unembedded report, pure-review contract, status index and implementation
journal are finalized afterward; final applicability checks preserve every
compiler/runtime input. Full validator scheduling/evidence writers, generic pure
approval and installed/platform/release acceptance remain with their existing
plan owners. This fix introduces no approval grant, source execution, new
primitive, effect replay or Rust workflow fallback.
