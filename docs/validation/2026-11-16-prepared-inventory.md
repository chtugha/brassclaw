# Prepared-package inventory and local branch cleanup — 2026-11-16

Inspected published Main: `989e71d67f8638d2059b85f7cc8298371500ae16`. Read all 60 local branch tips, their own commits, ancestry/patch equivalence, changed source and 25 registered worktrees. Reused applicable accepted successor receipts; branch existence alone is neither missing implementation nor acceptance.

## Publication and classification

- Published the reviewed Modal supervision/activation change as `989e71d67f8638d2059b85f7cc8298371500ae16` to `origin/main`; observed remote SHA matched. This is publication evidence, not a GitHub CI result.
- 17 tips are Main ancestors (including Main); 23 additional branches contain only patch-equivalent changes; 12 historical source packages have reviewed, accepted successors. Do not merge rejected intermediate histories to reproduce these changes.
- Eight branch names remain held by missing shared implementation/provenance contracts or migration collisions. The two `e8583f896` names intentionally identify the same unfinished tip.
- One independently ready change was outside Git branches: the prepared `process_host` admission-test correction. It is the only new build candidate. Additional paid jobs for superseded source would duplicate acceptance rather than add evidence.

## Inactivation performed on explicit user instruction

- Archived 51 exact tips under `refs/archive/accepted/20261116/<original-branch-name>` and removed their local `refs/heads` names. The 52 excluded inventory entries include protected `main`, which remains active.
- Retained all worktree files. Detached 20 worktrees at their unchanged original commits and verified identical tracked/untracked porcelain status before and after. No source changes were discarded.
- Pruned only the stale Git registration for `/private/tmp/brassclaw-j01-implementation` after exact dry-run review; its directory remains intact.
- Nine local branches remain: `main` and the eight held names. No remote references were removed. Archive refs preserve rejected histories as history, not as merge candidates.
- Restore an individual local name, if needed, with `git branch <original-name> refs/archive/accepted/20261116/<original-name>`; this does not check out or merge it.

## Exact branch disposition

| Original local branch | Tip | Decision / remaining prerequisite |
| --- | --- | --- |
| `codex/a02a-canonical-pure-coverage` | `f62e17a12eceef68ab43d48a92fc709d59ceae66` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/a03a-native-review-observation` | `07b700e8098708f33a9a239de4c2994cd95d898f` | Archived/inactive: Original generation/fixture separation retained by later combined-policy/custody implementations. |
| `codex/b00-q1-oracle-review` | `25a9536bdc17ad38d05a460c755b2502f47b4e57` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/b01-original-review-binding` | `dec1785cdb343ea772207009cd921b4518006b28` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/b02-private-work` | `44f20c3a8770475b901a65db85be4081504aab99` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/b03a-prefix-original-ready` | `83efab192d1aaea33291edc02fc0a870c4d9436f` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/b1-06-approval-writer` | `d41db6c37f5144d539fc1d7d86165bbda611e0a0` | Archived/inactive: Guarded writer already integrated; current e4da opaque publication is stricter; rejected 3f802 ancestry must stay excluded. |
| `codex/b1-r06-independent-pure-audit-20261114` | `3ca800ed6295c152c4f06dd398c1c49712782bb6` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/b2-02-retained-selection-v2` | `e0be06d8fbe9c08abd551ac522034255a2e90826` | Archived/inactive: Superseded by current strict shared /1 plus passive own /3; old /2 and V215 payload must not replace current meanings. |
| `codex/b2-r01-retained-contract` | `8b67a4efbb586ff9a28b484d4d29b2082f0f9db3` | Archived/inactive: Corrected serializer policy carried and accepted by current successors; original V209 fixture also integrated. |
| `codex/b3-06-durable-q1-work-wt` | `a069f5df327762391288c3bbc92e7323b44f0695` | Archived/inactive: Original preparation/custody accepted in corrected B3 successor; later startup/close corrections retained. |
| `codex/b3-06-reviewed-integration-20261113` | `3dd52ec6e0d14a277d2015ba84fd908e50721018` | Archived/inactive: Superseded by accepted corrected B3 integration. |
| `codex/b3-09-webui-review-panels` | `f857421e3fe77b2ec437e4e6ab87fcaf4fe84e0f` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/b3-r02-provider-ui-harness` | `3dc94344dff2c06bdf79aef28491f63f2359c590` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/b3-r03-native-factory` | `cd654b869467d09c0b4e49e062524a84de06b87e` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/b3-r05-native-auth-boundaries` | `ec09e6377c330f2ee3d065eaa2c1e27cbb9f1438` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/bob-b2r04-capture-retained-audit-20261114` | `0296da2b5031af88636327dfb4bd5da57bc5b2b3` | Held: Capture2 still lacks V207/V213 readers; its V216 collides with current retired-seed migration. |
| `codex/bob-reviewed-integration-20261113` | `04dfb7c74342c44ab1eb6c6df05ab7fcefab5d1c` | Archived/inactive: Earlier B1 checkpoint superseded by guarded then opaque writer. |
| `codex/bob3-auth-custody` | `cbf2f355e52323df77b7c51ae465b58f8be762d7` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/c00-test-unwind-profile` | `36ce1c4c5c59f460d3d04223088fab0a04a8b29b` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/cargo100-composition-lint-followup` | `489f57998735e1d2332f8d7a609f21e2d04a5b2c` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/ci-native-macos-runners-rc6` | `17951f9f4a450d64a948303708c88b9f71240936` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/d01-private-engine-bridge-retirement` | `b16976c410b020b1b727a64cadf1290a1739961c` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/e01-structural-checker-qualification` | `037b53d2a13c5af3502b1f007c561d5d044fdaa4` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/f01-parser-proof-transaction` | `15959f74b0d0a0b7203a1c299fd4915da2550b42` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/f01-pure-checker-qualification` | `ce00c3dec119e12f4be060042c9eb5c6db3c01fa` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/f01-receipt-step-identity` | `ee80e4b583017a2586e789ce32f8ca563acb9cc2` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/f01-run24-deadline-analysis` | `db2b714070c5ff19a85d02499c3f108cf5014a9d` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/g01-held-transaction-acks` | `dc19b6397eb8d3a843be4d907ccbce5dbc363a3f` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/i01-qualified-pure-source` | `f28dedc753c08edcaa66893b41512ba914383a3a` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/installer-local-snapshot` | `3fa5fb8e89591c3731d1eb8a1a7c8bcb49247d64` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/j01-pure-case` | `3c693fede2f67e7fab7b8cfe1d83ec047c27db73` | Archived/inactive: Original Pure product path integrated with later parser/original/custody fixes. |
| `codex/j01-run38-corrections` | `6215df8ab2f341790c213ab5bd0352a2ccfe264a` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/j01-run40-import` | `f06b66839c6090b9685a247ed67dff8ccda31da6` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/j01-source-completion` | `f98182e92cdd03b6a248b768035126ac3737e6ca` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/k01-snapshot-settings` | `fb6f969dd30d5befc02b523cb09df4d235f45e5f` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/l01-revision-policy` | `5dd34b32beb32d77e427c8c6fd7713224f3a9bae` | Archived/inactive: Original revision policy and V211-V213 integrated with later corrected readers. |
| `codex/m01-codex-product` | `eaa2a292e9da447f8eb43a662d30b57c47e84076` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/n01-e858-preserved` | `e8583f89691773672a7ed69d18fa6f99fb04a010` | Held: Structured flow requires shared selected/occurrence contracts; private V214 conflicts with current approval migration. |
| `codex/n01-fault-custody-b93fa1870` | `456c061284d6a1d7423e0a1730f8532e488dab39` | Held: Structured flow requires shared selected/occurrence contracts; private V214 conflicts with current approval migration. |
| `codex/n01-integrated-f9ef0cd63` | `7c7efe1ef04bebc87d72d0662f9b22e9c1fffaec` | Held: Structured flow requires shared selected/occurrence contracts; private V214 conflicts with current approval migration. |
| `codex/n01-qualified-flow` | `e8583f89691773672a7ed69d18fa6f99fb04a010` | Held: Structured flow requires shared selected/occurrence contracts; private V214 conflicts with current approval migration. |
| `codex/native-bootstrap-cancellation-repair` | `0dd07900d4f1761e1c8b9c02af7358f0b03f5d12` | Archived/inactive: Trace and actual original-read repair integrated; startup qualifier later retired. |
| `codex/o01-codex-responses` | `e9480bcafe6c44bfa065d7495e23d9ed751846d7` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/p01-provider-status` | `df1ca7d2efc72e7f011b41089cf590268d83cc70` | Archived/inactive: Own status files retained; Main additionally registers accepted tests. |
| `codex/q01-capture-policy` | `8ed62ca7b7f01224a3a8e224ba14cdceb8ed9407` | Held: Cold boot needs original historical History/S provenance and connected upgrade readers. |
| `codex/r01-independent-seeds` | `6f645ba0fbbeefc712a03a532e5c3e95efd6698b` | Held: Whole graph needs original usage reconstruction and shared capture readers; installed-startup qualification wiring is superseded by pre-provisioning validation. |
| `codex/reborn-feature-skill-publication` | `064d07a63eb1cfae25babd5148f9d04f2d9debd2` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/s01-private-admission-diagnostics` | `6b15ad7eafca346d55e6a23a3e58463332ba2d29` | Archived/inactive: Original observation and pool diagnostics retained by current successors. |
| `codex/s01-transaction-client-let-chain` | `a41732332661bdbd133c5b7b643954af577a9ae4` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/screen-mcp-ipc-comparison-20261115` | `47a4f50c783e950b3656d7de292cfeb3c44f106b` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/speedup2-implementation` | `04565b6f8705561e46fdced6fa7399147740ec34` | Archived/inactive: All own patches are equivalent to existing main patches; retain later main corrections. |
| `codex/t01-history-qualification` | `92fc13a616fdc2861bb16fd98296f74f6be47843` | Held: Depends on N/Q/R contracts and genuine private Memory source/fixture owner. |
| `codex/v03-cli-smoke` | `227179ca00b01e8e21725fbadcb98ca2865ab5c6` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/w04-test-driver-native-pg` | `3d3fc5ef00b030ebdc9dc511b05ede6fa372aa07` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/w05-root-fs4-md5-followup` | `7c1e9b2bde702a3fb87450b8152a94c48967b337` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `codex/w06-buildflow-review` | `9b8421013380d7538fda72207fd563329e7afe9b` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `main` | `989e71d67f8638d2059b85f7cc8298371500ae16` | Retained Main: Exact tip is an ancestor of published main; no pending branch delta. |
| `postgres/s0-s5-foundation` | `76b19a3578b795ab3c80d6f674d9a74344d75ca6` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |
| `rust-compilations-and-or-cargo-c-531f` | `4648e73b67ac8658ae26dc2ee5218c09f7934f37` | Archived/inactive: Exact tip is an ancestor of published main; no pending branch delta. |

## Worktree-only and other pending inputs

- Git index was empty. Main’s `.DS_Store`, `.bob/skills/`, `path_to_V3.51.md`, `speedup.md`, `speedup2.md` and `unnecessary_stuff_v3_at_bob1.md` were kept separate from the reviewed commit. The latter plan subsequently received an explicit implementation instruction; it is next after this bounded acceptance.
- The IPC experiment worktree’s Python is byte-identical to Main. Its sole remaining shell delta restores a fixed `/private/tmp` default, superseded by the accepted portable system-temporary-directory default. Its untracked experiments remain evidence, not an unreviewed product import.
- B1-R06’s untracked handoff is an audit proposal with no pending implementation; its narrow shipped-R1 work has accepted Main successors. General authored approval remains open in B1.

## Applicable existing evidence

- [Previous prepared integration inventory](2026-11-14-prepared-integration-inventory.md).
- [Original custody acceptance](2026-11-14-prepared-originals-custody-acceptance.md).
- [R05 native authentication acceptance](2026-11-14-prepared-r05-acceptance.md).
- [B2-R01/B3-R03 acceptance](2026-11-14-bob-b2r01-b3r03-acceptance.md).
- [Prior branch/build cleanup](2026-11-15-combined-build-and-branch-cleanup.md).
- [Current native publication boundary](2026-11-16-b1-native-publication-and-seed-errors.md).

Machine-readable full inventories, exact archive mapping, status and the reviewed cleanup runner are retained in `target/validation/prepared-inventory-20261116-r2/`. The failed r1 inventory correctly stopped at the invalid Git registration; r2 records that condition explicitly. Both Screen sessions ended and their final notifications were received. No compilation was performed by the inventories.
