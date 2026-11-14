# Prepared integration inventory — 2026-11-14

Leading-chat work only. All package chats are stopped. Main basis: `5a8aadfe184fd57bf042ea7038526ca52ac746ca`. This is a review inventory, not acceptance of pending branches. Exact branch tips, own net paths, final blob comparisons and patch identities are retained under `target/validation/prepared-integration-20261114/{branches,patch-equivalence,triage}.json`.

## Already integrated source

| Branch | Exact preserved tip | Proof |
| --- | --- | --- |
| `codex/a02a-canonical-pure-coverage` | `f62e17a12eceef68ab43d48a92fc709d59ceae66` | all_own_patches_applied_later_main_successors |
| `codex/b00-q1-oracle-review` | `25a9536bdc17ad38d05a460c755b2502f47b4e57` | all_own_patches_applied_later_main_successors |
| `codex/b01-original-review-binding` | `dec1785cdb343ea772207009cd921b4518006b28` | all_own_patches_applied_later_main_successors |
| `codex/b02-private-work` | `44f20c3a8770475b901a65db85be4081504aab99` | all_own_patches_applied_later_main_successors |
| `codex/b03a-prefix-original-ready` | `83efab192d1aaea33291edc02fc0a870c4d9436f` | all_own_patches_applied_later_main_successors |
| `codex/b1-06-approval-writer` | `d41db6c37f5144d539fc1d7d86165bbda611e0a0` | all_final_delta_blobs_already_in_main |
| `codex/b1-06-reviewed-integration-20261113` | `3cd63dfc32db69bce896adc4a75c131dbca1fcfa` | already_ancestor |
| `codex/b1-r06-independent-pure-audit-20261114` | `3ca800ed6295c152c4f06dd398c1c49712782bb6` | already_ancestor |
| `codex/b2-r01-retained-contract` | `8b67a4efbb586ff9a28b484d4d29b2082f0f9db3` | all_final_delta_blobs_already_in_main |
| `codex/b2-r01-reviewed-integration-20261114` | `c1ae9c4a37f0b6ce78578e2acd084b02454714ab` | already_ancestor |
| `codex/b3-06-corrected-integration-20261113` | `1bc98c0d284e4f966b7c1d72c48ade78deb39866` | already_ancestor |
| `codex/b3-06-durable-q1-work` | `d24a875086a3b738a4b4fb2bcd03fc7652c54e8c` | already_ancestor |
| `codex/b3-09-webui-review-panels` | `f857421e3fe77b2ec437e4e6ab87fcaf4fe84e0f` | already_ancestor |
| `codex/b3-r02-provider-ui-harness` | `3dc94344dff2c06bdf79aef28491f63f2359c590` | already_ancestor |
| `codex/b3-r02-reviewed-integration-20261113` | `3ca800ed6295c152c4f06dd398c1c49712782bb6` | already_ancestor |
| `codex/b3-r03-native-factory` | `cd654b869467d09c0b4e49e062524a84de06b87e` | all_final_delta_blobs_already_in_main |
| `codex/bob-b2r01-b3r03-reviewed-20261114` | `5a8aadfe184fd57bf042ea7038526ca52ac746ca` | already_ancestor |
| `codex/bob3-auth-custody` | `cbf2f355e52323df77b7c51ae465b58f8be762d7` | all_own_patches_applied_later_main_successors |
| `codex/c00-test-unwind-profile` | `36ce1c4c5c59f460d3d04223088fab0a04a8b29b` | all_own_patches_applied_later_main_successors |
| `codex/cargo-manifest-warning-causes` | `caa5064bede8e4507c6a788dcc4121952f9a5a23` | already_ancestor |
| `codex/cargo100-composition-lint-followup` | `489f57998735e1d2332f8d7a609f21e2d04a5b2c` | already_ancestor |
| `codex/cargo100-rust-lint-followup` | `5261194e42c4550c55aa219f789d85ee604c725b` | already_ancestor |
| `codex/ci-native-macos-runners-rc6` | `17951f9f4a450d64a948303708c88b9f71240936` | already_ancestor |
| `codex/d01-private-engine-bridge-retirement` | `b16976c410b020b1b727a64cadf1290a1739961c` | all_own_patches_applied_later_main_successors |
| `codex/e01-structural-checker-qualification` | `037b53d2a13c5af3502b1f007c561d5d044fdaa4` | all_own_patches_applied_later_main_successors |
| `codex/f01-parser-proof-transaction` | `15959f74b0d0a0b7203a1c299fd4915da2550b42` | all_own_patches_applied_later_main_successors |
| `codex/f01-pure-checker-qualification` | `ce00c3dec119e12f4be060042c9eb5c6db3c01fa` | all_own_patches_applied_later_main_successors |
| `codex/f01-receipt-step-identity` | `ee80e4b583017a2586e789ce32f8ca563acb9cc2` | all_own_patches_applied_later_main_successors |
| `codex/f01-run24-deadline-analysis` | `db2b714070c5ff19a85d02499c3f108cf5014a9d` | all_own_patches_applied_later_main_successors |
| `codex/g01-held-transaction-acks` | `dc19b6397eb8d3a843be4d907ccbce5dbc363a3f` | all_own_patches_applied_later_main_successors |
| `codex/h01-original-work-custody` | `7ca1f30e35965d0d5510e7b524b0f9e12e610dc6` | all_own_patches_applied_later_main_successors |
| `codex/i01-qualified-pure-source` | `f28dedc753c08edcaa66893b41512ba914383a3a` | all_own_patches_applied_later_main_successors |
| `codex/installer-local-snapshot` | `3fa5fb8e89591c3731d1eb8a1a7c8bcb49247d64` | already_ancestor |
| `codex/j01-run38-corrections` | `6215df8ab2f341790c213ab5bd0352a2ccfe264a` | all_own_patches_applied_later_main_successors |
| `codex/j01-run40-import` | `f06b66839c6090b9685a247ed67dff8ccda31da6` | all_final_delta_blobs_already_in_main |
| `codex/j01-source-completion` | `f98182e92cdd03b6a248b768035126ac3737e6ca` | all_own_patches_applied_later_main_successors |
| `codex/k01-snapshot-settings` | `fb6f969dd30d5befc02b523cb09df4d235f45e5f` | all_own_patches_applied_later_main_successors |
| `codex/m01-codex-product` | `eaa2a292e9da447f8eb43a662d30b57c47e84076` | all_own_patches_applied_later_main_successors |
| `codex/m01-codex-use-conflict` | `4ddc8c193d2e3360ecb3aeee749be4019dbe2c20` | all_own_patches_applied_later_main_successors |
| `codex/m01-startup-owner-fixture` | `bbd4e12d4ef40af4e58dffc2071f2e6361bd75da` | all_own_patches_applied_later_main_successors |
| `codex/o01-codex-responses` | `e9480bcafe6c44bfa065d7495e23d9ed751846d7` | all_own_patches_applied_later_main_successors |
| `codex/reborn-feature-skill-publication` | `064d07a63eb1cfae25babd5148f9d04f2d9debd2` | already_ancestor |
| `codex/repair-clippy-composition-16` | `a8423bd6754c00b41fc844a94106acd786b2acc6` | all_own_patches_applied_later_main_successors |
| `codex/s01-codex-request-inspect-error` | `7d366a816dc288faf05570ec36dc8b0c746772aa` | all_final_delta_blobs_already_in_main |
| `codex/s01-memory-decoder-as-chunks` | `35867b50dd7a6fc3bd0276407077ab56b0f28a61` | all_final_delta_blobs_already_in_main |
| `codex/s01-process-error-layout` | `46a895f20aebc332c6c02361b6478c8f4d5aaf0c` | all_final_delta_blobs_already_in_main |
| `codex/s01-transaction-client-let-chain` | `a41732332661bdbd133c5b7b643954af577a9ae4` | all_own_patches_applied_later_main_successors |
| `codex/speedup2-implementation` | `04565b6f8705561e46fdced6fa7399147740ec34` | all_own_patches_applied_later_main_successors |
| `codex/t01-architecture-test-gates` | `3812dae055ff788a1854e9edcd90a70b6dc02d95` | all_final_delta_blobs_already_in_main |
| `codex/t01-bounded-record-checkpoint` | `82edcdb57f483219061055c78e102ce3d38d5284` | all_own_patches_applied_later_main_successors |
| `codex/t01-skills-lint-baseline` | `b7c5aa988ce057bec0f3927d7089a9e9cc61101a` | all_final_delta_blobs_already_in_main |
| `codex/v03-cli-smoke` | `227179ca00b01e8e21725fbadcb98ca2865ab5c6` | all_final_delta_blobs_already_in_main |
| `codex/w04-test-driver-native-pg` | `3d3fc5ef00b030ebdc9dc511b05ede6fa372aa07` | already_ancestor |
| `codex/w05-cargo100-followup-18` | `55828a04cd5e74fb2357a6e3de919ed8f7ae9ac4` | all_own_patches_applied_later_main_successors |
| `codex/w05-root-fs4-md5-followup` | `7c1e9b2bde702a3fb87450b8152a94c48967b337` | all_final_delta_blobs_already_in_main |
| `codex/w05-unused-dependency-edges` | `c698241d7299c43f27faee6a697de16b24ad7b6a` | already_ancestor |
| `codex/w06-buildflow-review` | `9b8421013380d7538fda72207fd563329e7afe9b` | already_ancestor |
| `codex/w06-local-ci` | `dd277b7ab64abf7a0e5432f3c5a4421e56ade29f` | already_ancestor |
| `postgres/s0-s5-foundation` | `76b19a3578b795ab3c80d6f674d9a74344d75ca6` | already_ancestor |
| `rust-compilations-and-or-cargo-c-531f` | `4648e73b67ac8658ae26dc2ee5218c09f7934f37` | already_ancestor |

A current file differing from an old package file does not imply missing implementation: later Main fixes must remain. No old whole-file replacement is authorized by this inventory. B1-06 final source is already identical on all four own paths, but its ancestry contains explicitly rejected `3f802f79c`; do not import that history as an acceptance shortcut.

## Remaining delta review

| Branch | Exact preserved tip | State |
| --- | --- | --- |
| `codex/a03a-native-review-observation` | `07b700e8098708f33a9a239de4c2994cd95d898f` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/b2-02-retained-selection-v2` | `e0be06d8fbe9c08abd551ac522034255a2e90826` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/b3-06-durable-q1-work-wt` | `a069f5df327762391288c3bbc92e7323b44f0695` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/b3-06-reviewed-integration-20261113` | `3dd52ec6e0d14a277d2015ba84fd908e50721018` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/b3-r05-native-auth-boundaries` | `ec09e6377c330f2ee3d065eaa2c1e27cbb9f1438` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/bob-b2r04-capture-retained-audit-20261114` | `0296da2b5031af88636327dfb4bd5da57bc5b2b3` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/bob-reviewed-integration-20261113` | `04dfb7c74342c44ab1eb6c6df05ab7fcefab5d1c` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/j01-pure-case` | `3c693fede2f67e7fab7b8cfe1d83ec047c27db73` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/l01-revision-policy` | `5dd34b32beb32d77e427c8c6fd7713224f3a9bae` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/n01-e858-preserved` | `e8583f89691773672a7ed69d18fa6f99fb04a010` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/n01-fault-custody-b93fa1870` | `456c061284d6a1d7423e0a1730f8532e488dab39` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/n01-integrated-f9ef0cd63` | `7c7efe1ef04bebc87d72d0662f9b22e9c1fffaec` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/n01-qualified-flow` | `e8583f89691773672a7ed69d18fa6f99fb04a010` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/native-bootstrap-cancellation-repair` | `0dd07900d4f1761e1c8b9c02af7358f0b03f5d12` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/p01-provider-status` | `df1ca7d2efc72e7f011b41089cf590268d83cc70` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/q01-capture-policy` | `8ed62ca7b7f01224a3a8e224ba14cdceb8ed9407` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/r01-independent-seeds` | `6f645ba0fbbeefc712a03a532e5c3e95efd6698b` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/s01-private-admission-diagnostics` | `6b15ad7eafca346d55e6a23a3e58463332ba2d29` | Review individual net changes and prerequisites; no acceptance claim |
| `codex/t01-history-qualification` | `92fc13a616fdc2861bb16fd98296f74f6be47843` | Review individual net changes and prerequisites; no acceptance claim |

## Connected findings already established

- A03 generation approval/validation: original executor and original fixture separation is present in Main. Later changes replace direct hold-owner hashing with the existing combined owner checksum, pass the existing graph policy and release the DB checkout before controlled close. Replaying the old branch would undo these repairs.
- P01: all four own product/status files are already identical except `composition/lib.rs`, where Main additionally registers the accepted bounded-record native tests. No missing P implementation.
- L01/J01: connected J/I/K and L source was integrated previously; retained original parser provenance, transaction-bound restore and V211–V213 are present. Current differences include later boxed error/subject shapes, shared admission custody and corrected native fixture bounds, rather than a pending original feature import. Full per-path successor diffs are in the local inventory directory.
- S01 admission diagnostics are already present through the concrete factory observation, intent-boundary observer and independent diagnostic pool; later settlement observations extend this source. The separate 19-line historical-read trace amendment remains to be assessed for current usefulness; it is not product behavior.
- R05 is independently eligible: shared Auth log redaction and three actual PG/HTTP regression sources need no new format, approval writer or cancellation API. Root strengthened the new-login race assertion to exact BackendConflict. Candidate source is frozen while serial strict/native compilation runs; execution and Main integration remain pending.
- R04 contains a separable original selection-parser-limit carrier/accessor for the already-supported /1 and passive /3 serializers. It needs no Capture2/default/settings migration. Review and exercise the actual serializers before accepting this smaller fragment.
- Whole R04 default Capture2 still lacks the explicit V207 prefix and V213 private-original SQL reader adoption. Its own handoff confirms those are unimplemented; do not enable the new default merely to import the limit accessor.
- Q cold boot expressly rejects old installation/authored catalogues lacking complete original History/S provenance. The new startup formats cannot be shipped as a transparent upgrade while the corresponding historical-original/upgrade owner is absent. Preserve the current legacy reader and inspect independent fragments separately.
- N adds structured occurrence/runtime/qualification support. A computed Git-only integration finds eight textual conflicts, plus semantic work: its older separate /1-/2 passive codec overlaps the current shared /1-/3 codec; old migration V214 collides with accepted immutable-approval V214. These must be resolved in the owning contract, not hidden with a whole-file replacement or an arbitrary permissive parser. No claim that conflict repair alone proves remaining prerequisites absent.
- R and T include inherited N/Q source. Their whole-branch readiness is not inferred from a clean worktree or formatting. R historical consumer-usage reconstruction and T private Memory Spec are explicitly open in their handoffs. The latter must not be claimed as supported private Memory execution; inspect any independent Chat/native-warning fragment on its own actual prerequisites.

No newer B1 feature work starts before the eligible prepared backlog is accepted. Untracked human files, private branch tips, worktrees and prior failed receipts remain preserved.
