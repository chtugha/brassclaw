# K3 selected-entry deletion — scoped acceptance, 2026-10-26

Receipt: `target/accepted-evidence/k3-conditional-entry-delete-20261026/run-1/accepted.json`, SHA `eba3eebb485d75220f63733ec51d77205d5ddf1fae3ba776e54bbeee6298baad`. All12 exact-base applied source/test/document hashes and Prefix tokenpartition eight source hashes match.116 fresh positive Rust cases, zero failures/ignored/zero-case qualification.

## Implemented boundary

- Native memory and PostgreSQL `delete_if_snapshot` check the exact selected version/body under the same owner mutex/transaction as the deletion. Only one entry is removed; missing, changed versions/bodies and directories return false without deleting descendants.
- Scoped forwarding preserves the existing Delete permission and mount mapping. Composite routing preserves the exact arguments. Appended ConditionalEntryDelete capability is explicitly advertised only by native implementations and checked at mount time; Local defaults fail before effects and cannot overclaim it.
- Native PG uses the existing hierarchical namespace lock and explicit ReadCommitted transaction. A protocol transport cuts the actual COMMIT acknowledgement after the real delete: the method returns an error, readback proves removal, and a stale retry cannot delete the later recreated successor.
- Existing recursive delete is unchanged. No Rootfilesystem conditional operation is weakened into unconditional delete, and no transaction/queue/approval/Tool/workflow subsystem is added. This is an existing K3 step2 primitive prerequisite; full immutable input/manual callers remain open.

## Actual evidence

- `actual-test-brassclaw_filesystem`: 67 passed.
- `actual-test-catalog_contract`: 6 passed.
- `actual-test-conditional_entry_delete_contract`: 3 passed.
- `actual-test-db_root_filesystem_contract`: 1 passed.
- `actual-test-filesystem_contract`: 27 passed.
- `actual-test-metadata_contract`: 1 passed.
- `actual-test-namespace_contract`: 1 passed.
- `actual-composition-native_entry_delete_tests`: 2 passed.
- `actual-composition-native_secret_reference_tests`: 5 passed.
- `actual-composition-native_global_runtime_retains_one_root_across_match_and_no_match`: 1 passed.
- `actual-composition-native_global_monty_live_model_selection_retains_inflight_exchange`: 1 passed.
- `actual-current-reborn-dependency-boundaries`: 1 passed.

The worker was genuinely rebuilt from the common tree in16.96s. Its SHA is `ec89af721b3d35e95f5cdf398b955389d2c8dc39c5f168fdd1c78cdfae66c6cb`, byte-identical to Prefix's prior freshly built worker even after this change; that is an observation after rebuilding, not a reason assumed before it. Both retained global Match/No-Match and live model selection callers ran freshly with this actual executable.

Worker provenance now retains the complete common source inventory and a deliberately conservative9153-entry repository input map, including Resources/Common/ModelEncoding/Filesystem and the actual lock/patch/embedded inputs. It is not a minimal dependency closure. Nonembedded Markdown and scanner-only records follow the existing source-guard rules. Future unrelated-diff reuse needs explicit qualification; never reuse the incomplete old530-file host/vendor-only list.

The earlier [CAS-ABA report](2026-10-26-k3-secret-cas-aba.md) now records that inherited list limitation explicitly. Its observed904 results and immutable receipt are preserved; they are not added to this116-case fresh count. The new actual worker/global cases supersede the missing current-worker equivalence claim. No storage or source-only result is relabeled as deployment/platform/full-plan acceptance.

Checks:

- `affected-root-consumers-strict-lint`: passed (108.97s).
- `default-composition-lint`: passed (26.52s).
- `final-format`: passed (4.77s).
- `diff-whitespace`: passed (0.19s).

Cargo1.96.0 runs serially in screen under the shared lock with the configured canonical target. Worker and every executed target are retained before cleanup. Current architecture test logic was reused only after verifying its exact source/executable, then executed freshly against current Cargo metadata; no redundant architecture compilation. The first complete run passed without source, compiler, behavioral or warning repairs.

## Remaining work

Original/retained secret codecs/required ports/all-writer guards are a private draft, not active code. Their actual memory/native PG/persistent FS races, lost acknowledgements, legacy/system/tamper/retirement/lease tests, native provider caller lock order and full manual/browser/account/interaction/flow/result custody still require connected implementation and acceptance. Existing K3 step4 owns remaining lease-generation capture; K3 step6 owns other compatibility/settings consumers. K3–K7 and overall plans remain open. No commit/tag/release/SSH, external approval or component activation is claimed.

## Successor — 2026-10-26

The subsequent [immutable custody acceptance](2026-10-26-k3-secret-custody.md)
now connects this primitive to ordinary SecretStore deletion and actual
original/retained/retired owners. The earlier private-draft description records
this report's original boundary; its116-case receipt remains immutable. Full
manual/browser/account/flow/interaction publication is still open.
