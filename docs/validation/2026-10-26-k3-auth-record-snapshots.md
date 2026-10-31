# AuthProduct original entry snapshot fences — 26 October 2026

Status: **scoped acceptance of AuthProduct filesystem record writes**. Full
manual-token publication, K3–K7 and the complete plans remain open.

The old AuthProduct metadata writers compared a record version, which real
backends can reuse after physical deletion and recreation. A writer paused after
its last read could therefore overwrite a subsequently revoked account or admit
a callback after its flow was canceled. Existing secret-value snapshot protection
did not repair these separate AuthProduct account/flow/interaction/action records.

The existing owner now captures AuthRecordSnapshot (version plus exact entry-body
checksum) in the same read and carries it through every conditional metadata
writer. It uses the already implemented atomic EntryBodyCas backend primitive.
Account/flow helper rereads cannot substitute a current checksum for the original
one. New records retain absent-only insertion; weak/version-only/Any writes are
rejected and an unsupported backend stays unavailable. Snapshot mismatch becomes
BackendConflict. There is no additional database round trip for the final check.

CAS callback publication decisions also retain the original selected account
body identity. Exact existing account receipts can settle historical publication
without rewriting a later account. Historical pending decisions with no captured
body identity and no matching receipt remain preserved and conflicting. Their
controlled upgrade must drain/reconcile before cutover; this repair does not infer
a historical identity from current data or establish complete upgrade recovery.

The connected contract fixes obsolete libSQL-runtime wording and the ambiguous
absence-precondition sentence. Changelog includes the Prefix owner’s accepted
format-source observations before qualification. No task workflow or Rust agent
fallback was introduced.

## Authoritative evidence

Receipt: `target/accepted-evidence/k3-auth-record-snapshot-20261026/run-2/accepted.json`.

SHA-256: `d995d44d2b86fe2f8165f66bcd64cd9f999e0d8587935e72c6ab707704f6390f`.

All13 current source hashes and archived executable hashes were rechecked after
completion. Source inventory:9730 files; conservative worker/package inputs:9161.
The permitted changes from native-manual283 are exactly this13-file package plus
the Prefix owner’s accepted9-file format-source successor, recorded in
`accepted-prerequisites.json`. Unexpected runtime/package input changes fail the
runner. The actual six original model files are hash checked for the Prefix case.

One detached screen and common inherited Cargo target/serial lock were used.
Actual library test build191.65s, public targets4.82s. The current worker command
succeeded in0.47s using its cached executable; this is not a fresh worker compiler
claim. The architecture executable was retained after exact source/workspace/hash
qualification and freshly executed on the current tree. No previous test result
is counted as a fresh execution.

## Executed checks

All **296 distinct Rust test cases** below were freshly executed, with zero
failed and zero ignored. The five new cases are excluded from the later general
durable-auth group so they are not counted twice.

| Group | Passed | Seconds |
| --- | ---: | ---: |
| `actual-secret-library` | 65 | 0.55 |
| `actual-auth-record-snapshot-cases` | 5 | 6.55 |
| `actual-prefix-format-sources` | 8 | 6.73 |
| `actual-composition-pg_auth_product_services--manual--tests--` | 9 | 16.68 |
| `actual-composition-manual_token_flow--tests--` | 3 | 4.63 |
| `actual-composition-pg_auth_product_services--refresh_tests--` | 18 | 32.93 |
| `actual-composition-oauth_provider_client--tests--` | 13 | 9.18 |
| `actual-composition-product_auth_durable--tests--` | 48 | 7.59 |
| `actual-composition-native_secret_custody_tests--` | 9 | 16.56 |
| `actual-composition-interceptor_config_service--tests--` | 10 | 0.01 |
| `actual-composition-native_global_runtime_retains_one_root_across_match_and_no_match` | 1 | 71.54 |
| `actual-composition-native_global_monty_live_model_selection_retains_inflight_exchange` | 1 | 36.00 |
| `actual-public-manual_tokens` | 10 | 0.26 |
| `actual-public-webui_v2_product_auth` | 42 | 30.50 |
| `actual-public-webui_v2_product_auth_4201` | 41 | 0.45 |
| `actual-public-auth_callbacks` | 7 | 0.26 |
| `actual-public-auth_lifecycle` | 5 | 0.21 |
| `actual-current-reborn-dependency-boundaries` | 1 | 0.04 |

Strict Root/Composition/Secrets with test targets:72.71s; PostgreSQL without
RootLLM:15.27s; default Composition:14.27s. Each uses `-D warnings`. Workspace
formatting4.57s and authored whitespace0.21s passed.

The five new cases execute actual Memory and independent PostgreSQL-backed
AuthProduct owners. Controlled scheduling pauses the actual public writer before
its backend CAS while the other owner revokes/cancels, deletes and recreates the
real entry with its reused version. The stale account update and callback admission
must conflict and retain that successor. LocalFilesystem proves unsupported strong
CAS does not retry weakly. Memory and PostgreSQL legacy cases stop at actual account/
final-flow writes, then load historical missing-checksum JSON through a real strong
CAS: exact account receipts finish without account rewrite; missing receipts keep
both original decision and account unchanged with an explicit conflict.

Existing native manual nine, refresh/callback and their lost acknowledgement/
rejection cases, public HTTP/WebUI, secret owners and two global Monty callers
pass on this source. The Prefix owner’s seven unit plus actual native retained
model-source caller are rerun, without a model call, template execution or server
configuration change.

## Preserved failure

Run1 remains FAILED at strict lint on two imports made unused by the migration.
Flow/Cleanup imports were removed, without an allow/suppression or behavior change.
Run1 executed no runtime test. Run2 reran every required check and does not reuse
that failed run’s results.

## Remaining scope

This repairs the write-snapshot prerequisite in [the existing K3 step2 owner](../plans/simplified-v3-implementation.md).
It does not yet migrate full FS/generic raw-input/automatic-selection/result
publication, the generic facade’s latest-account fallback, original browser/setup
identity or explicit reference-only continuation. FS manual cleanup on uncertain
writes, selected interaction deletion/cancellation, issuer expiry/final-use grants,
DCR disposition, remaining protocol/cap/policy consumers and K4–K7 remain open.

Historical unproven callback decisions are retained rather than automatically
published with weak CAS. The complete controlled-upgrade/drain/recovery gate must
account for those decisions before full completion. There is no full-plan,
all-platform, deployment, release or accepted Prefix-design/serving claim.

## Exact changed inputs

| File | SHA-256 |
| --- | --- |
| `crates/brassclaw_reborn_composition/src/product_auth_durable.rs` | `50e7610235013bd8fe13ea096dd822c562b2e76c3bd485cbae15810e85444e77` |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/accounts.rs` | `9d07dd6398ccb31ce9a77dd2f0126c358edc2950c4562635a53c6d8ed6b402d1` |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/interactions.rs` | `1088bd3578f5ae211c7a5f1e718b058ffc1d3a9cb0df9ed1ae5cf94f267ccbd0` |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/flows.rs` | `f7637da584689028bf4cc54bcb94ed7a2f988c8d1deb88260795ea247cbe87a2` |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/cleanup.rs` | `3721cfd9457dcd32202dc3f0f9e00ddaa66dced38a1cef0ea74448f2e3fbcade` |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/refresh.rs` | `40b9b01b4f5b7792273ff9130122ed2d8cf25122a1d7b495357747b3b15c52d7` |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/callback.rs` | `2e37a886a04fa4f70ed9e04a98e2eeecbad798d2fe8e6743b53de948205e195a` |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/tests.rs` | `22ca83bc47dd4c654b1d2c0b60a22d3cbe4ebed4e4c03510b4a5e0b58f7fca4d` |
| `crates/brassclaw_reborn_composition/src/auth_flow_custody.rs` | `d978ffcb9deb1afeacef60ae2bf191456f57bc96949472fc2fd32952d7c672b3` |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/paths.rs` | `c88e5db06e3d242c80c50c80acbdfe11e4c48a8d451d34b1191ece9b91c17d73` |
| `crates/brassclaw_reborn_composition/src/product_auth_durable/record_snapshot_tests.rs` | `dffc3c26a5561abcd9f21d18cbc8fa51eed017ec38896c5b0322eff26314a3c0` |
| `docs/reborn/contracts/auth-product.md` | `36c3f9e1428e8121bc088ebdfc171022b24a2f74790c7e012a1c0733972220b5` |
| `CHANGELOG.md` | `45d6f58e128aaf706c85bc2b964fc8f4cd7eaff3b688cb9563f9afb9f841aecf` |
