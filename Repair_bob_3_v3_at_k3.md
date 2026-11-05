# Repair Bob 3 — Codex and scheduling integration at K3

Date: 2026-11-03. Status: **implementation plan; no login, refresh or recovery is accepted here**.
Reviewed committed baseline: `958cbb9c86d50644c59fcd40ced9c5a7cafc9cdc`.

## 1. Scope, architecture and ownership

Connect native Codex to the concretely composed Auth/HTTP owners, finish its
host-managed refresh path, and replace lossy Q1 kicks with durable original-work
scheduling. Consolidate the duplicated global driver lifecycle and restore
portable Cargo configuration. These are infrastructure/protocol repairs, not
new Rust workflow Tools, a second approval path or another Monty orchestrator.

Read [AGENTS.md](AGENTS.md), [development policy](docs/development-policy.md),
the four component guides, the applicable Auth/Secrets/Host/Composition crate
guides and [simplified shared admission/recovery](simplified_v3.md#gemeinsamer-admission--und-recoveryvertrag-für-produktaufträge).
Use the existing [implementation status](docs/implementation-status.md) and
original BUILD-036/validator records for scope; a staged API or successful SDK
unit test does not prove production factory attachment.

| Responsibility | Sole repair owner |
| --- | --- |
| Exact review subject, rule meaning, sealed verdict/evidence and approval | [Plan 1](Repair_bob_1_v3_at_k3.md), B1 |
| Qualified graph selection and approval/manifest readers | [Plan 2](Repair_bob_2_v3_at_k3.md), B2 |
| Codex native protocol/auth integration, durable original-work execution and common lifecycle | This plan, B3 |

B3 schedules the exact B1 review request and selected B2 graph. It does not
invent review subjects, choose latest submissions, infer approval from completion
or interpret check results. Reuse the existing durable review/turn/task owners;
do not introduce a validator-specific scheduler, sweep loop, job broker or
parallel queue. A wake is an optimization, not the durable source of work.

The review's missing native owner/refresh integration is owned by B3-02 through
B3-05; lossy scheduling by B3-06/B3-07; duplicated lifecycle by B3-08; host-specific
Cargo configuration by B3-10. B1 owns candidate/verdict correctness and B2 owns
approval/selection carriage. These mappings do not authorize parallel versions
of the same function, table, queue or interface.

All steps obey B1's mandatory implementation/evidence quality rule: no stub,
successful no-op, fake provider backend, simulated native receipt or new hidden
resource number. Configure necessary operational limits through existing
acknowledged DB/API/WebUI settings, with units, user explanation and effective
uptake; honor protocol-defined/provider-returned bounds in their protocol owner.
Keep supported deterministic validation model-free; shell/subagent Recipes remain
Tier 1. Infrastructure protocol mechanics do not become a new agent workflow.

## 2. Reconcile the current implementation and actual interfaces

- [x] **B3-01 — Establish current owners and feature matrix before edits.**

**Completed source reconciliation — 2026-11-03:** see the
[current owner map](docs/validation/2026-11-03-bob-committed-reconciliation.md).
Postgres/skills-db private admission now uses the correct thread namespace;
native Codex remains tied to actual root-provider/PG composition. Generic
injected/non-PG Auth remains explicitly unsupported for native Codex. Existing
source and native evidence are reused only within their recorded scopes.

Record current HEAD, affected before-hashes, current uncommitted diffs and valid
evidence. The reviewed tree already had edits to bootstrap/internal admission,
driver/runtime and validator tests. Reconcile these with their author/current
owner; preserve the changes and avoid recreating them from an older checkpoint.
Use B1/B2 and the scheduling contract below as the active implementation steps.
`missing4_v3_at_k3.md` is a superseded redirect/history reference, not a separate
scheduling or validator implementation checklist.

Trace these actual source paths and every relevant caller/trait implementation:

- composition `{factory,auth,auth_native_codex,provider_codex_login,provider_codex_credentials,pg_auth_codex_access,pg_auth_codex_publication}.rs`;
- composition `{pg_auth_device,pg_auth_refresh,auth_flow_custody,oauth_provider_client,llm_key_store,llm_config_service,llm_catalog,llm_reload,provider_mutation,runtime}.rs`;
- composition `product_auth_durable/refresh.rs` and the composed account/provider
  wrapper, not just the deliberately non-executing raw PG account backend;
- `crates/brassclaw_auth/src/{device,refresh,domain}.rs`;
- `crates/brassclaw_llm/src/{codex_host_session,token_refreshing,openai_codex_provider,openai_codex_session,lib}.rs`;
- `crates/brassclaw_host_runtime/src/egress/{original_custody,pipeline}.rs`, its
  injected product Auth/HTTP ports and `crates/brassclaw_secrets/src/pg_store.rs`;
- composition `{validation_queue,validation_review,pg_python_code_store,validator_task_driver,global_monty_driver,bootstrap_validation,retention_sweep}.rs` and actual shared durable task/turn claim/outbox implementations;
- WebUI Codex login hook, provider-login/mutation helpers, API descriptors and
  existing Rust/JS tests; V199 and actual subsequent auth/review migrations.

Identify actual PostgreSQL production, injected-port production, test, local
filesystem and feature-disabled paths. Record which require native Codex support
and which deliberately return typed unsupported/unavailable errors. No backend
silently switches to file/env credentials in host-managed mode.

**Exit:** caller-to-concrete-owner map, lock order and transaction/egress boundary
map. Confirm actual current function signatures instead of using compilation to
discover them. The names of new internal helpers below are design descriptions,
not claims that those APIs already exist.

### Existing provider connectors are the starting point

The source already has `ProviderRegistry`/`ProviderDefinition` and `LlmProvider`
for model communication, plus `AuthProviderClient`, `CredentialAccountService`,
`ProviderBackedCredentialAccountService` and the product Auth facade for credential
lifecycle. Trace these actual traits/implementations in `brassclaw_llm` and
`brassclaw_auth` before extending Codex. Device authorization, callback exchange
and host-managed refresh have different protocol inputs but share existing
credential/secret/journal/egress owners. Do not create another Codex account store,
refresh state machine, provider registry, HTTP client stack or public connector API.
Extend a genuinely missing operation in the existing owning port only after
checking all actual implementations and consumers. Keep issuer-specific parsing
and device protocol adaptation narrow; no workflow or credential authority goes
into Recipes, model prompts or a route-local helper.

## 3. Attach concrete Auth and transport once

- [ ] **B3-02 — Wire native Codex owners in the production factory.**

**Scoped implementation/acceptance — 2026-11-03:** A-01 is integrated through
`a63100515`. The existing PostgreSQL Auth Arc and composed host ports supply the
native owner. Explicit/default tenant and injected-facade cases passed. Default
custody now has an atomic exact original import, complete conflict inventory,
V204/V205 writer fences and a reachable exclusive filesystem recovery owner
while supported originals remain pending. Fixed-snapshot and strict-JSON tests,
original native refresh cases and affected strict lint passed; see
[the exact current receipts](docs/validation/2026-11-03-bob-repair-progress.md)
and [Auth follow-ups](docs/validation/2026-11-03-b3-auth-followups.md).
Do not repeat or bypass this migration. Actual issuer login, full product
projections and remaining feature/platform matrix still need their named gates;
local endpoint/source acceptance does not close all of B3-02 through B3-05.

1. Keep the existing `Arc<PgAuthProductServices>` when composing durable product
   auth. Attach that exact Arc and the actual injected
   `ProductAuthProviderRuntimePorts` inside the existing composition boundary
   before exposing product Auth or starting login/model consumers. The current
   private `with_native_codex_owners` is an attachment seam to reconcile, not a
   mandate for a second Codex authentication framework. Reuse the existing
   shared connector/facade and expose only its minimum required host capability.
2. Reuse the existing PG secret owner, durable device/refresh records and host
   HTTP egress service. Do not reconstruct stores through downcasts, use a
   route-local HTTP client or create another pool/account service.
3. Trace the injected `product_auth_ports` branch separately. If that branch is
   production-capable, extend its existing composition input to carry concrete
   host-native owners or an equivalent sealed supported port explicitly. A
   generic trait object cannot be assumed to contain a PG owner. Update all
   constructors/callers under their real feature gates.
4. Attach at most once and reject conflicting ownership; preserve `Arc` sharing
   through later builder calls/settings attachment. A comment saying no-op must
   match the actual behavior. Required production support missing at composition
   is a visible typed readiness/configuration error, not silent `.ok()` loss.
   Scope missing Codex account/support to the actual requested or required active
   Codex role. OpenAI API-key, Anthropic, Gemini and other configured provider
   paths must not require Codex login/refresh or fail merely because Codex is
   unconfigured. Provider capability readiness and a selected account's readiness
   are separate; no dummy credentials or network probe on unrelated startup.
5. Local/non-PG configurations keep their explicitly unsupported native path.
   Never make a fake PG owner or fall back to local Codex files to make them pass.
6. Remove obsolete module-level dead-code suppressions when the APIs are actually
   used. Fix remaining unused staged paths through real wiring, appropriate
   feature scope or removal of unused speculative interfaces; do not add warning
   suppressions or rename unused owners to imply completion.

**Exit:** the real production factory's `native_codex_owner()` and native
transport work; a test that manually calls the builder does not establish this.

- [ ] **B3-03 — Thread the same owner through every model/configuration consumer.**

Inventory startup construction, reload/reconnect, runtime active-provider
selection, ordinary Kohai, Sempai/preparation and selected task/child providers.
Each consumes the original captured provider/account/head selection through the
same concrete owner and final request guard. Keep actual protocol/endpoint/model
and accepted pair-design/prefix identities; never substitute current defaults.
Keep concrete PG custody inside its owning composition/Auth adapter. Pass the
existing provider/session preparation capability to consumers rather than teach
each model constructor to build or independently configure a native PG owner.
If a trait operation is genuinely missing, extend that shared port once; do not
invent a parallel connector or a new universal service merely to avoid a builder.

In `llm_catalog`, construct `NativeCodexSourceSupplier` only for the applicable
Codex provider and exact verified head. Required missing native owner/head is an
explicit pre-submission error. Correct the misleading comment that suggests the
SDK will ordinarily fall back: host-managed `create_openai_codex_provider` must
continue rejecting an absent source. Standalone file-backed SDK support retains
its separate explicit scope and is not imported into BrassClaw production.

Verify `ProviderDispatchGuard` and `NativeCodexAccessGuard` compose rather than
replace one another. Recheck original provider revision, head/login lineage,
account/access generation, endpoint, expiry, cancellation and actual final
request immediately before network submission. Missing/conflicting mappings
fail closed. Credentials/secret handles/claim tokens never enter Recipe state,
prompt text, DTO/debug output or normal logs.

**Exit:** factory-created providers work consistently on startup and reload, and
all relevant roles select the same original native source without fallback.

## 4. Finish original device-login and refresh custody

- [ ] **B3-04 — Accept device login through the actual product path.**

1. Enter through the existing authenticated setup/provider mutation facade.
   Reuse original mutation/flow/invocation IDs, captured provider request and
   previous selection. Validate expected provider/account/settings revisions.
2. Reuse `prepare_native_device`, original encrypted client/request custody,
   `run_native_device_once`, return retention and native ready/finish operations.
   Preserve the original OAuth client/issuer/endpoints and PKCE/device lineage;
   do not reread edited current setup to complete an old flow.
3. Durably claim original execution before HTTP; no PG transaction/row lock is
   held across provider transport. The host egress custodian authenticates the
   exact request and retains the unsanitized original encrypted response once.
   Public projections remain sanitized. Enforce egress/secrets/size/time policy.
4. Device authorization pending/slow-down/expiry/denial and user cancellation use
   the real provider protocol and persisted schedule. No new fixed polling quota
   or invented task deadline; reuse configurable supported time/size policies.
   Distinguish legitimate protocol polling from replay of a completed exchange:
   preserve the one flow, persist each actual poll invocation/count/outcome and
   next eligible poll time, honor provider interval/slow-down/expiry, and admit
   only the original next eligible operation. Multiple required pending polls
   are not new login flows or SDK model retries. Any protocol default belongs
   to its documented existing protocol contract, not a guessed timer here.
5. Preserve raw received originals before parsing/projecting them. Lost response,
   crash or uncertain acknowledgement does not mean the provider was never
   contacted. Reconcile retained originals/original IDs; do not restart login
   or token exchange automatically with new IDs.
6. `publish_codex_account_and_head_in_transaction` commits verified encrypted
   credentials/account, provider head and original mutation/role outcome through
   the existing CAS participants. Re-read exact original state after unknown
   commit. A later different login/selection cannot be overwritten by old flow.
7. Honor cancellation and stale attempts before each new transport/publication.
   Already received private originals may still need durable custody; that does
   not authorize late role publication or UI success for the cancelled request.

**Exit:** actual factory -> HTTP/facade -> native login -> verified publication
-> provider selection works, with pending/error/recovery state visible and no
credential exposure. Native acceptance uses the actual host
egress/journal/secret/PG path and a real
isolated protocol-test HTTP endpoint for controlled error inputs. That endpoint
does not substitute for required actual-issuer login/refresh/provider compatibility
acceptance through the production factory; record the two scopes separately.

- [ ] **B3-05 — Add host-managed refresh without replaying model requests.**

The current supplier calls `take_codex_access_session`, which rejects expired
accounts and never refreshes. Extend the existing native Auth owner and journal;
do not use the SDK's file refresh or a second credential backend.

1. Before constructing a fresh per-request token snapshot, authenticate the
   original expected provider/head/login/account and stored OAuth client. Check
   actual expiry using existing supported refresh margin/settings, with checked
   time arithmetic. Unknown expiry is not permission to invent an arbitrary
   lifetime; invalid/revoked/missing-refresh states have explicit recovery errors.
2. Use the existing `CredentialRefreshAction` host-instance path and
   `begin_host_instance_refresh_in_transaction`. Ordinary extension/user refresh
   authority must not be widened to refresh System-owned Codex accounts.
3. Construct the refresh original from actual authenticated account/secret
   generations and original client/issuer. Compare-and-claim under the existing
   account/journal lock order so concurrent same-generation requests share one
   durable refresh action. A process-local mutex alone is insufficient.
   Bind the original action to the expected head/login, actual account/access/
   refresh generations and captured client. Concurrent callers with different
   request IDs must discover the same admitted action for that exact generation
   before egress. A later genuinely new access/refresh generation can have a
   new original action; do not key all future refreshes only by an unrotated
   refresh token or by a caller-generated random ID.
4. Commit refresh intent/claim before provider egress. Release database locks
   before awaiting HTTP. Reuse the authenticated original-custody HTTP port;
   persist actual request and encrypted original response with the claim before
   deriving a sanitized result. No refresh token in public records or logs.
5. Distinguish confirmed no-send, actual provider return, invalid grant, expiry,
   cancellation, stale ownership and unknown completion. A timeout/lost response
   does not permit another token rotation. Recover retained returned bytes or
   preserve unresolved state; never guess the old refresh token remains valid.
6. Validate exact client/issuer/account/claim on completion, parse actual response
   strictly and publish access/refresh material plus expiry atomically with the
   journal outcome using existing secret CAS operations. If no new refresh token
   is returned, preserve the original refresh credential only when the actual
   supported issuer contract permits it. Never replace it with missing/null data.
7. Reconcile through existing `reconcile_refresh_action_in_transaction` and
   account/head publication owners. A different active login/head wins over an
   old refresh result; retain that old response privately but do not install it
   into the new account. Same-head access rotation preserves original login
   lineage and creates a new immutable access generation.
8. After confirmed successful publication, take a fresh access snapshot and its
   guard from the original expected head. Do not mutate an already selected
   per-request token or guard to point to a new generation. Concurrent consumers
   wait on/read back the original refresh outcome under existing bounded policy.
   Preserve existing account/client/secret generation checks on every wait and
   final send. A waiter timing out or cancelling cannot erase native custody or
   make another caller repeat the same uncertain issuer exchange.
9. Refresh failure is a pre-submission model error. The host SDK keeps
   `SingleSubmission`: no provider/model call is made when preparation fails;
   an already submitted model request is never transparently replayed after 401,
   timeout or auth repair. A later independent request may use repaired credentials.
10. Keep desired/effective auth/recovery status accurate. A refresh acknowledgement
    timeout retains original work/custody; it does not declare no effect or free
    resources still owned by native transport.

**Exit:** expired-but-refreshable native credentials work without user relogin,
exactly the original refresh lineage is reconciled, and model submission counters
prove no hidden auth-repair replay.

## 5. Make Q1 execution durable under the existing work owner

- [ ] **B3-06 — Persist original review work atomically with submission.**

1. B1-02 owns the exact submission/subject/candidate/check-set binding. Extend
   the existing review/turn/task admission transaction participant to retain its
   executable work identity, original selected B2 graph and supported status.
   Reuse the existing durable queue/work mechanism; do not add a validator-only
   broker, independently authoritative table family or Rust workflow scheduler.
2. Inventory the current pending queue, immutable submission, admitted run and
   outbox/wake schema before choosing columns or a linked record. If necessary,
   implement the complete necessary successor metadata in that same owner. Specify exact
   foreign keys/uniqueness/CAS semantics, format and migration together.
   Name the actual executing shared owner and its current native caller, not an
   assumed generic scheduler. The existing post-turn review delivery loop and
   V129 work semantics are domain-specific; do not insert validator requests
   into completed-turn events or infer that its polling loop is a generic Q1
   service. Extend missing common admission/claim/wake/reconciliation mechanics
   in the Simplified owner, proving genuinely same-contract consumers such as
   applicable private Prefix and validator work before sharing a new durable API.
   Distinct domain subjects/outcomes remain owned by their existing stores.
3. Retain original request ID, exact submission/checksum, selected validator/
   approval/interpreter references, immutable input, admission/run linkage,
   claim/attempt/generation fencing and outcome/settlement references. Distinguish
   durable work identity from replaceable worker lease and from Tool authority.
   Replace production use of `bootstrap_validation::prepare_admission` that
   currently creates a random conversation and synthetic inbound user message.
   Capture private typed work/result completion through the existing shared
   internal admission owner. Extend its actual typed input/continuation contract
   where necessary, with original provenance/selection and strict readers; do
   not fabricate a public chat, matchable command, reply or history to gain entry.
   Controlled installation chat fixtures retain only their actual isolated test
   scope and cannot become the production review scheduler.
4. Both `ValidationQueueStore::submit` and
   `PgPythonCodeStore::create_and_submit` use the same transaction-aware helper.
   All applicable authoring/upgrade/automatic callers go through it. A committed
   new request cannot exist only as a detached Rust future.
5. Commit candidate/submission/work/wake obligation coherently before returning
   success. If persistence owners differ, implement explicit original-ID
   prepare/readback reconciliation before effects, not best-effort dual writes.
6. Repeated same request with identical bytes resolves the original work; changed
   candidate/selection under the same ID is a conflict. Existing queue uniqueness
   still prevents two active proposed edits for the same supported component.
7. New work captures acknowledged effective preparation settings through B1/B2,
   not `MontyExecutionLimits::default()`. Store actual policy revision/limits
   needed for restoration. Live runtime settings keep their existing independent
   revision/acknowledgement contract and do not rewrite selected code/schema.
8. Define legacy rows conservatively. Bind a pending proposal only if original
   exact content and provably unstarted ownership can be reconstructed under
   CAS. Started/ambiguous rows retain unresolved status and evidence; no automatic
   new run, latest submission attachment, model call or behavior replay.

**Exit / B3-WORK contract:** B1's original request and B2's selected graph are
durably associated with one ordinary internal work/admission identity before
effects. B3 invokes B1 with that binding; B1 remains the sole verdict owner.

- [ ] **B3-07 — Replace detached kicks with durable dispatch and reconciliation.**

1. `fire_q1_kick_if_wired` becomes a notification to the existing shared durable
   work owner. Remove `tokio::spawn` that discards `run_q1_validation_retained`'s
   result. The durable request, not an in-memory wake, determines pending work.
2. Reuse the existing instance work discovery/reconciliation on startup/restart
   and durable notifications. Do not restore `spawn_q1_validation_sweep`, poll
   every validator row in a new loop or create another OnceLock service owner.
3. Claim only ready, unbegun original work through the existing transactional
   claim/admission boundary. Concurrent executors must obtain one winning claim.
   Preserve the original selection, submission, attempt lineage and counters.
4. Run via `ValidatorTaskDriver` and the already running global Monty. Queue wait
   and provider/database waits do not consume executing-VM task time incorrectly;
   duration never limits the global service lifetime.
5. Handle all results explicitly: queued/not-ready/unsupported, running, negative
   verdict, completed-pass, cancelled/stale, infrastructure failure and unresolved
   settlement. Reuse/extend existing projection vocabulary deliberately. No
   swallowed `Result`, fake Passed or permanent invisible pending state.
6. Store B1's exact verdict/evidence and original run status through the existing
   owner. Scheduling never promotes the validation queue from completion alone.
   Receipt verification inside `GlobalRecipePorts::finish` retains authenticated
   observations only. After terminal native delivery and required settlement
   acknowledgement, pass its actual sealed proof to B1-08 to commit the final
   verdict/graduation. Do not await that terminal receipt from its own finish
   callback, or let provisional `pass` release custody/approve a component.
7. A dropped waiter, lost wake, missing bridge or service restart does not delete
   work or make it successful. Surface actionable readiness/deferred/error state;
   replay status/projection from durable records without resubmitting effects.
8. Resume only an original supported admitted run/continuation or reconcile its
   retained terminal result. A claimed task with unknown execution is not
   provably unstarted. Keep stop-only/one-attempt Tool rules; no whole-Recipe
   replay, reset counters, auto recovery VM or hidden Tier-2/Rust fallback.
9. Do not invent general crash replay or dynamic repeated occurrences here.
   Implement and accept any actually missing original-work claim/admission/
   reconciliation support in the existing owner as part of this repair; its
   existence is not established by this plan's prose. Only a proven supported
   continuation may resume started work. Otherwise preserve explicit unresolved
   custody and its inspection path without claiming automatic recovery support.
10. Reconcile committed pending work at startup and after notification/listener
    loss using the shared durable owner. A missed in-memory wake cannot strand
    committed work. Use existing configured reconciliation/backoff/capacity and
    fairness controls, not a tight polling loop or new Q1-only timer. Surface
    database/admission overload distinctly from ownership loss, and boundedly
    stop/drain under the instance lifecycle before PG shutdown.

**Exit:** committed review work survives process/wake loss and has visible
original-state reconciliation, with no duplicate native validation effects.

## 6. Consolidate the global driver lifecycle

- [ ] **B3-08 — Share drive logic and preserve admission provenance.**

**Implemented boundary, validation scope:** ordinary/admitted work shares
`GlobalMontyDriver::drive_host` (17 selected native cases passed in the first
repair batch). Completed internal work now returns an opaque proof only after
the actual receipt and durable factory acknowledgement; Q1 consumes that proof
instead of ResultOnly alone. The second batch compiled; one newly added decoy
fixture needed correction before its positive integration case could execute.
That corrected positive/foreign-result case, exact verdicts and all six Q1
callers have since passed in the latest scoped acceptance, including incomplete
execution classification and affected strict lint. Preserve the earlier failed
receipt as history; it is no longer a pending fixture repair.
Private typed non-chat provenance and complete durable work/recovery gates
remain open, so the full step is not completed.

**Already implemented, scoped:** `0f1236a6c` supplies the admission-pool builder
and its runtime attachment, fixing private review thread lookup. At that earlier
baseline the two drive methods duplicated lifecycle logic. The later shared
`drive_host` completed that extraction; non-chat provenance and durable recovery
remain open.

1. **Completed, scoped:** retain the shared private `drive_host` operating on
   the already constructed `Arc<MontyTaskHost>` and original completion contract.
   Thin wrappers construct/accept the host only. Reconcile later changes against
   this owner rather than extracting a second implementation.
2. Preserve original conversation/message/turn/run/attempt, accepted input and
   history. Internal admission may use a different original data namespace;
   resolve it through the actual admitted thread-store owner/injected resolver.
   Do not read an unrelated default tenant, fabricate history, or mint new IDs.
   A namespace selection is data custody, not a new user/tenant permission grant.
   Preserve chat IDs/history only for real chat-origin work. Private review work
   carries its own original job/run/attempt and typed subject/selection input;
   extend the common admission/input discriminator if its current shape assumes
   a user message. Empty/dummy message IDs or a `validator-review:` string are
   not trusted provenance. Share lifecycle/host authority while keeping these
   two valid ingress data contracts explicit and strictly authenticated.
3. Avoid building another store/backend inside the drive loop if the existing
   admission composition can supply its exact thread/input owner. If a missing
   resolver port is required, add it once to the existing composition boundary,
   validate provenance and update all constructor/feature callers together.
4. Keep one implementation of registration, retained input loading, preparation,
   submission, cancellation, native receipt wait, bounded acknowledgement,
   settlement and registry cleanup. Ordinary and internal modes share this code.
5. Ordinary FinalReply requires an actually published correlated reply. Internal
   ResultOnly requires its qualified source and exact verified result reference,
   no assistant reply/history duplication and no withheld successful outcome.
   The completion contract is captured at admission, not inferred from a string.
6. Fence the exact attempt and request cancellation before awaiting policy or
   transport. Capture acknowledged cancellation policy once per accepted wait;
   later settings changes cannot shorten its already accepted absolute deadline.
7. Distinguish pre-submission failure, known not-submitted, submitted/settled,
   cancelled, transport failure and unknown outcome. Do not release or remove an
   attempt until its real host/receipt/native-accounting owner permits it.
   Conversely release completed results/state once custody is transferred and
   no owner needs them. RAII guard drop alone does not prove settlement.
   Native terminal receipt/accounting proof must be available to B1 before
   final Q1 graduation, without a receipt/finish wait cycle. Transfer that
   immutable proof through the existing completion/retention owner; retain
   custody until its actual acknowledgement permits release. Do not retain
   every completed mutable task indefinitely as a substitute for proof transfer.
8. Every early error/return must leave the original host either safely finalized
   or retained for explicit reconciliation. Preserve original receipts/late
   answers and withheld effects without publishing stale replies or replaying.
9. Shutdown stops admission/producers, boundedly drains/cancels with original
   custody, stops/joins the single worker, then releases PG last. Neither new
   wrapper starts a VM. Test failures/cancellation on both entries.

**Exit:** one driver lifecycle implementation, preserved internal/ordinary
semantics, and actual retention/cleanup behavior accepted through native tests.

## 7. Finish the product projections and portable build configuration

- [ ] **B3-09 — Connect WebUI/CLI/API status without leaking custody.**

**Partial implementation — 2026-11-03:** the existing provider-mutation reader
and reconciler now accept the login observer's abort/current-context fence and
its exact receipt validator. They check those before dispatch and after the
response, before changing pending state or emitting settings/bootstrap events.
Codex passes its captured auth/generation fence, AbortSignal and provider/role/
model receipt validator; NEAR passes its original-operation/provider validator
and current-observer fence. Foreign receipts and abandoned observers cannot
publish bootstrap navigation before the login caller rejects them. This adds
no auth store, scheduler, Recipe or provider transport.

Focused JavaScript acceptance covers 54 distinct passing cases in the existing
login, hook, mutation and bootstrap suites, including the new delayed-response,
auth-switch, abort and exact-receipt regressions. See the
[scoped receipt](docs/validation/2026-11-03-b3-login-observer-fencing.md).
No Cargo, browser or native issuer/PG acceptance was run for this work package;
updated embedded assets still need the next coordinated native build. All
remaining B3-09 product projections, original account/head/current-selection
acceptance and validator-review integration below remain open.

**Additional observer lifecycle repair — 2026-11-03:** the existing
`ProviderMutationStatus` component now captures the original auth context,
passes its current-observer fence and AbortSignal into the shared reader and
reconciler, and aborts observation on unmount. Late receipts/errors cannot
publish UI state or start a canonical settings refresh after the observer has
been abandoned. Duplicate clicks keep one original request; already committed
native work is not rolled back or resubmitted. Ten focused cases pass, including
the actual component -> shared helper -> apiFetch boundary with the shipped
QueryClient. The tested bytes were integrated after local Cargo finished;
the separate active Modal build keeps its already captured immutable sources.
See the [scoped receipt](docs/validation/2026-11-03-b3-mutation-status-observer.md).
No additional Cargo or native/browser acceptance was run.

**Provider mutation auth-context repair — 2026-11-03:** pending browser
mutations now use the existing origin/auth nonce namespace and preserve their
own context fence throughout POST completion and automatic read/reconciliation.
A changed context cannot reuse another original's ID/body or borrow its current
bearer for automatic recovery. Recognized abandoned contexts retire their
secret-bearing POST bytes while retaining original reference metadata; a new
explicit edit has its own ID/bytes and still uses backend revision/CAS checks.
Explicit reference-only inspection/reconciliation remains available to the
currently authenticated instance operator, including for an older original.
This fence adds no user/role authorization or separate Auth store.

The final repository caller/helper/login/bootstrap/status suite passes all
73 cases. The [scoped source and test receipt](docs/validation/2026-11-03-b3-mutation-auth.md)
retains the actual final output and describes the corrected explicit-inspection
boundary. Integration occurred after local Cargo/test completion, under the
common lock, without Cargo or Modal source sync. Subsequent
[Modal CLI and native packaging acceptance](docs/validation/2026-11-03-b3-modal-cli-build.md)
compiled that source successfully and verified the three changed runtime JS
assets byte-for-byte inside the native arm64 executable. Native startup probes
passed; issuer/PG acceptance and the remaining B3-09 integrations remain open.

Trace existing descriptors/handlers -> `RebornServicesApi` -> composition facade.
Keep native stores/HTTP/secret operations out of handlers and browser code.
Update the Codex login hook and provider-login/mutation helpers so poll/reconnect
retains original mutation/flow IDs, shows actual pending/expired/failed/unresolved
status and never treats a network timeout as permission to restart login.

After success, read back the original mutation/account/head/provider revision
before showing selected/ready state. Account/role changes in another tab cannot
be overwritten by a late old response. Preserve earlier selection on a failed
transaction; cancellation of a later dialog does not silently undo a committed
provider selection. Prefix setup follows the existing Prefix owner, not provider
server configuration edits or a new generation workflow.

Expose durable review queued/running/deferred/failed/unresolved/complete state
through existing status projections, including original review/run references
and safe diagnostics. Operator retries request reconciliation of original work,
not a fresh behavior execution with reset attempts. Do not expose raw tokens,
original auth bodies, secret references or internal claim material.

### Validator catalogue, exact human review and immutable editing — absorbed missing4 Step 12

This is mandatory B3-09 product integration. Approval semantics/writers remain
solely B1-06 and publication/readers solely B2; do not implement them again here.

1. Locate the actual settings component-catalogue/detail/queue panels and their
   shared hooks/API helpers. Starting points are `settings-tabs.js`,
   `pg_settings_listing.rs`, product-workflow review/settings ports and
   `brassclaw_webui_v2` handlers/descriptors. Verify current file paths and class
   support rather than inventing missing panels or duplicating an existing one.
2. Extend existing DTOs/projections once for private `validator-class-N` Recipe
   and associated code detail: stable class/UUID, exact draft revision, actual
   active qualified revision or explicit absence, original submission/checksum,
   selected validator/check-set/approval references, check diagnostics, missing
   required evidence, observed behavior and human-Q2/provenance status.
   Distinguish draft, approved, active and effective states; a source record is
   not activation and a passing Q1 is not human approval.
3. Populate the human review view from B1's prepared immutable subject/evidence,
   not a latest component-row lookup. Component detail and queue review must
   show the same original view/checksum. Label historical/obsolete selections;
   refreshing a page cannot silently swap the subject the operator is reviewing.
4. Route Recipe class-21 and pure PythonCode class-22 prepare/approve/reject
   actions through B1's typed class/profile dispatcher. Keep supported existing
   Skill/code review working. Do not send Recipe/pure IDs to a Skill-only
   association endpoint or let browser-selected class/mode choose approval trust.
5. Preserve pending, failed, incomplete, rejected, superseded and unresolved
   status/feedback as actually modeled by the backend. Disable approve when
   prerequisites are missing and enforce the same checks server-side. Handle
   stale-view/base conflicts by showing the actual new state and requiring a
   newly prepared exact view; never automatically replay an old approval action.
6. Enable supported operator edits of validator Recipes and associated code by
   creating new immutable drafts with observed predecessor/base CAS. Keep the
   approved/active revision read-only. Preserve overrides, pending proposals,
   partner context and old task selections; never expose mutable approved-body
   editing, successful-evidence POST, fake seed mode or approval-by-checkbox.
7. A rejected or incomplete unstarted proposal may be corrected and submitted as
   a new exact subject through the existing authoring route. Retrying a lost
   acknowledgement or reconciling begun work uses original IDs; it is not a new
   behavior execution or reset attempt. Clearly separate these UI actions.
8. Admin catalogue inspection of private validators does not publish matching or
   MCP commands. Preserve `05:validator`/private route exclusion. Editing class
   10 obeys its protected root contract; class 50 remains historical storage.
9. Add facade/descriptor/handler/JS and actual browser acceptance covering both
   class-21 and class-22 full review flows, draft-vs-active display, missing
   evidence, negative Q1, human rejection, immutable edit/base conflict, page
   reconnect, lost acknowledgement and late stale responses. Verify direct API
   calls and model/component-management ports cannot bypass Q2/backend gates.
10. Pair the UI cases with B1's M4-09/M4-10 and B2 publication/retention receipts.
    New approval must affect the next qualified task, while an older waiting
    task keeps its original selection. A browser mock or optimistic status
    update alone cannot establish native review/publication acceptance.

- [x] **B3-10 — Remove the repository's host-specific Cargo target path.**

Delete the committed `/Users/ollama/brassclaw-target` setting from
`.cargo/config.toml`, preserving unrelated cross-compilation/registry/profile
configuration. The existing local Codex configuration supplies the one canonical
target on this machine. Do not replace it with another user's path, target2,
an inline environment assignment, generated committed local file or shell wrapper.

Inspect CI/build scripts for hidden assumptions about this target. Other hosts
use their configured target or Cargo's portable default. Do not edit unrelated
cross-platform linkers without a diagnosed problem. Verify config parsing and
appropriate existing Linux/Windows/macOS CI paths; a local macOS check alone
does not prove those platforms. No build on the test machine. Do not install a
new toolchain or clean caches merely for this documentation/config-only step.

**Completed — 2026-11-03, config-only acceptance:** removed the repository
`[build].target-dir` override. The current shell inherits
`CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` from its existing environment;
no host configuration, alternate target or inline override was created. The edit
and checks ran under `/private/tmp/brassclaw-target-cargo.lock`, without Cargo.
TOML parsing and an exact structural comparison confirm that only the target
override was removed; cross-linker, registry and profile settings are unchanged.
Executable build/CI configuration inspection found no dependency on the removed
absolute path. The existing worker workflow discovers `target_directory` through
Cargo metadata; existing Linux, Windows and macOS jobs retain their own configured
or default targets. Whitespace checks pass. This accepts the portable config
repair, not execution of a new cross-platform build/test matrix. Other agents'
source, runtime diagnostics, tests and receipts were not modified.

## 8. Connected regression, integration and interruption acceptance

- [ ] **B3-11 — Add tests through the real factory and native owners.**

| Scenario | Required observation |
| --- | --- |
| Actual PostgreSQL production factory and injected-port branch | Native owner/transport attached once; no builder-only success claim |
| Unsupported local/features branch | Typed unavailable/unsupported result; no file/env fallback |
| Native device pending/slow-down/denial/expiry/success | Original flow, request and encrypted return custody; correct UI/status |
| Lost login response/commit/HTTP acknowledgement | Same original IDs/readback; no repeated exchange or duplicate selection |
| Cancelled or superseded login before/after return | Private custody retained as needed; no stale role/account publication |
| Startup/reload/Kohai/Sempai/task/child source construction | Same concrete native owner and original selected account/head/guard |
| Fresh token, expired token with refresh, no refresh credential | Correct fresh snapshot or explicit recovery error before model send |
| Two requests and two processes refreshing the same generation | One winning original refresh action/transport; no rotating-token race |
| Refresh succeeds without a new refresh token | Original retained only under supported issuer contract; no null overwrite |
| Refresh return received but publication/ack lost | Reconcile retained original bytes; no second issuer request |
| Refresh timeout/unknown return or invalid grant | Explicit unresolved/reauthorization result, no model submission/replay |
| Head/account/client/access generation changes before final send | Old immutable guard rejects; no hidden source switch |
| One submitted model call then 401/transport failure | No automatic second model request for auth repair |
| Review saved then process exits before/during wake delivery | Durable original work discoverable, no hidden indefinite pending |
| Queue proposal/submission mismatch or duplicate execution claims | B1 rejects mismatch; one original claim/selection, no extra dispatch |
| Completed negative Q1 versus completed positive Q1 | Actual B1 verdict controls graduation; no completion-only pass |
| Crash before admission versus crash after possible effects | Only provably unbegun work admitted; started unknown work retained |
| Ordinary and admitted driver paths: preparation error, cancellation, receipt/settlement failure | Shared lifecycle preserves correct fencing, bounded ACK and custody |
| Settings changed while older ACK/preparation wait is active | Captured original revision/deadline preserved; new work uses acknowledged settings |
| Completed task with transferred custody | No unnecessary mutable task/result retention; unresolved owners still retained |
| Portable config and host-managed Codex canary credentials | No absolute target dependency and no secrets in logs/events/DTOs |
| Validator catalogue/review/editing product flow | B3-09's ten integration requirements pass for class 21 and 22 with actual native backend state, private-route exclusion and immutable edit/CAS |
| Private review admission repeated after lost acknowledgement | Same durable job/run/selection, no synthetic chat message or new random conversation, no extra validator execution |
| Observation retained before terminal receipt/settlement | No successful Q1 or custody release until actual terminal proof; no finish/receipt deadlock |
| Two concurrent refresh request IDs for one credential generation | Same original admitted refresh; a subsequent actual new access generation remains refreshable without reusing the old action |
| Listener loss, restart and capacity pressure | Committed original work is reconciled under shared configured controls; no new domain scheduler or false ownership failure |
| Codex unconfigured while another provider is selected | Existing API-key/other protocol connector and role startup remain functional; no Codex credential requirement or side effect leaks across providers |

Use existing `pg_auth_device_tests`, `pg_auth_refresh_tests`, provider mutation/
bootstrap/request-policy tests, Codex host-session tests, Q1 retained/queue tests
and relevant JS tests. Add missing real-factory and cross-process cases. A local
mock source cannot close the native composition gate. Controlled inputs use real
isolated resources/HTTP test endpoints through the actual native connector,
egress, stores and guards; never replace those owners with simulated successful
implementations. Actual issuer/provider end-to-end acceptance remains required
for deployed Codex compatibility, with exact source/configuration and no model
replay. Read `LOCAL_TEST_ENV.md`
before any separately authorized remote/provider acceptance; no provider server
file/template change and no remote compilation is required by this plan.

- [ ] **B3-12 — Batch validation and record actual completion.**

Complete each coherent source/consumer/test repair before compiling. Follow the
development policy's two-package-chat/supervisor workflow: isolated package chats
implement and share the serialized, activated Modal builder; the supervisor
reviews, owns native tests/final validation and integrates accepted changes.
Run required long local batches detached in Screen and continue independent work
outside frozen captured inputs. Keep one Cargo execution per build environment
and one canonical local target. Native and Modal builds may run concurrently.
Inherit `CARGO_TARGET_DIR`, invoke Cargo directly, and use no inline override,
explicit `/bin/zsh -lc` wrapper or routine clean.

Validate affected Auth/LLM/Host/Secrets/Composition features and WebUI contracts,
then required real PG/global-worker/ordinary-caller cases. Read actual Cargo
feature names/test filters before running. Compile shared artifacts once and
reuse them for meaningful selected cases. Zero tests, skipped prerequisites,
parser-only checks or a success status from the Screen launcher are not passes.

Capture exact source/patch hashes, commands/features, migrations/effective
settings, application/worker binary hashes, redacted issuer/dispatch counters,
per-step logs, original runner and terminal completion/acceptance receipts under
the development policy. Keep original native encrypted custody in its owner,
not copied into readable test evidence. Fix warnings/deprecations in affected
code; do not suppress them to obtain a green lint result.

Update the three repair plans, existing owner plans/status and changelog only
after actual results. The accepted status record must identify source commits,
test/receipt identities and exact scope: class-21/class-22 human review, qualified
immutable internal validator selection, full B1 M4-01–M4-12 production matrix and
all applicable B2/B3 consumer gates. Mark an inherited case pending when it was
not actually exercised; do not repeat the old plan's blanket closed-source claims.
Superseding `missing4` does not satisfy any of these implementation gates.
The original documentation delivery granted no new release, deployment or
test-machine-wipe authorization. Ongoing implementation, integration and release
work follows the user's applicable standing authorizations and the release
contract; creating these plans was not itself implementation or acceptance.

## 9. Completion and v3.5 boundary

Completion requires working production Codex attachment and refresh, durable
original review work with visible/reconciled outcomes, one shared driver
lifecycle, portable configuration and the native acceptance above. B1 must
prove subject-bound verdict/approval and B2 every applicable selection reader.
No SDK-only, database-only or fake-source result closes the product path.

The existing v3.5 connection/shared-facility work consumes this implementation;
it must not build another auth store, queue, dispatcher, lease owner or execution
loop. Domain workflow decisions remain in Recipes/PythonCode. Dynamic occurrences,
general fatal crash continuation and exactly-once external execution retain their
separate proof obligations; this plan never infers them from durable scheduling.

## 10. Architecture/source audit — 2026-11-03

Audited after Bob 1 and Bob 2 against the ground-truth guides/roadmap and actual
Auth/provider traits, host-instance refresh, bootstrap admission, global finish
and terminal driver paths. Existing shared-source changes are preserved. These
are corrected implementation requirements, not accepted behavior.

| Finding in the earlier plan | Corrected instruction |
| --- | --- |
| Concrete Codex attachment could become a parallel per-provider authentication backbone | B3-01/B3-02/B3-03 start from actual shared provider/Auth ports, keep concrete storage custody private and isolate optional Codex readiness |
| A generic durable executor was assumed to exist; post-turn work has different domain semantics | B3-06/B3-07 name and extend actual shared admission mechanics, require genuine same-contract reuse and reject V129/event/sweep shortcuts |
| Production Q1 would retain random bootstrap conversations/synthetic user messages | B3-06/B3-08 require original private typed admission and a strict ingress discriminator without fake chat/intent/reply effects |
| Final Q1/native settlement could be awaited from its own finish callback | B3-07/B3-08 consume B1's observation-then-settled-verdict protocol and transfer actual terminal custody proof without premature release |
| Refresh request IDs, unchanged refresh tokens and device polling could conflate concurrency with replay | B3-04/B3-05 bind protocol polls and refresh actions to actual original flow/credential generations; no model replay or guessed timer |
| Fixture wording could count a simulated provider as production acceptance | B3-04/B3-11 require the actual native connector/stores/egress and separately scoped real-issuer compatibility evidence |

Use only existing supported technical/resource policies or a complete owner-bound
settings extension; no magic timeout, quota or fallback makes a missing path
complete. At this audit all implementation checkboxes remained open; B3-10's
subsequent config-only acceptance is recorded in that step. The other steps still
require their named native evidence. No Cargo or source implementation was
performed by the architecture/source audit itself.


## 11. Isolated Codex/Auth source repair — 2026-11-03

The explicitly authorized second worktree (`codex/bob3-auth-custody`, baseline
`0f1236a6c`) implements the B3-02 default-tenant custody transfer and explicit
injected-facade native support, B3-03 session-capability propagation and B3-05
original native refresh. It reuses the existing Auth journal, secret CAS and
host egress owners. V204 is reserved for the Auth filesystem cutover; B1's V203
remains separate. Archived filesystem bytes remain unchanged and old writers
are fenced after atomic import. In-flight old originals and incomplete
filesystem publication decisions must reconcile before cutover, without invented
completion. Generic injected/non-PG ports remain explicitly unsupported for Codex.

The native host parser no longer invents a one-hour lifetime when issuer expiry
is absent/zero. Added real-PG/local-HTTP regression sources cover generation
coalescing, retained refresh omission, unknown/invalid returns, original response
reconciliation, default factory attachment and migration preservation/fencing.
No local Cargo or native Rust tests were run in this worktree. SQL-only PostgreSQL
17 fence acceptance passed; compilation/native/issuer gates have their distinct
scope in [the Auth custody receipt](docs/validation/2026-11-03-b3-auth-custody.md).
No implementation checkbox is closed by these source or SQL results. B3-04's
actual human issuer login, full product projections and the B3-11 native matrix
remain required; the existing login/device owner is reused, not replaced.

**Subsequent supervisor acceptance:** the connected A-01 package and follow-ups
were integrated and locally executed: 71 passing run-6 cases plus the two
corrected run-7 fixtures, applicable LLM/Auth cases and strict eight-package lint.
The worktree itself ran no native Cargo. The earlier source/SQL-only observations
above remain historical, not the latest test state. See the active repair record
for exact hashes, failures/corrections and limitations. No real OpenAI issuer,
full product/platform/release or whole Bob-plan completion is inferred.
