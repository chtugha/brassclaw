# BUILD-019 — shared subject reader and Skill interface checks

The [machine-readable receipt](2026-10-31-joint-validator-checks.json) records
the exact source, executable, runtime, model inputs and results. Receipt SHA-256:
`30f6f809a9ef9ce512dafebd332585a0ca3c79227e858775ec90843d6c15a0ee`.

## Implemented boundary

One private preloaded reader handles complete single-root `/1` and Skill/code
function `/2` subjects without executing candidate source or making serialized
graph copies. Source extraction, contracts, definitions and graph checks reuse
this reader. The pure-call rule keeps its separate pure-only scope. Comparison
bases remain data and unchanged partner context receives no new approval.

The new Skill-interface rule checks prose presence, exact association agreement,
recursive input/result declarations and matching selected code export signatures.
It reuses the existing preload export-shape predicate. Extra implementation
exports and compatible schema differences are not rejected merely for differing
from the usage declaration.

## Actual validation

- R1 compiled the current Composition test image in 241.4 seconds and passed
  six native cases: three new Skill/reader cases, two existing preload cases and
  the existing contract flow. Its next test selector had an incorrect namespace;
  the runner stopped before that test. The original failure remains recorded.
- R2 verified identical captured sources, executable, worker, Python runtime and
  model files, reused that compilation and those six successful cases, and used
  the actual compiled test listing to select the remaining four cases.
- Source inventory, structure, pure structure and the actual global/base/Sempai
  consumer passed: **10 distinct native cases, zero ignored** across both runs.
- Connected strict Clippy for Composition and Skills passed with `-D warnings`
  in 102.9 seconds. Format, whitespace and Python dependency checks passed.
- Original runner 6588, native child 7547 and Clippy 8315 exited. The finalizer
  independently acquired the common lock and verified the immutable evidence.

The receipt covers 19 changed paths, 8,183 named inputs and 301 reviewed Root
paths. The actual image is `158ca5ec…`; the unchanged companion worker is
`227a4b2c…`. R2 copied three attributed R1 logs only for its warning scan; those
copies are not fresh executions. No warnings were suppressed.

## Remaining requirements

These are declaration and structural observations, not complete joint Q1,
behavioral/semantic approval or human Q2. Source/call/binding completeness,
schema variance, actual admitted run/attempt/interpreter/settlement custody,
trusted verdicts, atomic Q2 and compatible publication remain required.
The existing single-class preparation refusal for function subjects remains.
General Pure/S qualification, PrefixFree/design activation, real inference,
platform deployment and full-plan acceptance are not established by this batch.

## Subsequent checkpoint formatting

The staged-file whitespace check additionally found one blank line at EOF in
the newly added `validator_review_subject.py`. The app-update checkpoint removes
exactly that final newline: SHA-256 `1902a11d39b01f44335c74e742cc550b9f95e03dff805506ddece9c79b4e2249`
becomes `98a9add85fc9d39f3d955ddd5098a569bd9fda37336f7d8cda21f878beae78e1`.
No Python token or behavior changes. The receipt, source map and executable above
remain evidence for the original bytes; they have not been rewritten to claim
a fresh build. Include this literal embedded-input delta in the next technical
batch before claiming exact packaged uptake of the checkpoint bytes.
