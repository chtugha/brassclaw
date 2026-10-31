# Native manual-token original and atomic publication — 26 October 2026

Status: **scoped native acceptance; K3/full-plan completion remains open**.

The ordinary PostgreSQL AuthProduct owner now retains the original raw input only
in the existing encrypted SecretStore, with the exact selected account snapshot,
intended output and original result. Its high-level manual-token operation commits
immutable input/output, account publication, consumed interaction and completed
flow in one actual PostgreSQL transaction. Same-input retries recover that exact
result after restart, lost acknowledgement or later account changes; changed input
conflicts without selecting a newer account or dispatching a continuation.

The lower AuthInteractionService operation preserves its separate contract: it
publishes the account/interaction and does not complete the correlated flow. An
unbound native setup creates its interaction-derived account rather than treating
another account as an implicitly authorized update. Explicit bound snapshots are
checked before effects; legacy bound setups can use their original retained flow
snapshot, never a reconstruction from the latest account.

The concrete factory-owned PgSecretStore is unchanged as an owner. Its new paired
transaction port acquires one sorted lock set and reuses existing locked original/
retained insertion helpers. A caller must roll back None/error; the secret port
does not commit AuthProduct records. No unknown commit permits compensating
deletion, revocation or replacement of new/preceding material.

## Authoritative evidence

Receipt: `target/accepted-evidence/k3-native-manual-publication-20261026/run-2/accepted.json`.

SHA-256: `da198317a5ac405f1034e0abdc801d9a1637c306ee86b8d1e19194fa054f4945`.

The preserved runner is `original-validation.py` beside that receipt. All eight
final author hashes and archived executable hashes were independently rechecked
after completion. The source snapshot has 9,727 files; the worker applicability
inventory conservatively includes 9,160 repository/runtime/package inputs. This
is not a claimed minimal dependency closure. The preceding Prefix dispatch source,
configured-model package and corrected 20-document contract are explicitly guarded.

The actual worker build command succeeded in 0.46 seconds using the current
common target; its executable was cached, so this is **not** a fresh compiler
execution claim. The changed native/Secret library test programs were built in
202.34 seconds and public test targets in 4.95 seconds. The existing architecture
test executable was reused only after checking its source/workspace applicability
and executable hash, then freshly executed against the current tree. No prior
test execution was counted as a fresh result.

## Executed checks

All **283** Rust cases below were freshly executed; every group reports zero
failed and zero ignored tests.

| Group | Passed | Seconds |
| --- | ---: | ---: |
| `actual-secret-library` | 65 | 0.55 |
| `actual-composition-pg_auth_product_services--manual--tests--` | 9 | 19.74 |
| `actual-composition-manual_token_flow--tests--` | 3 | 4.69 |
| `actual-composition-pg_auth_product_services--refresh_tests--` | 18 | 33.35 |
| `actual-composition-oauth_provider_client--tests--` | 13 | 9.31 |
| `actual-composition-product_auth_durable--tests--` | 48 | 7.61 |
| `actual-composition-native_secret_custody_tests--` | 9 | 16.79 |
| `actual-composition-interceptor_config_service--tests--` | 10 | 0.01 |
| `actual-composition-native_global_runtime_retains_one_root_across_match_and_no_match` | 1 | 71.94 |
| `actual-composition-native_global_monty_live_model_selection_retains_inflight_exchange` | 1 | 36.49 |
| `actual-public-manual_tokens` | 10 | 0.26 |
| `actual-public-webui_v2_product_auth` | 42 | 30.55 |
| `actual-public-webui_v2_product_auth_4201` | 41 | 0.43 |
| `actual-public-auth_callbacks` | 7 | 0.26 |
| `actual-public-auth_lifecycle` | 5 | 0.27 |
| `actual-current-reborn-dependency-boundaries` | 1 | 0.05 |

Strict Root/Composition/Secrets including test targets: 102.77 seconds. PostgreSQL
without RootLLM: 19.68 seconds. Default Composition: 16.11 seconds. All use
`-D warnings`; no warning suppression was added. Workspace formatting passed in
4.61 seconds and authored whitespace in 0.19 seconds. One serial detached screen
and the existing common Cargo target/lock were used; no remote build was run.

The nine new real native cases cover:

- Actual production-shaped facade: failed continuation, changed raw conflict, original retry.
- Original-flow-backed legacy bound setup.
- Occupied output and rollback without cleanup/overwrite.
- Restart/continuation acknowledgement/later revocation and held account row.
- Real rejected final-flow write rolling back all records.
- Lost COMMIT acknowledgement and newer-account non-substitution.
- Independent changed-input owners with exactly one winner.
- Canceled flow and stale bound setup with no secret publication.
- Unbound setup preserving another account and its material.

## Preserved failure

Run1 remains FAILED before any Cargo/test execution: its prerequisite comparison
incorrectly required the old custody-era OAuth test constructor instead of the
already accepted native-owner successor. Run2 verifies that successor receipt
and its exact inputs explicitly. This runner correction changed no author source
and did not suppress a compiler/runtime failure.

## Remaining work and scope limits

The generic/Filesystem raw-input/automatic-selection/publication path remains
open in [the existing K3 step2 owner](../plans/simplified-v3-implementation.md).
Its account/flow receipt protocol must use real CAS; this native transaction is
not evidence of multi-record filesystem atomicity. The generic facade still has
its legacy latest-account recovery; the native path prevents entering it on an
unverified repeat, but that does not repair every generic caller.

The browser still needs original setup/submit identity before transport and a
separate explicit reference-only continuation instead of skipping new raw input
because credentialRef was cached. Dedicated setup/original grants, issuer expiry,
ordinary generation/final-use fencing, DCR artifact disposition, remaining
protocol/policy/cap migration, K4–K7 and the full plans remain open. Public Rust
HTTP cases are not browser acceptance. Prefix designs/default-template serving,
all-platform/deployment acceptance and a release are not established here.

## Exact changed inputs

| File | SHA-256 |
| --- | --- |
| `crates/brassclaw_secrets/src/pg_store.rs` | `8ebed9929bed170da0932fac5fa4be7fad7a1cdc22f08d0636a0c1bcb798f523` |
| `crates/brassclaw_reborn_composition/src/pg_auth_product_services.rs` | `b3d5cc1edef9bbae39140c24b72f375dbebefd0f5d630ca849bb3c3cbb8790cf` |
| `crates/brassclaw_reborn_composition/src/pg_auth_callback.rs` | `391eea61c10f95782bc08e0767ef18bbcaff611428a318b44137340f211e5c1e` |
| `crates/brassclaw_reborn_composition/src/pg_auth_manual.rs` | `2ad83febb8ff23d0131512c5e45c700a5f1f3225225634eee84575710fbfaa01` |
| `crates/brassclaw_reborn_composition/src/pg_auth_manual_tests.rs` | `08c97151753f2c5a7f50536902d7768ecac690bd3ed9efea20e8b9afa8e13c3a` |
| `crates/brassclaw_secrets/CLAUDE.md` | `587d5fe51423afd60ceaef5f893900c721409a8bde27b30f1816483a909f4d08` |
| `docs/reborn/contracts/auth-product.md` | `ac816d68b8715b60e3711920dc49c164502499858d421b50549e460c04ca480a` |
| `CHANGELOG.md` | `337a2d0c3556906f5152a59eba51cc7604e20d998734edee40ac8ca1005babaa` |
