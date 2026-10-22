# Simplify connections v3.5 — reuse BrassClaw's built-in infrastructure

> **Current status — 2026-10-22:** use [the combined implementation and ownership
> index](../implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


Date: 2026-10-10. Status: proposed implementation plan; all steps remain open.
Creating this document changes no code, component, schema, configuration or
deployment. “v3.5” names this consolidation effort; it does not supersede the
binding final-v3 contracts.

## 1. Objective and architectural starting point

Make the final-v3 architecture use existing shared connection, authentication,
admission, dispatch, event and recovery facilities wherever callers currently
repeat the same responsibility or apply contradictory standards. Establish one
owner for each shared rule, and thin protocol adapters that delegate to it.
Preserve necessary checks at distinct boundaries and operation-specific evidence.

**Recipe first:** user workflows remain explicit sequences of existing Tool
usages and pure PythonCode, assembled by IBS and executed by the already-running
global Monty service. A workflow that needs outcome inspection or reconciliation
uses explicit reusable steps. Connection management, durable recording, kernel
enforcement and worker supervision remain infrastructure. Do not introduce a
Rust “reliable workflow” Tool or a second executor to perform those sequences.

Read these authorities before implementation:

- [recipe.md](../../recipe.md): workflow assembly, typed bindings and persisted
  `knowledge`/`stepnumber` syntax.
- [skills.md](../../skills.md): preloadable usages, exact-combination approval,
  isolation and the exact failure/retry contract.
- [tools.md](../../tools.md): primitives, retained implementations, live policy,
  durable effects and supervised recovery.
- [toolskills.md](../../toolskills.md): binding metadata and actual support gaps.
- [simplified_v3.md](../../simplified_v3.md), especially §§1.1, 9 and 10 and
  Phases 0a, 3, 3a, 6–8: one operator, live global Tool policy, global Monty,
  DB-only provider definitions and coordinated cutover.
- [Development policy](../development-policy.md): evidence selection and stopping.

Coordinate with the [existing v3 implementation plan](simplified-v3-implementation.md)
and [Sempai v3.5 conversion plan](sempai_v3.5.md). This plan consolidates technical
facilities; the Sempai plan moves domain sequencing into reusable components.
Neither plan establishes that the full v3 architecture is implemented.

## 2. Source inventory and concrete conversion targets

These are source observations, not production acceptance claims. Reinspect at
implementation time: this working tree contains concurrent changes, including
review reconciliation work. Do not overwrite or certify those changes implicitly.

| Area | Existing built-in facility | Conversion instruction and current limitation |
| --- | --- | --- |
| PostgreSQL | [brassclaw_pg pool](../../crates/brassclaw_pg/src/pool.rs), `deadpool_postgres`, `tokio_postgres`, rustls | Make this infrastructure own the common parsed connection/TLS/pool policy. The current builder is not yet sufficient to replace every other builder unchanged. |
| Event-store connections | [Event-store construction](../../crates/brassclaw_reborn_event_store/src/lib.rs) | Prefer its existing `SharedPool` configuration when using the same database. Its standalone path separately parses hosts and enforces TLS: consolidate that policy without weakening it. |
| Adapter authentication/admission | [NativeProductAdapterRunner](../../crates/brassclaw_product_adapters/src/runner.rs) and adapter auth/egress contracts | Delegate common verification, bounded admission and acknowledgement classification; keep each external protocol's signature, payload and response rules. Inventory feature-gated and non-webhook paths before claiming universal coverage. |
| Inbound actions | [Product workflow](../../crates/brassclaw_product_workflow/src/lib.rs), [IdempotencyLedger](../../crates/brassclaw_product_workflow/src/ledger.rs), [durable ledger](../../crates/brassclaw_product_workflow_storage/src/lib.rs) | Reuse binding, staging, deduplication and submission. Inbound delivery deduplication is not proof that downstream Tool effects are deduplicated. |
| Turn ownership | [TurnStateStore](../../crates/brassclaw_turns/src/store.rs) and [Postgres turn store](../../crates/brassclaw_turns/src/pg_turn_state_store.rs) | Reuse supported durable run, claim, cancellation and checkpoint contracts. Inspect actual continuation support; a stored record does not reconstruct a lost VM. |
| Tool policy | [Instance policy](../../crates/brassclaw_authorization/src/instance_policy.rs), [stable Tool policy](../../crates/brassclaw_authorization/src/stable_tool_policy.rs), [retained kernel dispatch](../../crates/brassclaw_host_runtime/src/services/retained_capability.rs) | Wire existing policy services to every actual dispatch. Verify stable UUID/alias registration and live publication; type existence alone does not prove all callers use them. |
| Monty communication | [Transport actor](../../crates/brassclaw_monty_host/src/transport_actor.rs), [global task factory](../../crates/brassclaw_reborn_composition/src/global_task_factory.rs) | Reuse request correlation, receipts, capacity, worker acknowledgement and supervision. Remove caller-local implementations only after equivalent semantics are established. |
| Task admission/effect records | [PgMontyAdmission](../../crates/brassclaw_reborn_composition/src/pg_monty_admission.rs), [invocation retention](../../crates/brassclaw_reborn_composition/src/pg_monty_invocation.rs) | Reuse existing intent/answer and selection retention where compatible. The invocation module describes a stop-only candidate path; it is not a general retry/resume engine. |
| Event reconnect/replay | [brassclaw_events](../../crates/brassclaw_events/src/lib.rs), [Postgres event logs](../../crates/brassclaw_reborn_event_store/src/pg_store.rs) | Use durable cursors and explicit replay gaps rather than adapter-local event histories. Audit/event logs are not authoritative Tool-effect ledgers. |
| HTTP/network transport | [ReqwestNetworkTransport](../../crates/brassclaw_network/src/transport.rs) and [product egress contract](../../crates/brassclaw_product_adapters/src/egress.rs) | Reuse compatible pooled transport and brokered egress. Preserve DNS, destination, secret and response-size enforcement; do not replace specialized protocol behavior blindly. |
| Provider/MCP integration | [MCP provider gateway](../../crates/brassclaw_reborn_composition/src/mcp_provider_gateway.rs) | Reuse ordinary chat ingress and existing provider execution. Retain the specific request-scoped connection and advertisement lifecycle required by v3. |
| Review reconciliation | [Review delivery](../../crates/brassclaw_reborn_composition/src/completed_turn_review/delivery.rs) and [native reconciliation contract](../reborn/post-turn-review-reconciliation.md) | Extract genuinely common custody/accounting primitives only after tracing evidence and release predicates. Unknown provider spend, lost native owners and conflicting records remain unresolved. |

### Confirmed connection-policy divergence

The general Postgres builder classifies local URLs using URL host inspection,
uses `NoTls` for loopback, and supplies a rustls connector for remote connections.
The standalone event-store builder uses parsed Postgres configuration, examines
host lists/addresses and Unix sockets, honors explicit local TLS, rejects remote
`sslmode=disable`, and upgrades remote `Prefer` to `Require`. They also differ in
pool construction and certificate-root fallback.

Consolidation must first give the shared builder an explicit, reviewed contract
covering these cases. Merely routing the event store through today's general
builder could remove protections. Preserve mandatory remote TLS and certificate
validation; explicitly review certificate-root fallback and ambiguous/local-host
classification rather than blindly adopting either implementation.

Other duplicated mechanisms are **audit candidates**, not findings established
by their similar names. An inbound fingerprint, a Tool invocation identity and a
provider billing reservation describe different operations. They may share
mechanics while retaining separate records and proof requirements.

## 3. Required ownership after conversion

| Responsibility | Single logical owner | What consumers retain |
| --- | --- | --- |
| Connection configuration, TLS and pool lifetime | Database infrastructure, constructed by instance composition | Queries, domain transactions and typed repository errors |
| Operator authentication | Existing product/management authentication boundary | Protocol-specific credential acquisition and verification |
| Tool allow/block and technical constraints | Existing kernel/authorizer services | Typed invocation data and required live dispatch checks through that boundary |
| Durable admission and run fencing | Existing turn/global-task infrastructure | Conversation/run/attempt references; no independent claim authority |
| Monty worker exchanges | Existing transport actor and supervisor | Typed submissions and receipt consumption; no competing worker read/write loop |
| Durable operation custody | Existing persistence/host services, extended only for demonstrated gaps | Operation-specific outcome inspection and reconciliation evidence |
| Event replay | Existing durable event logs and projection contracts | Protocol encoding, cursor transport and snapshot/rebase presentation |
| User workflow and explicit reconciliation steps | Recipes and pinned PythonCode/Skill exports | Domain decisions and typed handoffs |

“Single owner” means one implementation and authority per responsibility, not
one giant adapter, one database table or one universal wire protocol. Use the
existing PostgreSQL, HTTP/MCP, durable event and Monty IPC technologies at their
appropriate boundaries. Do not introduce a new broker, workflow platform or
generic RPC envelope without documenting a requirement the built-ins cannot meet.

Keep the following distinctions:

- Protocol authentication, operator management authentication and Tool policy
  protect different boundaries. An external channel sender is not an operator.
- Input validation, schema compatibility and current dispatch policy are separate
  checks. Share validators, but preserve validation at the actual trust boundary.
- Worker claims and generation fences prevent stale execution; they grant no
  Tool permission. Remove legacy run/invocation approval leases from the v3 path.
- Component approval and pinned selection are separate from live Tool policy.
  Never replace current policy with a cached bind-time allowance.
- A transport deadline, task executing-time budget, claim lifetime and external
  service timeout have different meanings. Consolidate duplicate values for the
  same constraint, not unrelated clocks.

## 4. Implementation sequence

### Phase A — trace callers and establish the responsibility map

- [ ] Trace ordinary startup, CLI, WebUI/API, external product adapters, triggers,
  matched Recipes, No-Match, provider/MCP calls, Sempai review and shutdown.
  Read applicable subsystem guides before changing each area.
- [ ] Inventory production connection/client builders, authentication decisions,
  claim stores, retry loops, deadline constants, event buffers and recovery owners.
  Separate tests/fixtures and standalone bootstrap utilities from product callers.
- [ ] For each proposed removal, record caller, current owner, effects, feature
  gates, replacement API, contract differences and required evidence. Classify it
  as exact duplication, conflicting rule, justified boundary check or missing support.
- [ ] For workflow changes, identify existing Recipe/Skill/PythonCode/ToolSkill
  UUIDs and actual primitive contracts before proposing Rust work. For technical
  gaps, extend the owning service rather than hiding the gap in a new workflow Tool.

Exit: every change has a named shared owner and a demonstrated reason. Do not
remove a check because its name resembles a check elsewhere.

### Phase B — consolidate PostgreSQL connection policy and lifecycle

- [ ] Consolidate parsing, endpoint classification, TLS mode enforcement,
  certificate validation, root loading, pool bounds and redacted errors in
  `brassclaw_pg` or its existing neutral infrastructure boundary. Inspect current
  driver configuration semantics before changing it; do not invent options.
- [ ] Cover URL and keyword forms, Unix sockets, hostaddr, multiple/mixed hosts,
  query-specified hosts, explicit loopback TLS and remote TLS rejection. Treat
  ambiguous endpoints conservatively. Preserve stricter existing guarantees.
- [ ] Construct the ordinary instance's pool in composition and inject clones
  into compatible repositories/event stores using existing supported constructors.
  Use `SharedPool` for the event store when it shares the same database.
- [ ] Preserve credential/database isolation where separate pools are necessary;
  those pools still use the same connection-policy implementation. Bound the
  aggregate connection count, including migration/admin reservations, against
  embedded Postgres capacity rather than assuming a per-pool cap is enough.
- [ ] Audit CLI migration/config helpers for independently constructed `NoTls`
  pools. Give standalone utilities the shared policy without forcing them to boot
  the global orchestrator just to migrate a database.
- [ ] Delegate connection acquisition/replacement to the existing driver/pool.
  Do not wrap arbitrary SQL mutations in automatic reconnect-and-retry loops.
  An uncertain transaction commit requires readback by durable identity before
  deciding whether it may be repeated.
- [ ] Preserve startup ordering and drain database users before embedded Postgres
  shutdown. Database failure remains explicit; no alternate file-backed catalogue
  or silent in-memory replacement for authoritative state.

Exit: production Postgres callers share one reviewed connection policy; necessary
separate pools and their capacity/lifetime are documented.

### Phase C — route adapters and Tool dispatch through existing boundaries

- [ ] Use `NativeProductAdapterRunner` and `ProductWorkflow` where their real
  contracts fit. Keep adapters responsible for parsing/rendering, external event
  identities, signatures and protocol acknowledgement timing.
- [ ] Remove repeated product binding/staging/submission logic after proving
  that the shared workflow gives equivalent durable outcomes. Audit WebUI, CLI
  and non-webhook paths rather than claiming webhook coverage applies to them.
- [ ] Reuse common auth verifiers, redaction, egress and limits. Preserve required
  origin/CSRF, body-size, rate-limit, secret, sandbox and network checks.
- [ ] Wire one instance policy source through native, retained, extension and
  applicable administrative Tool dispatches. Verify stable Tool UUID mappings
  across retained versions and aliases. Missing/conflicting identity fails closed.
- [ ] Remove scoped feature-role/profile decisions and operations-based Tool
  approvals only as part of the coordinated v3 cutover. Preserve meaningful
  conversation/run/data IDs, external authentication and component Q1/Q2.

Exit: adapters delegate common lifecycle work; every actual Tool call reaches
the same current authority rules without an extra operation approval layer.

### Phase D — unify transport ownership, reconnect and event replay

- [ ] Route Monty exchanges through the existing actor. Reuse its correlation,
  completion inbox, receipts, frame requirements, capacity credits, control lane
  and owned stop deadline. Remove parallel pipe readers and caller-local credit
  counters only after proving custody and release semantics remain intact.
- [ ] Reuse compatible HTTP/network clients and egress services. Maintain
  operation-specific protocol behavior and external auth; a reconnected client
  does not authorize replay of a request body.
- [ ] Classify each disconnect: observer lost, caller abandoned, transport lost,
  worker failed or authoritative persistence unavailable. Document whether the
  task continues, requests cancellation, or needs supervised recovery. Do not
  equate socket loss with successful cancellation or proof of no effect.
- [ ] Use durable event cursors for reconnecting observers. Replay retained
  events; expose a replay gap and use the supported snapshot/rebase contract.
  Do not resubmit a task to reconstruct a missing UI event.
- [ ] Keep the MCP server running. Kohai connects/advertises after final prefix
  addition immediately before provider submission and disconnects that request
  on complete answer. Discovery comes from approved available
  `mcp-call-skill-recipes`, refreshed at startup/restart and qualified catalogue
  changes; existing calls retain their advertised contract.
- [ ] MCP accepts the listed completed command, opens an ordinary chat, forwards
  its correlated result and closes that chat. No direct Python/Monty/IBS dispatch,
  per-call component creation or duplicate execution path is introduced.

Exit: each communication channel has one owner; reconnect preserves operation
identity and observer replay cannot create new effects.

### Phase E — share durable operation mechanics without flattening semantics

- [ ] Compare inbound, turn, Monty invocation and review records. Extract only
  common primitives such as guarded reservation, exact answer retention,
  compare-and-set settlement and immutable readback. Keep domain tables and
  release predicates where their semantics differ; no mandatory universal ledger.
- [ ] Before adding an interface, prove at least two real consumers need the
  same contract. Reuse existing traits/stores first. Document missing support
  explicitly; there is no assumed built-in universal recovery API.
- [ ] Retain logical invocation identity separately from execution attempt and
  service generation. Persist intent/count before effects, original arguments
  and deduplication key, exact pinned workflow selection, outcome evidence and
  continuation position where the supported continuation contract requires it.
  Keep sensitive values behind host-private references.
- [ ] Make settlement monotonic: completed results are reused; uncertainty stays
  unresolved until evidence establishes an outcome. Lease expiry permits a new
  owner to inspect work, not blind repetition of an external effect.
- [ ] Apply `skills.md`'s exact retry contract. Counts include initial dispatch
  and survive reclaim/waits. Only declared `confirmed_no_effect_transient` or
  `unknown_completion` outcomes with verified read-only/deduplicated evidence
  may retry. Preserve the same key/arguments and recheck live policy, cancellation,
  freshness and resources. A generic timeout is not retry eligibility.
- [ ] Keep operation-specific inspection as a separate usage. Recipes sequence
  reconciliation decisions; native infrastructure enforces evidence, accounting
  and release. Do not infer completion from an operator note or event-log entry.
- [ ] Preserve review reconciliation's actual native settlement/answer/accounting
  predicates. Lost native owners, unknown spend or unsupported accountants cannot
  be declared repaired by introducing a shared record format.
- [ ] Fence fatal worker generations, close admission and reconcile known effects
  before supervised replacement. No parallel VM, whole-Recipe replay, per-chat VM,
  hidden Rust loop or Tier-2 fallback. Durable records alone do not implement resume.

Exit: common mechanisms are reused by real callers while service-specific proof
remains explicit. Unknown outcomes cannot become “safe to retry” by abstraction.

### Phase F — integrate components, settings and upgrades

- [ ] Shared workflow usages declare preloadable exports, typed recursive
  contracts and exact associations; pure helpers may be pinned dependencies.
  Preserve one UUID per Recipe step and matching ToolSkill/execution pairing.
  Independent Tool effects remain separate steps.
- [ ] Resolve one coherent active approved catalogue at task start and retain
  exact Recipe/variant/step/input/dependency/association/artifact selections.
  Existing tasks and advertised MCP calls keep them through waits and recovery.
- [ ] Qualify shipped successor components through reviewed `system_seed`
  bootstrap with integrity, Q1 and behavioral evidence, without extra installed-
  instance human Q2. Installed authored changes retain their Q1/behavior/human-Q2
  requirements. Preserve overrides and old artifacts required by retained work.
- [ ] Use existing revision-checked settings persistence and actor acknowledgement
  to publish effective limits coherently. Keep desired/effective revisions visible.
  Remove duplicate constants only when they represent the same constraint; keep
  technical model/frame limits and independent resource clocks.
- [ ] Keep provider definitions DB-only under §10. Sharing protocol code does
  not justify a second JSON/env/provider registry or reactivation of disabled data.
- [ ] Read historical records through compatibility adapters. New migration
  versions preserve original identities, counts, outcomes and unresolved status;
  do not edit applied migrations or overwrite historical evidence.

Exit: consolidation works with existing approval, settings and retention contracts
rather than introducing parallel sources of truth.

## 5. Acceptance through actual callers

Use the development policy to select focused checks and reuse valid evidence.
Read `LOCAL_TEST_ENV.md`, if present, before remote tests or provider setup. Run
real Postgres/native-worker tests for persistence and lifecycle claims; helper
tests and skipped integration tests do not establish production support.

| Case | Required result |
| --- | --- |
| Fresh startup and normal shutdown | One ready global Monty; expected pools/capacity; producers stop and DB users drain before embedded Postgres stops. |
| Connection-policy variants | Same accepted/rejected behavior through all shared builders; mandatory remote TLS, certificate verification and explicit local TLS preserved. |
| Pool pressure and connection loss | Bounded acquisition and visible failure; no pool explosion or automatic mutation replay. |
| Commit succeeded but acknowledgement was lost | Readback by original durable identity resolves it; no duplicate write/effect. |
| Duplicate inbound delivery or expired reservation | One accepted logical action; reclaim consults downstream outcome and does not reset effect counts. |
| Multiple adapters performing the same action | Same shared workflow/authority semantics; protocol auth and data isolation remain correct. |
| Browser/subscriber disconnect and reconnect | Correct documented cancellation/continuation policy; cursor replay or explicit gap/rebase; no task resubmission. |
| Lost caller/worker answer | Actual receipt retained where available; unknown outcome stays unresolved; capacity is not released prematurely. |
| Live Tool block after binding/wait | Next dispatch denied through every retained version/alias; no additional run approval and no replay of an already-started effect. |
| Stale attempt, cancellation and competing recovery | Guarded settlement rejects stale ownership; no duplicate dispatch/reply or leaked capacity. |
| Permitted retry and rejected retry | Original key/arguments and durable count retained; only declared evidence-backed outcomes retry. |
| Successful effect followed by malformed output/reply failure | Effect retained; validation/posting failure never repeats it. |
| Fatal worker failure | Old generation fenced; supervised recovery only; no hidden executor or whole-Recipe/Tier-2 replay. |
| Catalogue/provider/settings edit during work | Pinned workflow stays fixed; current Tool policy remains live; provider lifecycle follows its defined contract; acknowledged settings apply coherently. |
| MCP provider request lifecycle | Correct advertisement/connection timing and ordinary-chat command ingress; discovery change does not alter in-flight contracts. |
| Historical and unresolved records after upgrade/rollback | Original identities/evidence readable; no reset of counts, invented completion or destruction of retained owners. |
| Review native-accounting reconciliation | Shared mechanics retain exact existing proof and release predicates; unsupported/lost evidence remains unresolved. |

Record the actual caller, features, revisions/artifacts, migration state, effective
settings, command/result and limitations. Check CLI, WebUI and external adapters
where their paths differ; one helper or webhook test does not certify all products.

## 6. Cutover, deletion and completion

- [ ] Convert one coherent responsibility at a time; update all of its affected
  consumers before claiming one standard. Do not stack wrappers over obsolete
  decisions and call that consolidation.
- [ ] Compare only pure decisions/read-only retained evidence before cutover.
  Never shadow external effects, provider requests or candidate writes.
- [ ] Switch future admissions to the qualified shared path. Preserve existing
  in-flight selections, original operation identities and native custody. Prevent
  concurrent consumption by old and successor delivery owners.
- [ ] Rollback affects future admission/configuration selection only; it does not
  redispatch uncertain work, restore completed effects or reset attempts.
- [ ] Retire duplicate builders, retry loops, policy resolvers, event buffers,
  settings constants and recovery wrappers once no retained task/artifact needs
  them. Keep documented exceptions with concrete contracts and owners.
- [ ] Update the final-v3 guides and implementation evidence after acceptance,
  preserving historical results. Mark only the accepted conversion complete.

Completion requires a caller-to-owner map showing one common implementation for
each consolidated responsibility, removal of the unnecessary duplicate paths,
and production-path evidence that the original safety/lifecycle guarantees hold.
New shared effect primitives must have at least two real consumers. Similar
record names alone are not a reason to merge distinct operations.

Measure reduced policy/builder implementations and duplicated ownership logic,
not the number of Recipe steps. Record latency, reconnect or resource improvements
only after representative measurements. Do not claim exactly-once external
execution, automatic crash resume, full-v3 completion or improved performance
from this plan or from source inspection alone.
