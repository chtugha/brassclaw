# CI prerequisite verification: retain coherent snapshots without a whole-gallery I/O deadline

**Status — 2026-11-16: scoped implementation and acceptance complete.** All nine
steps below are implemented and reviewed. Nine focused native regressions and
the complete deployment/model-original case pass, with the previously passed
Input/held-ACK cases reused. Strict CLI/Composition all-targets lint passes for
the current Rust inputs. The final artifact/result and exact applicability are
recorded in [the receipt](../validation/2026-11-16-bootstrap-io-originals.md).
Independent GitHub runner/spawn findings and general Bob activation remain open.

Observed 2026-11-16 in all three genuine native ACK cases: `qualify_pure_workflow`
returns `pure_workflow_prerequisites`, revision 1, timeout 2000 ms. The existing
`bootstrap_evidence::io::bounded_for` encloses observation readback, all nine
structural originals, all twelve Pure originals and the parser original in one
deadline. These are separate complete original-verification operations; the same
run already verified individual originals under that configured deadline before
entering this aggregate. Do not replace the configured deadline with a larger
constant, disable it, skip prerequisites, introduce a second approval owner or
repeat completed native execution.

1. Keep the existing one RepeatableRead/read-only transaction and the existing
   `EvidenceClient` custody. Bound pool acquisition, transaction start and commit
   individually through the same existing I/O owner. On any failure, the uncertain
   connection is detached by its existing Drop contract; recycle only after an
   acknowledged commit. Do not split the proof across database snapshots.
2. Remove only the aggregate `pure_workflow_prerequisites` wrapper. Keep all
   actual observation, checker and parser verification calls and their ordering.
   Bound the parser original as one logical original operation.
3. In the existing `InstallationSourceObservations` reader, bound each actual
   observed profile's complete `read_completed` call with its own captured
   ResourceStore settings and subject identity. Preserve both required profiles,
   exact bytes/status and the supplied transaction; no nested pool or dispatch.
4. In the existing structural reader, bound the coherence check and each original
   complete approval verification individually. Preserve its exact nine roots,
   receipt prerequisite chain and byte/integrity comparisons.
5. In the existing Pure reader, bound its coherence check and each complete
   original `verify_final` individually. Keep all twelve expected scopes,
   structural prerequisites, original order and exact references. Do not cache
   positive results between invocations or use in-memory receipts instead of DB
   readback. Each logical operation captures the existing acknowledged technical
   policy; no new numeric value or user setting is introduced.
6. Audit the complete caller chain before the next compilation. The final boot
   pass has the same aggregate wrappers (`boot_inventory_originals`,
   `boot_structural_checkers`, `boot_pure_checkers`, `boot_pure_workflow`); the
   subsequent genuine run proved the latter two still fail at 2000 ms. Remove
   those aggregate wrappers only after every reader owns its complete-original
   containment, including its Shipped branch. Keep the same final transaction.
   Retain the existing acknowledged settings source on the private checked
   structural/Pure owners so empty-context transferred proofs remain bounded.
   Carry that same source through the existing Shipped observation/workflow
   constructors. No new authority, serialized field or operator flag is added.
   Bound each workflow case and the final workflow claim/report independently;
   checkers keep their own complete-original bounds. Apply the same separation
   to the structural prerequisites consumed by `qualify_pure`.
   Bound the existing final boot transaction's acquisition/begin/individual
   metadata exchanges/commit; recycle only on acknowledged success. Keep all
   integrity/selection/routing checks and the same catalogue generation.
   Do not globally alter `bounded_for`, invent a per-platform timing exception,
   relax real held-ACK custody or remove independent task/test deadlines.
7. The Pure subject inspection test must explicitly enter the already existing
   development qualification scope at startup. Installation transfer intentionally
   creates no instance-local native subject journal. All real subject mutation,
   exact walk-count, foreign database, pool-one and smaller-live-limit assertions
   remain, including no replay of completed observations.
8. Review the connected changes, build the updated Composition harness once with
   its warm cache and normal worker, and retain actual Cargo artifacts/checksums.
   Run both structural ACK cases, the Pure original test, shipped R1 ACK case and
   existing held-intent deadline/custody regression on actual private PostgreSQL.
   Run the latest Prefix reservation and two Input tests from that same harness.
   Independent cases may run in isolated containers. No repeated cold compilation.
9. Verify formatting and relevant strict lint for the resulting source. Record
   actual case results, preserved earlier failures and exact source applicability.
   Return to the Unnecessary plan's remaining acceptance, then CI/B1 priorities.

## Historical implementation and validation journal

The following entries retain the original failures and intermediate pending
states; the current status above and final receipt supersede those pending states.

Status: implementation/source review complete. Updated all-targets CLI/Composition
Clippy with `postgres,skills-db` and `-D warnings` passes, job
`6b61210e-d964-4719-b6e7-efb990cae7b4`, source manifest
`988929e0db0fc0a9c886f2fc9cd5dc849fae39e755a1462199849212dbd912de`.
Shared native build `cea2ea67-fb8c-460e-a5e3-503b58ac1588` succeeded and its two Input
cases plus the real held-COMMIT deadline/custody regression pass. The other five
cases expose the final boot aggregate wrappers left by the first correction;
their exact failures are retained in `ci-composition-accept-20261116-r3`. Complete
step 6 and its whole caller-chain audit before another compilation. Earlier
aggregate failures and the missing native-subject fixture failure remain preserved.

The complete step-6 caller correction is now prepared privately, including the
transferred branches, the instrument/function pair reader and the existing final
publication transaction. It is not included in the accepted CLI checkpoint and
is not covered by the earlier strict/native results. Next: strict lint of these
exact new bytes first, then one shared native build. Run the five failed cases
and `fresh_and_repeated_start_transfer_shipped_validation_without_qualification_work`
from that same harness. The two unchanged Input cases and unchanged held-intent
regression already passed and need no replay. Independent operator authoring/
inspection request deadlines retain their existing contract; this repair removes
aggregate qualification-gallery wrappers, not those request limits.

Updated strict lint now passes for that complete follow-up: job
`cfd0f7c3-8b09-4607-a91e-cd83e9638264`, manifest
`b6bbfdd2005757f745a2b0294222bf78941bebc1873f3c935fe82d64f67b288b`.
At submission, all ten changed Rust files matched the linted snapshot. The shared native build
`78a61584-3395-4ae7-bb89-dcaa43d6df5f` compiled and collected its verified native
test/normal-worker artifacts successfully. Runtime acceptance r4 is settled;
five complete native cases pass. Deployment proceeds through real Prefix and
Sempai source jobs, then exposes an outdated Chat `/1` status assertion at line
578. Its private `/2` projection is already the supported production contract.
The corrected test must require the exact private original/status/delivery and
absence of chat fields, retaining the five actual calls and receipt checks.
The entire remaining test was reviewed; its complete native result is still
required, not inferred from the passing prefix substeps. See the
[actual five-pass/one-fail receipt](../validation/2026-11-16-bootstrap-io-originals.md).

Adjacent fixture audit found `bootstrap_checker_tests.rs` starting ordinary
installation in the pruning/originals test and in the intent-ACK and worker-death/
late-cancellation helpers. Those tests inspect native qualification journals or
inject faults into qualification, so they must also explicitly enter the existing
test-only `qualify_package` scope. Keep ordinary startup transfer unchanged; wrap
both first and repeated development boots, preserve every fault/count assertion,
and verify those four real cases before claiming the structural fixture migration
complete. All five such startup calls are now corrected with the same existing
scope, without changing production or assertions. This follow-up and the private
status assertion are not part of the already captured r3 build. Reuse its five
successful results; the next shared harness runs the four newly corrected
structural cases plus the full failed deployment case, not those five again.

The complete eleven-file Composition follow-up is captured for strict lint job
`f3c3bff0-b81c-4bb6-ae88-828f922271b3` and shared native build
`31d3cd16-47f7-464a-83d3-41a6107eca34`. Both use manifest
`62fa6ca48854178653d98dc8f238d45399207890b865f06f19db6663350db42c`
and the existing Rust 1.100 image on independent locked Volumes. They may compile
concurrently; no Cargo target/source directory is shared. The exact changed Rust
delta against Main `38486c8a8` is retained with build r4. Strict all-targets
CLI/Composition lint with `postgres,skills-db` and `-D warnings` passes for that
exact snapshot; all eleven changed Rust files still match. Its Screen ended with
exit 0, no signal/error and accepted/received final IPC, with socket absence
confirmed. Native build r4 also compiled and collected successfully, including
the exact normal worker. Its Screen ended with confirmed final delivery/socket
absence. R5 now executes the four structural fixtures and complete deployment
case from that one new harness; its actual runtime results remain pending.

R5 is now settled: all four structural fixtures pass. Full deployment passed
the corrected private status view, then failed in its nested real model import
helper because the runner did not configure frozen model assets. The remaining
fix is runner preparation, not more Rust changes: reuse the exact existing six-
file Ornith preparation owner, pinned CPU oracle requirements and offline HF
oracle; verify them as the actual fixture user before any PostgreSQL startup.
Pass `BRASSCLAW_TEST_MODEL_ARTIFACT_ROOT` and `BRASSCLAW_TEST_PYTHON` explicitly.
Retain exact source/helper hashes, installation identity/freeze and real preflight
output. R6 repeats only that failed deployment case using the unchanged r4 binary.
No new compilation, image, provider-file access, model weights or dummy assets.


R6's real offline oracle and complete model-measurement subpaths now pass.
The full case next exposes only the provider inspection timestamp in its
inactive-model no-mutation comparison. Normalize that RFC 3339 observation field
alone after validating it; retain comparison of all other provider/auth/role data.
Capture the corrected test in one warm harness and strict lint, then repeat only
the full failed case with the already verified oracle cache. Exact failures and
source applicability remain in the linked receipt. The unrelated GitHub utility
spawn race and compilation-charging timer require a separate owning CI subplan.
