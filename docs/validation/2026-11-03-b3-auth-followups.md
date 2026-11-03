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
all five native tables; this is the existing unnumbered internal Auth schema,
not an additional numbered refinery migration or credential backend. Inserts,
updates and deletes under either old/new pending tenant are rejected. On retry,
the migrator removes the guard marker transaction-locally while holding writer
locks. Failure restores it; a still-pending outcome reinstates it before
releasing locks; successful native cutover removes it atomically with publishing
the completed-cutover marker/source writer fence. V204 bytes stay unchanged.

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
  drives the actual PG-backed filesystem claim into `Completing`, recreates the
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
