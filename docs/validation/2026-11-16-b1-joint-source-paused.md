# B1 joint-source implementation — explicit user pause, 2026-11-16

Implementation remains paused. The user subsequently authorized only updating
this changelog and committing/pushing the current unvalidated checkpoint to main,
explicitly without further checks or validation. That publication does not
release the pause for implementation, builds or tests. Solo implementation
remains the requested workflow.

## Baseline before this unvalidated checkpoint

Local and last verified GitHub main: `d987f081169bb500740c6d90165797a4826d9efb`.
This contains the CI cfg(test) context correction, Encoding deadline regression
fix, receipts and solo/parallel-independent-build policy. Its strict Code Style
workflow passed. Last observed Tests workflow 43863120825 had only Composition
still running and no failed jobs. Do not claim final CI completion without checking.

## Uncommitted source

B1-02 joint Skill/code source review is connected in source but NOT accepted.
The detailed owning subplan is
`docs/plans/subplan_step_b1_02_joint_source_of_bob1.md` (git-ignored: force-add only
this exact reviewed plan at eventual publication).

Implemented source: shared coverage parser `/2`; four canonical function-source
Recipes; shared collector, existing Pure aggregate and new receipt; distinct
installation joint subjects and selection profile; V217 outer selection `/3`;
independent extra checker fixtures and whole-workflow fixtures through the
existing contained executor; original Work/Ready/source admission; strict
standalone Pure positive-original gate. Internal checked owner names are now
`QualifiedSourceReview`, `CheckedSourceWorkflow` and field `source_review`.
No authored approval writer or full-Q1/human-Q2 gate was opened.

Native build r1, job `adac70f3-63b1-4e30-be81-1240cb275a86`, failed with E0716 at
validator_function_source.rs:123/174. The unannotated shared closure retained an
inferred borrowed lifetime. Fixed with explicit `part: &str`; analogous new
fixture-role closure also typed. Failure and confirmed container/Screen exit are
retained in its job `failure-reconciliation.json`.

Later source changes in native r2 also remove duplicate prerequisite proof reads,
prune irrelevant draft retention, reuse exact selected helpers across primary
classes and correct the obsolete joint-support description. These changes have
only parser/format/logic review, no successful Cargo acceptance yet.

New tests: actual retained typed-edge assembly for all four primary classes in
validator_component_tests.rs; actual existing-worker joint source matrix in
runtime/tests/qualified_source_review_tests.rs. Run historical Pure/original
cases and the Skills coverage parser tests as appropriate. Audit malformed and
swapped function proof/profile negatives; extend independent interface-boundary
fixtures where required before claiming checker qualification complete.

## Builds stopped at the user's request

- Native r2: job `d33b466d-f6cd-4cdd-84e6-60b4146d085f`, App
  `ap-btBWpOfsFWP4ub15BjUYAK`, Screen `b1-joint-source-build-20261116-r2`, state
  directory `/var/folders/x5/7_xq13_x5y5cdj1b_plpjt9c0000gn/T/screen-build-b1-joint-source-build-20261116-r2-_aji_d0e`.
- Strict r1: job `af36d99f-aae3-4572-acae-293bb137b2bd`, App
  `ap-yOu4BgpIHRSgam0R8JH45y`, Screen `b1-joint-source-clippy-20261116-r1`, state
  directory `/var/folders/x5/7_xq13_x5y5cdj1b_plpjt9c0000gn/T/screen-build-b1-joint-source-clippy-20261116-r1-ihmrcaz4`.

Both Apps explicitly stopped. Each container listing returned empty; Screen
socket directory `/private/tmp/brassclaw-screen-sockets-01a10ef5` has no sessions.
Both helper finals exited 1, accepted by IPC and received in the chat. These
are operator-cancelled checks, not successful builds or test results. Their
registries retain the cancellation reason and prohibit automatic retry. No
PostgreSQL tests were launched by these compilation-only jobs.

On release, first inspect the preserved actual logs/registry and current diff;
reuse warm independent volumes after confirming settled state. Do not blindly
restart or claim the old frozen manifest covers newer source. No pending
implementation agents exist. Do not stage the user's .DS_Store, .bob/skills/,
path_to_V3.50.md, speedup.md or speedup2.md.
