# Bob 3 Auth follow-ups to 77b31f122

The leading chat requested these changes after reviewing the isolated Auth
commit. No local Cargo or additional paid Modal build is authorized/used for
these follow-ups; the leading chat owns combined native compilation/execution.
These are source changes with pending native acceptance.

## Native identity/index conflict checks

Migration's conflict readback now compares the actual indexed `user_id` as well
as JSON for accounts, flows, interactions and requirement selections. Refresh
readback compares actual `account_id`, nullable `refresh_handle`,
`blocks_refresh` and JSON. Primary-key tenant/id/scope selectors remain exact.
`FOR UPDATE` retains the compared native row through the cutover transaction;
identical JSON cannot hide a conflicting index or overwrite it.

Written actual-PG test (uncompiled/unexecuted here):
`native_auth_migration_rejects_same_json_with_conflicting_native_indexes`.
It independently corrupts all four user indexes and all three refresh indexes
while retaining identical JSON, calls the actual import helpers and requires
conflict with no overwrite. This is SQL identity/index fidelity, not provider
or issuer acceptance. Existing full-import tests remain applicable integration
selectors after recompilation.

## Open originals retain a reachable custody owner

The default-tenant migration now returns a typed custody disposition. An open
callback (`Completing`/`CallbackReceived`), original filesystem publication,
nonterminal DCR registration, unconsumed manual-token publication or submitted
refresh records a pending candidate, without returning early. The migrator
checks every later source record, native identity/index comparison and existing
cross-reference check. It inventories all five native Auth stores before import,
under source/native table writer locks. Pending custody is permitted only when
all five stores were empty for this namespace: even matching pre-existing native
copies plus open filesystem work are a custody conflict, never an allowed
partially hidden native backend. Malformed/conflicting or unavailable records
remain errors. No completed-cutover marker is committed in pending mode.

The transient import is inside a savepoint. A valid pending decision rolls it
back and commits only a persistent native writer fence. The existing internal
Auth `MIGRATIONS`/`run_auth_migrations` owner now attaches the
`brassclaw_product_auth_filesystem_recoveries` tenant marker plus write guards on
all five native tables; these attachments remain in the existing internal Auth
schema. V205 below supplies the shared snapshot-safe custody row/functions. Inserts,
updates and deletes under either old/new pending tenant are rejected. On retry,
the migrator removes the guard marker transaction-locally while holding writer
locks. Failure restores it; a still-pending outcome reinstates it before
releasing locks; successful native cutover removes it atomically with publishing
the completed-cutover marker/source writer fence. The permanent custody row is
switched in that same transaction and never deleted. V203/V204 bytes stay unchanged.

The existing factory exposes only `FilesystemAuthProductServices` in this mode,
using the same scoped PostgreSQL-backed filesystem and existing PostgreSQL
secret owner. It attaches no native Codex owner. The unexposed candidate PG
Auth facade is discarded; there is no live switch or dual credential backend.
The ordinary provider catalogue and existing Auth original/reconciliation
interfaces remain reachable. An explicitly supplied `reborn-cli` tenant also
runs migration, preventing namespace spelling from bypassing the cutover.

Recovery uses the existing `RebornProductAuthServices` facade and its
`AuthFlowManager` original read/reconcile/cancel operations. Existing callback
routes resume the matching original before PKCE/configuration or issuer work;
manual-token continuation accepts the retained interaction/flow/invocation;
refresh repeats use the retained refresh ID. This adds no replacement submission
or guessed result. A submitted original without a confirmed return stays
unconfirmed; this change does not promise to resolve unknown external effects.
After supported reconciliation or explicit terminal disposition, restarting
retries the atomic import and source-writer fence. While originals remain open,
the next start remains in pending custody. It does not retry issuer dispatch.

Safe status is available through `custody_status()` and protected
`GET /api/reborn/product-auth/custody`, using existing bearer, per-caller rate,
origin and `NoBody` enforcement. Startup also reports pending custody to the
operator. The DTO carries only custody mode, native Codex support availability
and whether startup will retry cutover; it exposes no credential IDs/material.
Native support availability is separate from an account being authorized.
Explicit composed-port reinjection retains this same status and owner pair;
generic external ports report external custody without acquiring a native owner.

Additional written tests, **uncompiled/unexecuted here**:

- `pg_factory_pending_auth_custody_exposes_original_recovery_then_retries_native_cutover`
  drives the actual PG-backed filesystem claim into `CallbackReceived` with a
  `Submitted` original outcome, recreates the
  real factory, reads/reconciles the same unconfirmed original, projects status
  through the real Auth router, explicitly cancels, then restarts with the
  explicit default tenant and verifies native cutover retains the original.
  Before the supported pending case, it requires actual explicit-default-tenant
  factory failures for an early open original plus a later native index conflict,
  a genuine native-only account, a genuine native-only flow, and a malformed
  later filesystem account with empty native stores. Original/native
  records stay retained and none of these conflicts grants pending ownership.
  During pending custody it verifies all five native write guards, rejection
  through a stale native facade, zero committed transient imports, and actual
  native write availability only after successful cutover. The later filesystem
  account is also imported, demonstrating that original recovery did not hide it.
  The provider-catalogue assertion proves availability of the unrelated catalogue,
  not an executed model request or live provider acceptance.
- `auth_custody_status_requires_bearer_and_rejects_request_body` drives the actual
  composed WebUI bearer/body middleware. Its in-memory facade is only a status
  projection fixture; it claims no native or provider acceptance.
- `native_default_auth_migration_retains_accounts_flows_interactions_and_fences_old_writers`
  now requires the explicit pending disposition for the genuine submitted FS
  refresh before the existing real terminal/import/source-fence assertions.
- Retain `native_auth_migration_malformed_original_rolls_back_without_cutover_or_source_loss`
  for the fail-closed malformed case.

Supervisor acceptance: compile/run the actual Composition library selectors with
`postgres,skills-db,root-llm-provider`,
then apply affected-package lint requirements. Exact test names in source are
authoritative. No local Cargo, test execution, new Modal submission, source sync
or remote tests were performed for these follow-ups. The earlier Modal receipt
in `2026-11-03-b3-auth-custody.md` applies only to its captured inputs and does
not compile this recovery/status change or these test sources. Direct rustfmt
and `git diff --check` are source checks only.

The supervisor's integration compiler reported the first import test module's
unused `RootFilesystem` import on the original package. That module now removes
it; the separate malformed-source test module retains its actually used trait
import. No warning suppression or build was added for this correction.

Architecture review keeps the existing Auth, secret, host HTTP and composition
owners. This is an installation persistence/lifecycle correction, not a new
Tool or hidden task workflow. Recipe/Skill/ToolSkill relationships, immutable
catalogue/task selections and current kernel policy remain independent. Preserve
original IDs, encrypted material and unknown outcomes; never infer success,
reset attempts, replay an effect or use SDK files/environment sessions as fallback.
No new approval, scheduler, VM or B1/B2/driver implementation is added. The internal
Auth schema extension above is part of this same custody/lifecycle correction.
The current supervisor policy/package board at `57364c351` restricts this chat
to A-01 and its review corrections; native acceptance/integration remain with
the supervisor, and a subsequent package needs its own fresh chat.

## Fixed transaction snapshots and ambiguous source JSON

The supervisor identified two further defects in `fffe781ad`: marker-only
`SELECT EXISTS` fences can miss a later committed marker under a fixed MVCC
snapshot, and decoding source bytes straight into `serde_json::Value` silently
collapses duplicate object names before typed record validation.

Reserved successor V205 creates `brassclaw_product_auth_custody`, with one
permanent custodian row per tenant, and owns both trigger functions. Existing
cutover/recovery states are backfilled; conflicting old markers fail migration.
The common SQL guard inserts an absent initial row with conflict detection,
then reads it `FOR SHARE`. Custody transfer updates that same row under the
existing source/native table writer locks. A fixed snapshot predating its insert
or update must fail serialization (`40001`), rather than interpreting an absent
or obsolete visible marker as permission. An already visible competing custodian
fails with `55000`. Ordinary matching-owner writers share the row lock; they do
not rewrite the row or reset authority. The migrator alone switches the row in
its atomic import/pending transaction. No exception is swallowed and no automatic
retry, guessed outcome or effect replay is added. V204's existing filesystem
trigger identity immediately invokes its V205 successor; new/existing native
attachments invoke the V205 native guard.

The source reader reuses `brassclaw_common::json::read_json` with
`DuplicateKeys::Reject`, including nested objects and decoded names. Its
construction bounds derive from the actual source length (nodes, names, depth
and decoded string bytes cannot require fewer source bytes). No new arbitrary
record-size or request-rate constant is introduced. The previous Value reader
is retained solely as a compatibility admission check for its existing technical
recursion/numeric profile before existing Clone/SQL consumers take the tree;
its normalized result is discarded. Only the strict owner's tree reaches
component decoding/import. Original physical bytes/version remain untouched.
String contents are never recursively decoded as JSON, interpreted as code or
promoted to an executable payload.

Focused native selectors, **test sources only, not compiled or executed here**:

- `pg_auth_product_services::filesystem_migration::tests::native_default_auth_migration_retains_accounts_flows_interactions_and_fences_old_writers`
  now includes both requested actual two-connection cases. Connection A starts
  REPEATABLE READ and fixes its snapshot without DML. B runs the actual pending
  migration and commits; A's new native INSERT must fail with `40001` and remain
  absent. A second fixed snapshot sees the actual unchanged source row and FS
  custodian; B performs the real native cutover and commits; A's later no-op FS
  UPDATE must also fail with `40001`. B does not update the source row, so an
  incidental same-row update cannot supply the asserted serialization failure.
  Existing byte/version, original-journal, retained readback and normal writer
  fence assertions remain in the same case.
- `pg_auth_product_services::filesystem_migration::conflict_tests::native_auth_migration_rejects_duplicate_original_keys_and_retains_exact_bytes`
  writes duplicate account ID, nested scope/user identity and nested flow custody
  keys into real PG filesystem records. It confirms direct typed reader rejection,
  actual importer failure, rollback of earlier imports/ownership markers, and
  exact unchanged physical body/version. Its positive case imports a JSON-looking
  source string containing duplicate names intact as data.

Compile the actual embedded V205 bundle and Composition library with
`postgres,skills-db,root-llm-provider`, run these alongside the earlier factory,
index-conflict and status selectors, and apply affected-package lint requirements.
Direct rustfmt/diff checks are the only new checks performed in this review cycle;
all native execution and final acceptance remain supervisor-owned. Previous
V204-only SQL and Modal receipts do not validate V205 or the new Rust/test inputs.

## Supervisor native failures on the original integrated package

The supervisor's actual local run on `4cebad6b6` completed 69 selected cases:
59 passed, 10 failed. Its authoritative logs/report remain at
`/Volumes/SSDE/brassclaw/target/validation/bob-native-repair-20261103/run-4/native-tests-1/`.
They do not execute the unintegrated follow-up commits or V205 above.

- `test-025` and `test-026` failed before migration: their short fixed strings
  violate `SecretsCrypto::new`'s real master-key contract. Both and the new strict
  source fixture now use `brassclaw_secrets::keychain::generate_master_key_hex`.
  Encryption validation is unchanged; these are genuine PG/encrypted-store tests.
- The five refresh cases `test-027` through `test-031` stop at real native
  account publication. The publication path reauthenticates the returned token
  Original, then incorrectly attempts a runtime lease directly on that Original.
- `test-039` challenge display and `test-043` preparation recovery make the same
  incorrect direct-Original lease. `test-034`'s transport-custody fixture also
  assumes an immutable request Original is directly leasable.

The SecretStore intentionally excludes `Original` from runtime leasing;
`Retained` outputs are the supported exact-source material boundary. The fix
reuses `put_retained_from_original_in_transaction` with a deterministic private
read handle, exact source scope/reference and the same caller transaction, then
leases/consumes that retained output. Preparation/challenge/token callers still
authenticate their exact protocol context first. No SecretStore permission or
lease policy is weakened. Failed transactions retain no new copy, lease or
publication; original identity and bytes stay immutable. Repeated read-only
recovery can reuse the exact retained output without issuing provider I/O.
Token consumption remains inside the existing account/head/provider publication
transaction; replay/admission rules are unchanged.

`test-034` now explicitly requires that the actual request Original cannot be
leased directly, authenticates its persisted protocol context, and reads it
through that same production retained-material helper. Its real HTTP, duplicate
preflight, deferred commit fault and no-repeat assertions remain intact.
The refresh material path already uses a retained response and ordinary refresh
credential correctly; it is not rerouted or exempted from the failed login setup.

Re-run all ten exact failed source selectors from the supervisor's selection
manifest, including all five `device::tests::codex_refresh_tests` cases and:

- `device::tests::native_device_actual_custodian_rejects_changed_request_duplicate_preflight_and_failed_request_commit`
- `device::tests::native_device_http_originals_restart_and_ready_keep_exact_scope_client_expiry_and_raw`
- `device::tests::native_device_original_reads_do_not_wait_for_flow_writer_and_preparation_recovery_is_exact`

The `device::...` names above have prefix `pg_auth_product_services::`. The two
filesystem selectors are already named above. All ten are cause-reviewed here,
**not rerun or claimed repaired by execution**. The supervisor owns the next
combined native compile, relevant lints and real acceptance, including any
downstream failure revealed after these formerly blocking failures.
