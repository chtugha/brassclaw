# K3 immutable secret custody — scoped acceptance, 2026-10-26

Receipt: `target/accepted-evidence/k3-secret-custody-20261026/run-4/accepted.json`, SHA `eb7e35762df835ec2a74694b740f12d8160c1d0294de119fe9440ef14ab71fc3`. All22 exact applied source/test/contract hashes and all5 preceding Prefix dispatch-policy sources remain intact. 348 fresh positive Rust cases, zero ignored/zero-case qualifications. Historical R1/R2 failures remain failures; no passed count from either is reused here.

## Implemented boundary

The existing SecretStore now has required first-original retention, authenticated custody metadata and retained-output creation ports, forwarded by every actual implementation. Original input/context/full scope are immutable. An exact repeat returns actual stored metadata without rewriting; changed inputs, occupied ordinary records and retired identities cannot replace it. Raw input never receives a runtime metadata/lease projection. Output creation accepts only a selected live original reference inside its actual owner, never caller replacement material.

The purpose codec reuses existing AES-GCM/HKDF and length-prefixed AAD. Full captured scope/context/source/disposition are authenticated, temporary plaintext ownership zeroizes, raw JSON remains ordinary material and public reserved-System parsing stays closed. Private stored-scope rehydration reuses the previous PG decoder. Old ordinary serialization/reference algorithms remain unchanged; FS optional custody marker is omitted for old ordinary records; PG uses an explicit custody-v1 prefix rather than guessing from raw plaintext.

All ordinary write/CAS/delete paths authenticate their selected record and cannot overwrite or resurrect custody identities. Retirement drops raw material and keeps original/context/source references occupied. Ordinary FS removal uses the previously accepted native version/body conditional single-entry deletion; no unconditional downgrade or stale delete of a recreated immutable original. Memory uses its actual lock; PG native transaction ports order source/target advisory locks before row locks. Provider-style read ports do not acquire a new writer advisory lock after owning their lease row. Read-only original lookups remain nonlocking/nonmigrating.

FS original-to-output capture selects authenticated source DATA at the read. It is neither a cross-record atomic grant nor live authority. Original input retirement does not revoke an independently issued output. AuthProduct account/flow/interaction/result/browser publication, cancellation reconciliation and ordinary mutable-value generation lease fencing remain their following K3 steps. This package exposes the storage foundation; existing manual request consumers are not yet migrated to these new ports.

## Actual evidence

- `actual-storage-test-boundary_contract`: 1 passed.
- `actual-storage-test-brassclaw_secrets`: 65 passed.
- `actual-storage-test-secret_store_contract`: 9 passed.
- `actual-host-obligations--tests--`: 7 passed.
- `actual-host-egress--credential--tests--`: 3 passed.
- `actual-outbound-state-store`: 7 passed.
- `actual-composition-native_secret_custody_tests--`: 9 passed.
- `actual-composition-native_secret_reference_tests--`: 5 passed.
- `actual-composition-manual_token_flow--tests--`: 3 passed.
- `actual-composition-product_auth_durable--tests--`: 48 passed.
- `actual-composition-pg_auth_product_services--refresh_tests--`: 18 passed.
- `actual-composition-oauth_provider_client--tests--`: 13 passed.
- `actual-composition-oauth_dcr--tests--`: 14 passed.
- `actual-composition-llm_config_service--provider_login`: 8 passed.
- `actual-composition-extension_lifecycle--tests--`: 26 passed.
- `actual-composition-native_pg_provider_secret_leases_preserve_scope_and_one_shot_consumption`: 1 passed.
- `actual-composition-native_pg_secret_owner_keys_and_authenticated_legacy_upgrade`: 1 passed.
- `actual-composition-native_secret_transaction_fences_absent_row_wrong_key_writer`: 1 passed.
- `actual-composition-native_model_discovery_uses_explicit_endpoint_policy_and_atomic_credentials`: 1 passed.
- `actual-composition-native_global_runtime_retains_one_root_across_match_and_no_match`: 1 passed.
- `actual-composition-native_global_monty_live_model_selection_retains_inflight_exchange`: 1 passed.
- `actual-public-manual_tokens`: 10 passed.
- `actual-public-webui_v2_product_auth`: 42 passed.
- `actual-public-webui_v2_product_auth_4201`: 41 passed.
- `actual-public-auth_callbacks`: 7 passed.
- `actual-public-auth_lifecycle`: 5 passed.
- `actual-current-reborn-dependency-boundaries`: 1 passed.

The native9-case custody group uses actual PostgreSQL and persistent PG-backed FS with independent owners, user and internal System scopes, different/equal input races, ordinary-writer and delete/recreate races, original/output/retirement COMMIT acknowledgement cuts, held-row/read-only lookups, actual provider lease/reference lock ordering, wrong-key/purpose/ciphertext corruption and copying ciphertext under a different physical namespace. Real writes/readback prove the disposition; operation counters or metadata declarations alone do not. Existing PG legacy upgrade, ordinary key/lease/absence-CAS, model-discovery/login, public manual/callback/lifecycle and both ordinary global Monty callers ran freshly.

Current worker Cargo build: 16.37s; actual observed SHA `a617985957ccb8c5f3f0263f3ffe9135d9e352a86b8235d4bf5e39eba7e431a8`. A 9158-entry conservative complete repository input inventory accompanies it. This is an actual current-tree Cargo build/artifact, not an old530-file equivalence assumption or a minimal dependency closure. Architecture executable reuse checks its complete crate inputs, root Cargo manifest/lock and retained artifact before fresh execution against current source/Cargo metadata.

Strict affected/root tests lint 90.98s; default Composition lint 36.46s; format 4.68s; whitespace 0.08s. Cargo1.96.0 runs serially under the configured one target/common lock in screen. Actual required artifacts were retained before capacity cleanup; no private projection or second Cargo target was compiled.

## Failures and limitations

R1 compiled and ran63/65 Secrets cases successfully but two new fixtures wrongly used reserved System controlbytes in a virtual path and assumed independent memory adapters shared their local leases. R2 corrected adapter ownership but the new fixture used an unknown /internal root (64/65). Read actual path root/normalization and mount contracts; corrected the two test-only destination literals to a canonical /secrets path, leaving actual typed System scope/crypto checks unchanged. R1/R2/R3 production files were identical and R3 freshly passed348 actual cases, but strict lint rejected DecodedSecretValue (344-byte maximum vs16-byte ordinary variant). The final4-file source repair boxes only the custody body, preserving owned material moves and all persisted formats/APIs while reducing ordinary-read size. Complete fresh R4 then executed the full scope and strict/default checks; no failed-run positives are reused. No production fallback, fake success, warning suppression or test bypass was introduced.

This is not complete manual/browser/account/interaction/flow/result custody, ordinary mutable lease-generation fencing, generalized AuthProduct System persistence, final provider-wire/prefix serving acceptance, K3–K7 completion, platform/release/deployment or a remote model-server change. The operator removed the Sempai server-template override. Each provider/model/role has a fixed already model-specific generated prefix. Kohai formats only the Orchestrator prompt part and then adds that exact unchanged prefix; final validation concerns their actual composition. The earlier model-neutral-prefix/whole-prompt-render interpretation was incorrect and has been retracted in the handoff and sent to the Prefix owner. No commit/tag/push/SSH/component activation is claimed.
