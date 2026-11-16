# CI prerequisite verification: retain coherent snapshots without a whole-gallery I/O deadline

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
