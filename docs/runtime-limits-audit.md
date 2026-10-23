# Runtime limits audit

> **Current status — 2026-10-22:** use [the combined implementation and ownership
> index](implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


Source audit: 2026-10-17. This is an implementation inventory, not a full-v3
acceptance claim. The release checklist must close the remaining fixed values.

## Controls already wired to Monty settings

Settings → Monty VM exposes the following controls with English and German
explanations. Desired and effective revisions distinguish a saved value from
runtime acknowledgement. Unsafe reductions must not discard retained work.

| Control | Meaning |
| --- | --- |
| Shared heap (`max_memory_bytes`) | Ceiling for the global VM's live heap. It does not reserve RAM. Startup/manual values have the required 512 MiB minimum. |
| Memory mode and startup reserve | Startup sizing measures capacity once and holds that ceiling. Manual edits are live. Automatic adjustment is optional. |
| Automatic sample interval | Time between capacity measurements in automatic mode. Startup/manual modes do not measure periodically. Mild pressure alone does not pause all admission. |
| Worker adapter reserve | Additional physical memory for Rust hosting and worker overhead. The finite physical backstop also includes the heap and two accepted IPC frames. |
| Preparation worker memory | Physical ceiling for disposable parsing/pure-validation workers, separate from the long-lived global heap. |
| Native artifact size | Maximum executable bytes retained for identity verification. |
| IPC frame and queued bytes/tasks | Bounds for accepted transport frames and waiting work. They grant no Tool permission. |
| Recipe contexts and retained attempts | Capacity for isolated live Recipe state and recovery evidence. Completed task state is released; unresolved effects retain their evidence. |
| Source/feed/value/stdout limits | Bounds on compiler inputs, feeds and values crossing execution boundaries. Source accounting is not code-cache occupancy. |
| Task duration | Executing VM time per task, excluding queue, idle and external waits. It does not limit global Monty lifetime. |
| Token-budget enable switch | Disabled by default. Actual model context/output capacities remain technical constraints. |

Authoritative source: `crates/brassclaw_host_api/src/monty_resources.rs`, the
Monty product DTOs and `live_monty_settings.rs`. Field-by-field help is in
`brassclaw_webui_v2_static/static/js/i18n/{en,de}.js`.

## Values still requiring implementation

| Current fixed value | Owner and required repair |
| --- | --- |
| Monty compiler nesting profile: 30 in debug builds / 200 in release builds | `vendor/monty-control/crates/monty/src/parse.rs::MAX_NESTING_DEPTH` is a fixed compiler stack-safety guard, not an adjustable operator budget. Raising `max_value_depth` does not change it. The new inventory parser reuses the existing compiler profile; a compiler `syntax` rejection can reflect this guard and is not proof of semantic invalidity. Simplified Phase 3a's parser/deep-data owner must implement safe configurable profile/cleanup support and acknowledged UI limits/help with actual worker/platform qualification before raising/removing it. Do not silently disable the guard or describe every cap as adjustable. |
| Ordinary workers and pending host calls | Both capacities are persisted and acknowledged live, with EN/DE help and actual membership/occupancy. Worker product r8 accepts startup, revisioned resize, draining and restart through the native WebUI/runtime; protocol20 wire r3 additionally retains an actual dependent child across growth and reduction. Dependent capacity is observed separately and shares heap/pending-call limits. Native dependent-service-20261015-r10 additionally accepts ordinary HTTP MCP sibling admission while their exact live parent holds the only ordinary slot, through the same VM and Recipes; service tests cover nested loans and descendant cancellation. Native multi-parent/recovery, rendered resource-form and cross-platform/release acceptance remain open. |
| Catalogue Recipe/component/variant counts, bytes and activation actor length | V141 adds five live execution controls with EN/DE help. Frozen native `catalogue-policy-20261012-r4` passed acknowledgement, lowering/restoration, replay, migration and ordinary global execution. New format-2 generations pin preparation limits. Historical format-1 compatibility is not a policy-retention certificate. |
| Matcher eligibility: former 4096 instruction/reference, 65,536 visit, 8192 row, 2048-character and 64 MiB guards | Removed from the neutral eligibility helper. Catalogue admission remains the owning layer’s retained policy. Exact conflicting revisions and invalid empty entries still fail; checked counts and fallible allocations never return a partial set. Native catalogue-eligibility-20261013-r2 matches 9,000 templates, including final entries, through the operator/global path. Engine checks cover larger complete graphs and repeated snapshots. Routing metadata now uses one coherent aggregate instead of one query per template, retaining the same checksum and all-class integrity checks. |
| Retained dependency graph: former 64-level traversal | Removed through an iterative DFS with fallible frame reservation; cycle/missing/duplicate/unrelated/shared-root checks and exact references remain. Native retained-graph-depth-20261014-r1 covers a 2,048-node neutral graph, a real PG deep graph with old selections after successors, deep-cycle rejection, global startup and strict affected lint. This does not remove the separate JSON parser depth or store capacities below. |
| Typed execution: former 262,144-node / 64 MiB consumer and journal bounds | Captured acknowledged value limits now reach ordinary task/step bindings, Tool arguments, observations and immutable per-intent V155 journal bounds. Native typed-resource-20261014-r2 accepts a 300,000-node value, live global profiles, bounded escaping/envelope serialization, real kernel/PG late answers after cancellation, retained child handoffs and strict root/standalone lint. Returned value node/depth checks exclude the journal wrapper; encoded bytes include it. EN/DE help explains retention and byte accounting. Native review-data-profile-20261014-r2 additionally accepts complete Monty 300,000-item handoff across a live edit and exact per-step profiles in workflow/usage behavioral review, with real kernel/PG answer/persistence and strict root/standalone lint. The JSON-to-Monty adapter now builds one bounded post-order arena without recursive construction or repeated ancestor graph merging. Native input-arena-20261017-r1 accepts 12,000-level adapter construction/rejection on a 64 KiB stack, exact scalar/budget semantics, actual worker retained state/value failure, ordinary global/MCP and strict affected PostgreSQL/Bedrock test lint. The successor output-forest-20261017-r1 also replaces recursive output traversal and holds every positional/keyword result in one guarded forest until validation completes. Late invalid arguments/keys/bytes clean up earlier deep JSON iteratively; shared graph expansion is charged per JSON occurrence. Eleven host units, actual worker live limits/task-local failure, ordinary global/MCP and strict affected PostgreSQL/Bedrock lint pass. The json-streaming-20261017-r1 successor adds shared stack-protected streaming serialization to frame/journal writers and kernel/primitive byte counting, and iterative borrowed bound/checksum walking with preserved v1 digests. It removes the kernel counter’s second full serialized copy. Contract/worker/retained-input tests, all137 registered Tool cases, actual Memory/global/MCP/PostgreSQL, architecture, strict affected PostgreSQL/Bedrock lint and separately locked process-frame callers pass. The dispatch-output-20261017-r2 successor replaces kernel dispatch cloning/redaction with iterative guarded copying, including cleanup after late key/secret rejection, output limits and audit failure. The immutable detector is compiled during handler setup and shared across clones. The 12,000-level real completion-handler regression runs on a 64 KiB stack with a preconstructed handler; public source/result DTOs still need explicit dismantling. The locally accepted guarded-parser prerequisite (R1 root and R3 independent snapshots, with explicit unchanged-input proof) additionally keeps strict JSON partial subtrees in one indexed forest through syntax/budget/trailing-data rejection, with stack-protected Serde callbacks. The supported 64-level schema/parser profile and Serde reader recursion limit remain unchanged. Public DTO/retained-execution cloning and cleanup, protocol decoding and deep recovery are still separate work. These are prerequisites, not complete deep wire support. The transport/parser 64-level support restriction, recursive output/owned JSON consumers and other bootstrap/prefix review-helper profiles remain required work. |
| Host-answer ownership and structured exception payloads | Locally accepted shared-answer-20261017 R1/R3 gives retained PortAnswer/HostAnswer Return data immutable shared handles, iterative final cleanup/equality and guarded direct answer codecs. Actual worker/Engine/PG/global/MCP callers and strict root/independent lint pass; later prefix edits have separate scope. Outer tagged protocol DTOs, raw task/input/output ownership and deep recovery still prevent lifting the current depth-support gate. The pinned control fork also retains fixed 64 KiB UnicodeErrorData::MAX_OBJECT_LEN and JsonErrorData::MAX_DOC_LEN omission rules. Those payload bounds need acknowledged resource/accounting and explicit error semantics, with real worker/recovery acceptance; they are not configurable yet. |
| Retained component store: 4096 references / 64 MiB snapshot; revision parser 8 MiB / 262,144 nodes / depth 64; database input text 2048 characters and step links 4096 | `revision_store.rs`, `component_revision.rs`, V028/V054. These are separate remaining limits, not removed by the matcher repair. Integrate actual store/parser/index admission with acknowledged profiles and retention; do not merely increase the catalogue form or remove database checks without considering index and parser constraints. |
| Retained IBS flow: former 255 executable steps / 16 result-path fields | Removed from `retained_inputs.rs`; the current global interpreter has no matching extra quotas. Frozen retained-flow-20261013-r1 preserves 300 complete steps and a typed 17-field path, with existing native retained-input regressions and strict engine lint. Actual value/transport/heap/task bounds remain effective; full branch/occurrence recovery is separate. |
| 10,000 events, terminal records and idempotency records / 90-second runner lease | `brassclaw_turns/src/memory.rs`. Expose retention/lease policy and coordinate live stores. Protected ancestors and unresolved work remain retained. |
| Lifecycle delivery: former 4096 cursor clear-all | Removed. Exact scope/run/cursor receipts prevent per-thread sequence collisions and survive count crossings. Same-event publication coalesces through weak per-event gates; errors remain correlated and observable until acknowledgement. Native lifecycle-delivery-20261014-r4 passes all139 coordinator contracts, real PG equal-cursor/error checks, global startup and strict affected lint. Safe durable receipt reclamation/restart remains required; these identities contain no task input/result payloads. |
| Prepared run identities: former 4096-entry ceiling | Accepted locally in prepared-identity-count-20261017-r1: the fixed count is removed from `brassclaw_turns::coordinator`. This ledger owns routing metadata rather than VM slots or Tool grants; fallible allocation reports failure before inserting and collisions cannot replace an existing identity. Submission/abort removes exact reservations; a fully drained ledger releases its backing table without periodic sampling. The current source inventory finds the prepare/abort methods in this compatibility coordinator and contract tests, not ordinary production callers, so a cosmetic WebUI setting is inappropriate. The focused >4096 fence/admission/abort regression, all142 coordinator contracts and strict affected all-target lint pass. Native prepared-identity-20261014-r1 remains historical evidence for the earlier scope/child cleanup repair. Other turn-store/subagent retention caps remain separate work. |
| Complete prepared Recipe model input | The host-only prepared-message API has no 512-byte status-summary, 64 KiB snippet or 32/128-message history window. Complete typed bodies are materialized under existing scoped refs and single-use prompt authority; enabled explicit count hints reject the whole request and disabled budgets ignore them. No new cap or UI field was added. Safety context, hooks, child capability-view narrowing and Monty task fencing remain. Local prepared-prompt-20261017-r2/r3/r4 covers exact large Unicode/Markdown/marker text, 141 messages, immutable refs, metadata errors, actual host resolution, cancellation/fencing, ordinary PostgreSQL/global execution and strict affected lint. This is infrastructure acceptance: it does not yet migrate the legacy child-goal composer, implement provider Tool-result replay in the three-role DTO or activate the canonical model usage. Captured consuming-usage admission/model limits still apply. |
| Legacy money-budget defaults and gates | Remaining: ordinary `runtime.rs` resolves absent `RebornRuntimeInput.budget_defaults` from compiled defaults/environment; the production search finds no `with_budget_defaults` call supplying the separately stored `budget.*` configuration. `BudgetDefaults` includes user/project daily defaults (5/2 USD), thresholds and a 1.20 estimate factor; the accountant also has a 24-hour legacy approval-gate expiry. The visible legacy `agent.max_cost_per_day_cents` field is a different owner and does not bind these defaults. Complete the instance-budget/legacy authorization cutover with explicit operator controls and help; preserve existing limit/override and ledger provenance, avoid resetting the current period on a cap-only edit, and retire per-run legacy gates rather than adding new invocation approval settings. Do not assume DB persistence is live uptake or relabel estimate-based spend as actual billing. |
| Model-work cost quote fallback | Implemented and accepted: `model_quote_output_tokens` (default 8192) is a live WebUI reservation estimate with EN/DE explanation, replacing the separate `ModelCostTable` constant. Explicit work estimates and known cost-row maxima take precedence; accepted holds retain their estimate/revision and nested accountants share the actual acknowledged source. Real global-settings/PostgreSQL holds, write rejection, precedence, cleanup, V170 preservation, ordinary global Monty and actual model/accounting-error integration pass. It does not cap output or establish actual provider usage. Exact selected technical maximum propagation, original pricing/route retention and explicit unknown/estimated versus observed settlement still require implementation. |
| Resource-governor PostgreSQL exchange deadline | Implemented and accepted: `resource_store_timeout_millis` (default 2000 ms) is editable in the existing Monty execution settings with EN/DE help. The actual PostgreSQL governor and gate store share the startup/acknowledged live source; one read/CAS write captures one policy. Real delayed-PG acceptance proves a timed-out mutation can commit exactly once, readback retains its outcome, and the last acknowledged deadline remains usable after global owner shutdown. Migration preserves explicit values and bumps incomplete rows once. No model retry or zero-spend inference follows from timeout. Full accounting-owner restoration after restart remains separate. |
| Subagent goals and gate/tombstone records | Locally accepted goal-retention-20261017-r1 removed oldest-goal eviction and memory/PostgreSQL goal payload caps. The successor subagent-prompt-20261021-r4 closes the separate spawn decoder/schema 64 KiB cap, composer 64/128 KiB caps and 512-byte status-summary misuse: full task/handoff/direction data uses host-only materialization with actual context, hooks, authority and capability narrowing. It also removes in-process gate/tombstone 4096 ceilings/eviction, rejects conflicting/foreign parent records and frees backing tables on exact explicit drain. Actual PG boot/142-message data path, surrounding callers, architecture, global Monty, real model custody/accounting and strict lint pass; the receipt scopes exact sources/artifacts. Durable R09 child admission/selection/waits and retained cleanup/cancellation/discard recovery remain open. Tombstones currently have no ordinary runtime consumer; their exact-delete API is not durable acknowledgement proof. Existing technical transport/runtime constraints remain separate. |
| MCP request cancellation index: former 4096 lifetime/active IDs per exchange | Repaired with live `max_mcp_inflight_calls` (default 4096), complete revisioned settings and EN/DE WebUI help. Native mcp-call-lifecycle-20261015-r3 proves 4097 actual registrations, live lowering without canceling accepted work, shared duplicate ownership, concurrent/aborted waiter cleanup, whole empty-batch reclamation and completed durable receipt recovery after late cancellation. V160 preserves overrides and evaluates prior defaults once; native WebUI/runtime uptake and strict affected lint pass. Ordinary saturated sibling chat admission is accepted separately in dependent-service-20261015-r10; full multi-parent/recovery and rendered form/platform acceptance remain open. HTTP byte limits are now independently live and explained, as recorded below. The request-ID guard/index repair is accepted in mcp-request-digest-20261015-r1 with r2 validation, as described below; aggregate transport buffering remains open. |
| MCP HTTP request and response bytes | Repaired with `max_mcp_request_bytes` and `max_mcp_response_bytes` (default 1 MiB), V163 preservation of complete/partial overrides and one prior-default evaluation, complete revisioned publication and EN/DE help. Native mcp-transport-bytes-20261015-r3 with r8 retained evidence accepts requests and complete ordinary-chat replies beyond the old ceiling, a held body across reduction, no replay after response overflow following an effect, actual listener and rendered settings with persisted/effective revisions. The three command/catalogue contract regressions also pass. The extra 64 KiB invocation guard/advertised maxLength is removed; exact typed usage contracts remain. The former 256-byte request-ID guard and raw-ID B-tree key are repaired in mcp-request-digest-20261015-r1/r2. V164 indexes a canonical scalar digest while retaining and comparing the exact JSONB ID; the actual populated predecessor upgrade preserves all rows, IDs, phases, references and receipts. Real HTTP accepts an incompressible ID over 14 KiB; equivalent numeric spellings recover the same original run with unchanged effect counts, and changed-command reuse fails. Native parent/HTTP39.253s, global Monty38.259s and strict PG/composition test lint66.138s pass. Existing live HTTP byte limits remain effective. The secondary declaration 64 KiB, catalogue input 1024 and 64-variable/example guards are removed in mcp-declaration-20261017-r1. Four contract/retained-worker tests preserve complete larger declarations and typed positions, conflict/qualification errors and no publication on failure; ordinary global/MCP callers and strict affected PostgreSQL/Bedrock test lint pass. This is not evidence for a fully qualified 1025-command catalogue. Enclosing revision/parser bounds and aggregate buffering remain open. Browser CI is wired through the existing native fixture and both actual local browser checks pass (settings-browser-ci-20261016); GitHub job execution, packaged and platform/remote acceptance remain required. |
| Host HTTP client retention | Repaired: `max_http_client_cache_entries` (default 128; zero disables retention) is persisted and exposed with EN/DE help. Individual LRU eviction preserves active handles and vetted-address keys; old builders cannot insert after a revision change. Native http-cache-policy-20261012-r2 passed actual host HTTP, PostgreSQL migration/rejection, live settings with one global VM and strict affected lint. Rendered browser and cross-platform acceptance remain open. |
| Global Tool-policy management and remaining authorization cutover | UUID-keyed GET/PUT policy APIs over the kernel table pass native instance-tool-admin-20261014-r3 CAS/rejection/class-0 identity, actual router/Recipe blocking and restart, exact revision wire, bearer/operator boundary and strict lint. No policy rows or approvals are created by editing. The editor now uses this API with exact revisions, allow/block and EN/DE help. Native instance-policy-ui-20261015-r3 passes actual Chromium/React, native facade/PG writes, stale-edit fencing and lost committed acknowledgement without application replay, plus the surrounding global regression and strict lint. No scoped permission writes remain in this editor. V166 now adds an optional per-Tool result-byte ceiling to that same UUID/revision row. The editor explains it in EN/DE, preserves exact positive unsigned values and existing limits across toggles, and explicitly clears to null. Native tool-output-policy-20261017-r5 accepts real PostgreSQL CAS/upgrade/rejection, Chromium edits, live kernel limit/increase/clear and post-effect failure recovery without replay in the same global VM; strict affected PostgreSQL/Bedrock test lint passes. Unknown advisory output estimates retain measured enforcement. Wall-clock/sandbox ceilings remain unsupported and are not exposed as working controls. Ordinary model-call registration/authority, complete technical parameter uptake and packaged product/platform qualification remain required; do not claim all Tool callbacks already share this policy. |
| Shared first-party primitive limits: 1 MiB input/output, coding input 6/21 MiB and 5-second manifest wall time | `first_party_tools/mod.rs` handlers and retained native metadata. HTTP has its accepted independent live-default path. The additional fixed input fallback for memory_write is removed in mcp-transport-bytes-20261015-r3: the real complete-history regression passes after an already successful reply, with eleven existing memory/input/mount cases and strict affected lint. Captured task value/IPC, backend/kernel/mount checks remain. Memory input/output byte fallbacks and its shared manifest ceiling/estimate are removed in memory-complete-data-20261015-r1: complete escaped/Unicode read/search results exceed 2 MiB with exact encoded-byte accounting; a retained Tool still enforces a configured kernel output ceiling and immediately honors its increase. Native PostgreSQL104.697s, MCP/history36.036s, global Monty37.682s, thirteen registered Memory cases, surface metadata and strict affected test lint62.618s pass. Other primitives retain their listed guards. The silent memory-search maxima (Tool 20, request 1,000, pre-fusion 5,000 and one FTS page) are removed in memory-search-counts-20261015-r1: explicit positive backend-representable counts are preserved, invalid optional counts are errors, and actual FTS pages are collected without a hidden total cap. All 113 memory tests, twelve registered Tool cases, native PostgreSQL count/default/policy checks, ordinary global Monty and strict affected lint pass. The successor real registered Tool/PostgreSQL test crosses 1,024 hits with 1,050 writes/results and passes104.222s in mcp-request-digest-20261015-r1/r2. The per-call default remains five; u32 count/rank representation, live kernel/mount checks and explicit kernel/transport constraints remain. Coordinate actual live handler/kernel profiles and retained implementation contracts before exposing adjustable defaults. Large typed-value review acceptance within these primitive bounds does not certify this cutover. |
| Memory write conflict preflight and filesystem namespace | Repaired in memory-namespace-20261016-r8: exact ancestors and the target subtree replace unrelated full-scope document/chunk/version enumeration. Complete listing, sidecar filtering and CAS semantics remain. Actual PostgreSQL transactions use shared ancestor/exclusive leaf locks through validation and commit; in-memory mutations check under the state mutex. Native parent/child, directory/file, cancellation, rollback, sibling-progress and CAS races pass (4.290s), along with all Memory contracts, registered Tools, ordinary Monty/MCP callers and strict affected lint. Existing invalid hierarchies and mixed old/direct-SQL writers are not silently repaired or certified. This introduces no application quota and is not a controlled production speed benchmark. |
| Memory tree depth | Repaired locally in memory-tree-20261016-r2: acknowledged `default_memory_tree_depth` (1) and `max_memory_tree_depth` (10) are live and explained in EN/DE WebUI. Explicit requests above ten work after raising the setting; invalid/null/over-limit values fail without clamping or fallback. A flat arena checks actual output depth, nodes and encoded bytes before creating nested JSON; result ordering and scopes remain. V165 preserves overrides/default evaluation and rejects invalid values or overflow; native retained Tool/PG, ordinary global/MCP, browser save/effective revision, canonical complete-update parser and strict lint pass. This does not remove the separate transport/parser depth restriction or bound unrelated input bodies fetched by document listing. Full library/installed-platform acceptance remains open. |
| Memory patch conflict attempts | The hidden `MAX_MEMORY_PATCH_RETRIES=8` loop is removed in memory-patch-20261017-r2. One Tool invocation performs one conditional patch write; a confirmed conflict is an explicit error without success/indexing/chat-memory effects, silent reread, fallback or replay. Actual PostgreSQL namespace-lock racing proves the competing body/version/sidecars remain intact; the next retained dispatch observes a live Tool block. Registered Memory/global/MCP callers and strict affected lint pass. Retry policy belongs to the exact approved Recipe/runner contract with retained arguments, persistent counts, eligible evidence and fresh kernel/attempt/cancellation checks. Without that contract a conflict stops; this removes an implicit limit rather than adding a second configurable runner. Full Recipe retry/deduplication/recovery and per-Tool technical controls remain required. |
| Memory document path listing projection | Accepted locally in memory-projection-20261017-r6. Generic body-free exact/page metadata reads are implemented by real PostgreSQL/in-memory backends and the existing scoped/mount boundaries, with capability checks and no full-body fallback. Listing/preflight preserve kind/scope/sidecar filtering, canonical ordering and complete pagination; chunk cleanup collects every matching path before deletion. Actual PostgreSQL SELECT(path,kind,is_dir)-only credentials prove listing succeeds while body queries fail; several pages, large bodies/indexes, all scope axes, corrupted-header errors, native namespace/registered Memory/global/MCP callers and strict affected lint pass. The successor memory-archive-20261017-r1 adds generic native projection of one indexed field with its own capability and existing scoped/mount gates. The archive allocator scans complete pages with only a running maximum, validates positive i64 counters against canonical paths, rejects malformed indexes and checks representation exhaustion. Actual column-access denial/malformed selected JSON and multi-page/boundary/committed-write contracts pass on PostgreSQL/in-memory, alongside affected callers and strict lint. Signed i64 is the existing storage representation, not an arbitrary history-count setting. Durable archive reconciliation, live patch retry policy, other allocations and full-platform/product acceptance remain open. |
| Network response/timeout defaults and HTTP Tool schema limits | The transport absolute 10 MiB ceiling and timeout-to-default clamp are removed. Frozen network-limits-20261012-r2 passed actual larger-response/delayed-response/invalid-value tests and network/host-runtime strict lint. Omitted request bounds now use live `default_http_request_timeout_millis` (30,000 ms) and `default_http_response_body_bytes` (10 MiB), with EN/DE WebUI help and native http-request-defaults-20261013-r4 acceptance. Accepted requests retain their captured limits. The HTTP Tool timeout now forwards omission to the live default and preserves explicit positive u32 values exactly; its fixed 30-second clamp and static wall-time ceiling are removed from the handler/native schema/resource manifest and packaged metadata. Native http-tool-timeout-20261013-r3 passed actual registered Tool sockets, 44 HTTP contracts, ordinary global execution and strict test-target lint. The complete HTTP Tool repair is accepted in native http-complete-response-20261013-r4: omitted response bounds reach the live default, strict overflow stays an error, and complete sanitized bodies/headers replace model-visible clipping. Fixed HTTP inline/save/output/header/upload ceilings and its shared 1 MiB input fallback are removed. Native tests cross each former bound (9 MiB escaped text, 2 MiB upload with 70+ headers, and 11 MiB real file save); all134 built-in contracts, ordinary global execution and strict test lint pass. Immutable upgrades of stored seed metadata, other Tool caps and SDK/ABI technical bounds remain separate work. Per-request limits are exact, finite and allocation/clock-representable. |
| LLM response-cache entries / lifetime | Repaired: provider definitions now store enabled/capacity/lifetime, with Inference dialog controls and EN/DE help. Both selected role chains uptake edits; zero capacity/lifetime disables retention. Native `live-cache-policy-20261012-r2` covers actual DB/WebUI/HTTP policy changes, retained old chains and restart restoration. Full browser/remote and model/pricing publication remain open. |
| Model discovery 5-second timeout, unbounded response decoding and static fallback lists | `brassclaw_llm/src/model_discovery.rs` replaces the removed name-based module. Candidate replaces name-based endpoint/credential shortcuts and fabricated fallback success; Inference provider dialogs expose total discovery timeout/response bytes with EN/DE help. Native `model-discovery-20261012-r6` covers policy/probe/credential capture and local HTTP adapters; browser, OAuth/signed discovery and final model-property cutover remain open. |
| Provider request/timing policy | Eight Inference controls with EN/DE help govern request timeout, additional retries, circuit threshold/recovery, base/max backoff, jitter and provider-hint wait ceiling. Native `request-policy-20261012-r2` and `retry-timing-20261012-r2` cover actual primary/Sempai/HTTP/DB changes, retained deadlines, counts, 401 classification, circuit state/recovery and a real Retry-After ceiling. Optional model response bytes now have an Inference control and EN/DE help: null (UI 0) retains unrestricted behavior; a finite ceiling rejects overflow without replay or truncation. Frozen response-capacity-20261012-r5 passed real registry/direct HTTP and primary/Sempai changes, including Codex SSE decoding; finite unsupported transports fail before publication/construction. Complete dedicated auth/session controls, full model metadata and browser/remote acceptance remain open. |
| Fallback model context and provider-chain values | Provider/model settings and gateway construction. Replace hidden model/URL overrides and publish exact model properties with the selected revision. A provider-level default is insufficient for arbitrary model changes. |
| Bootstrap validation fixture limits and retention schedules | Installation qualification and persistence maintenance. Derive configurable finite profiles; preserve checksums and protected checkpoints. |

The exact identifiers `cachemax` and `max_shared_code_cache_bytes` have no active
Rust/Python implementation in the audited crates/vendor tree. The current
Monty first-feed artifact cache holds weak references, checks live source bounds
on hits and removes entries with no strong owner during preparation. There is
no separate implemented setting with either old name; do not add a cosmetic
control that changes no owner. Retained running generations still need their
actual compiled definitions.

This inventory needs the final production source scan, platform/release checks
and test-machine acceptance after the outstanding owners are implemented.

Embedding dimensions and vector-cache ceilings now come from exact model
contracts in the selected provider's immutable DB revision, with per-model
Inference editor fields and EN/DE help. Runtime model-name/dimension/endpoint
fallbacks have been removed. Unknown model names require an explicit contract;
zero cache entries disables retention. V151 preserves explicit maps, zero/empty
values, removed providers and old revisions. A formerly selected unknown model
retains its old expected 1536/10000 shape as explicit migration data, not a
claim about the remote model or a future runtime fallback.

Native metadata management/upgrade and repeated initialization passed; the
custom-model HTTP path observes configured three-dimensional results and
cache-off behavior. Frontend helper/syntax and WebUI handler checks passed;
rendered browser acceptance is still open. Native ordinary-runtime memory attachment and same-dimension vector-space isolation are accepted. WebUI facade cache resizing/rejection/disable and safe source replacement are accepted in the scope recorded below; complete provider revision coordination and browser acceptance remain open.
The approximate 32,000-byte
input bound has been removed from the concrete adapters; the neutral interface
now distinguishes an explicitly known byte limit from no artificial limit.
Actual model/service input limits still apply. Full-input transport acceptance
and affected-feature lint establish this repair separately from model metadata.
The transport repair validates these current contracts and prevents backend
truncation; it does not expose cosmetic controls or certify model metadata.

The embedding cache adapter now supports a real zero/off ceiling, on-demand
allocation, synchronous resize with LRU eviction, and clear/resize fencing of
in-flight inserts. Frozen embedding-safety-20261014-r3 passed 23 neutral
embedding contracts and the real PostgreSQL/provider/HTTP caller regression.
The captured provider factory now uses the persisted per-model cache ceiling;
ordinary first-party memory now attaches the DB-selected source before global Monty
startup. Frozen live-memory-source-20261014-r6 accepts registered memory operations,
model-space swaps, live Tool blocking, invalid configuration before write, lexical
disable, synchronous WebUI facade cache 0/4 edits, DB rejection preserving the live
cache and client retention for cache-only edits. Retired caches are emptied/fenced
while active requests keep their results. Captured provider deadlines and response
capacities reach the actual OpenAI-compatible/Ollama/NEAR AI adapters; overflow,
timeout and redirects fail without truncation or replay. Help explains these controls
in EN/DE. Writes report incomplete indexing explicitly after a committed document
and retain text search; indexing failure never invites a write replay. Real PG/HTTP,
52 memory tests, affected host/architecture checks and strict Bedrock-feature lint
pass. This is native Tool/product-facade evidence, not rendered browser, complete
desired/effective provider/model revision coordination or whole-v3 acceptance.
Native embedding-safety-20261014-r4
also accepts sanitized service/transport/decode errors and the sensitive NEAR
AI bearer-header path with affected-feature lint; Bedrock is compile/lint
evidence only, without a live AWS request.

Network-boundary-20261012-r1 additionally passed real socket/subprocess proxy
isolation and sanitized-error acceptance. This repairs egress integrity and
credential privacy; it does not expose the outstanding network limits.

Gemini request construction no longer injects the fixed 8,192-token thinking
budget or forced HIGH level. Frozen dedicated-env-20261012-r3 verifies
model-owned defaults and preserved explicit output limits; strict consumer
lint passed. Dedicated authenticated transport/model metadata remains open.


Captured typed-value execution/journal acceptance is recorded in the table above.
Receipt `/private/tmp/brassclaw-typed-resource-accepted.json`; frozen runtime source
and retained artifacts `target/validation/typed-resource-artifacts-20261014-r2`.
The six unchanged retained-input cases reuse r1's passing evidence. Subsequent
EN/DE help edits passed both Node syntax checks and diff review; rendered browser
acceptance remains open. Transport/schema parser depth support, bootstrap/prefix trusted
review helpers and metadata/storage quotas still need their own implementation.


## Native Recipe model-response custody (2026-10-19)

The task-bound `host.kohai_complete` candidate uses the original invocation's
acknowledged typed-data byte ceiling for complete native response evidence. It
adds no independent fixed response quota or WebUI control. It keeps the actual
received response until an exact immutable PostgreSQL write is acknowledged;
failed persistence prevents task-owner release. A later kernel output-ceiling
failure or cancellation cannot authorize a second model call. Persistence-only
reconciliation cannot resume the Recipe. Successfully recorded responses release
their full in-memory source; the task retains small occurrence metadata.

The R5 live-model, global Monty and five task-host cases, R7 strict Composition
lint, and R8 journal/ten Q1 tests/strict standalone lint passed. Exact source
snapshots, per-case reuse and retained artifacts are recorded in the implementation
plan and `/private/tmp/brassclaw-native-model-usage-accepted.json`. This candidate
does not yet qualify or activate the canonical model Skill/Recipe. Complete
structured Tool-result history, durable crash recovery and full provider/model
budget uptake remain separate acceptance requirements. The implementation plan
records the exact candidate profile and failed earlier fixture runs.

Canonical draft preparation and the model-tier invariant now have separate
local acceptance in canonical-model-definition-20261019-r1: real PostgreSQL
retention, actual worker source/interface inspection, retained-input regressions,
ordinary global runtime and affected strict lint. The commands explicitly reuse
the JSON/model/reply primitives; they are not active routes yet. The JSON Tool's
fixed shared byte/resource ceilings now have scoped local acceptance in
json-acknowledged-20261019-r1/r2: the native handler consumes acknowledged
value depth/node/byte limits, bounds parsing/allocation and complete output,
and removes the extra shared 1 MiB/five-second profile. Actual registered Tool,
PostgreSQL live limits/write rejection, retained policy, ordinary global Monty,
dependency and strict lint checks pass. The strict component parser preserves
its original duplicate/depth profile through the shared guarded reader. EN/DE
help explains the JSON consumer. Later concurrent prefix/shared-startup edits
are outside that frozen acceptance and require final integration qualification.
This is not canonical model workflow activation or cross-release artifact
migration evidence. Other listed primitive guards and explicit usage contracts
remain open as recorded above.
Configured provider retry/failover and protected-root model policy/durability
also need their own acceptance. No cosmetic memory control was added for these
unimplemented owners, and the concurrent prefix-owner module remains outside
this unit's tested dependency cohort.

## Native registration startup consumer (2026-10-14)

The earlier startup-native constructor used a separate fixed 1 GiB ceiling and
default OS-verifier output/deadline limits. The ordinary factory now consumes one
persisted preparation snapshot and requires explicit limits for all three values.
The existing artifact-size EN/DE help names its startup effect. R4 native factory,
global Monty, standalone retained execution/Q1 and strict native/default feature
checks passed; see the implementation plan's exact scope and retained receipt.
Existing retained artifacts survive reductions; invalid source reads fail startup.
This is local macOS evidence. A Windows loaded-image candidate now passes local
cross-compilation and affected lint; actual Windows mapping/rename/replacement
checks are added to the existing native CI gate and remain required. Linux/Windows
runtime acceptance remains open, alongside the fixed values already listed above.

The nine-step canonical model-answer candidate subsequently passed on the same
already-running global service (`canonical-model-20261020-r2`, 2026-10-20), with
a real selected model, original model component identities, immutable response
journal and exactly one stored assistant message. Its strict Composition lint
also passed. This remains candidate execution, without combination approval,
ordinary routing/managed completion or whole-plan acceptance. The subsequent
provider adapter-observation custody repair uses the original invocation's
captured data ceiling for a separate immutable evidence stage; it adds no
independent fixed quota. That successor is still under validation and must not
be inferred complete from the earlier successful neutral-response tests.


Native model evidence and configured-rate ownership (2026-10-20): locally
accepted `provider-observation-20261020-r10` keeps separate immutable original
buffered OpenAI-compatible HTTP, adapter and validated-loop observations. The
original invocation's acknowledged `max_value_bytes` covers encoded evidence;
no independent response-evidence quota is introduced. Completed observation
memory is released after exact durable acknowledgement. Real same-global-VM
complete/truncated/two write-rejection candidates, original-response and
accounting recovery, relevant callers and strict affected/root lint passed.
The accountant also retains its configured rate from admission through direct,
auxiliary and retained settlement; a later table read cannot reprice the work.
This in-process quote is not durable rate provenance or a completed provider
price editor. Observed-failure accounting, other wire protocols/streaming and
partial transport custody, auxiliary evidence owners, canonical activation and
combined production/platform acceptance remain open. The new six-transport
parameter repair is a separate unaccepted successor at this point.


SDK request-parameter prerequisite accepted (2026-10-20):
`sdk-request-parameters-20261020-r3` passed all three actual single-submission
HTTP tests, including six registry/SDK transports, and the Rig request/response
contracts. The matrix made 24 calls: 22 original HTTP requests and two explicit
Anthropic required-output-limit preflight errors. No technical maximum was
invented. The existing-VM four-case model candidate passed in 48.48 s; affected
PostgreSQL + Bedrock test-target Clippy passed with `-D warnings` in 136.86 s.
The receipt `/private/tmp/brassclaw-sdk-request-parameters-accepted.json` records
frozen sources, binary hashes and the verified reuse of the unchanged
Composition caller across an LLM-unit-test-only change. These are SDK transport
and development candidate results, not cloud-provider inference, canonical
catalogue activation, durable pricing, combined prefix or whole-plan acceptance.


Subagent consuming-path successor under validation (2026-10-20):
`subagent-prompt-20261020-r1` removes the separate spawn schema/decoder
64 KiB limits and the prompt composer's 64/128 KiB limits. Full direction,
task and handoff use host-only prepared message materialization; the short
LoopSafeSummary contract remains unchanged. The new context-prefix prompt
method retains normal context loading, cursor/history policy, identity,
instructions, memory, safety, surface validation, hooks and the existing
single-use prompt authority. Complete-message Recipe callers retain their
existing explicit-message behavior. The child's capability view is intersected
on all three prompt entry points. The initial persisted child input preserves
its literal task/handoff rather than rewriting data through the status sanitizer.

Focused regressions include exact large Unicode/whitespace/code data, real
PostgreSQL goals plus 142 eligible transcript messages with token budgets
disabled, hook materialization and authority reuse rejection. These checks and
ordinary global Monty/affected strict lint are pending, not accepted results.
This is a host infrastructure prerequisite; it does not activate R09 or repair
its missing contextual child admission, inherited immutable selection and
durable cleanup/recovery. Existing technical transport/runtime constraints and
the separate gate/tombstone issues still require their own consuming-path work.


Subagent retained-bookkeeping successor prepared (2026-10-21, not yet validated):
The separate root changes in `subagent/gate_resolution.rs` remove the fixed
4096-state admission ceiling, reserve changed maps/vectors before logical
insertion, reject a conflicting replay of the same gate/child identity and
release backing tables after explicit full drain. The cached count is exact
bookkeeping, not a resource/admission setting. Existing result delivery and
explicit gate deletion remain in the completion observer.

`tombstone_store.rs` removes oldest-record eviction, retains immutable child
outcomes, rejects conflicting writes and exposes exact expected-record deletion
with idempotent absence. Draining releases its map allocation; poisoned storage
still fails. This store currently has no ordinary runtime consumer: tests and
readiness metadata are not durable cancellation/discard integration. The new
method explicitly does not establish durable acknowledgement by itself. R09
recovery and retained rollback outcomes remain required work. The two-file
changes are outside the currently running `subagent-prompt-20261020-r2` scope.


Subagent boundary/retention successor R4 under validation (2026-10-21):
Actual R3 PostgreSQL boot/history and global-VM regressions passed, but its
architecture library target executed zero tests and was explicitly rejected.
The actual architecture dependency target then found two pre-existing violations:
a lower runtime ID in the prepared host-message contract and a test listener
inside product source. The R4 successor uses a distinct `LoopPromptMessageSetId`
with explicit native UUID correlation and moves only the real loopback fixture
lifecycle into `tests/common`. Architecture rules remain unchanged.

R4 combines these repairs with the prepared gate/tombstone retention changes
and the exact first-child-message regression, then rebuilds affected host and
prepared-message contracts, actual architecture targets, native PG/global/model
callers and strict affected lint. No R3/R4 whole-scope acceptance is claimed yet;
failed predecessors remain attributed. The retained-bookkeeping-only launcher
is superseded by this combined scope and must not be run redundantly.


Subagent host/boundary/retention prerequisite accepted (2026-10-21):
`subagent-prompt-20261021-r4` passed both unchanged architecture targets
(28 dependency + 4 composition checks), all 48 Subagent host adapter tests,
106 Subagent store/observer tests, 12 prompt-hook tests, four host-prompt tests,
the instruction bundle regression, all three complete-message contracts and
the actual host preparation/fencing contract. Real PostgreSQL seeded/integrity
boot and complete-goal/142-history-message cases passed (7.25 s). Ordinary
global Monty passed (53.31 s), the real four-case canonical model candidate
passed (40.82 s), actual original native response custody passed (46.71 s),
and original-hold accounting recovery passed (13.67 s). Affected/root
PostgreSQL + Bedrock test-target Clippy passed with `-D warnings` (121.20 s).
The receipt `/private/tmp/brassclaw-subagent-prompt-accepted.json` pins the exact
frozen inputs, binaries and unchanged worker. Other-chat prefix/vendor changes
remain outside this source scope. No new source change followed the freeze.

The former task/handoff/composer caps and short-summary misuse are repaired;
complete input data is retained literally. In-process gates/tombstones no longer
use 4096-entry ceilings/eviction, and exact owner/conflict/deletion/drain rules
are observed. The distinct host message-set ID preserves native UUID correlation
without carrying effect authority. The actual test HTTP listener lives in the
existing test-support directory. Architecture rules were not weakened. R1's
wrong test-only import, R2's missing production Direction boot, the R3 zero-test
architecture invocation and its two actual boundary failures remain failed
historical evidence. R4 rebuilt and executed all affected checks after repair.

This accepts host infrastructure and retained in-process bookkeeping, not R09
child admission/activation, inherited immutable child selections, durable
rollback/cancellation/discard recovery, a general pure-approval schema or
dynamic effect occurrences. The tombstone store has no ordinary runtime
consumer; exact deletion alone is not durable acknowledgement. The actual model
candidate remains a development InternalRecipe. Full v3, combined prefix,
platform, release and installed-function acceptance remain open.


### Claim-fenced child admission prerequisite (2026-10-21; r2 accepted)

The existing Turn coordinator and aggregate stores now expose a separate trusted
`submit_claimed_child_run` / `submit_claimed_child_turn` entry. It retains the
original parent runner/lease proof and checks exact scope, run, turn, Running
status and unexpired non-null lease inside the existing state transition,
before cached replay and after asynchronous profile preparation. PostgreSQL CAS
retries retain that same supplied proof. Unsupported stores return Unavailable;
they never call the unfenced compatibility entry. Lifecycle publication, prepared
identity checks, descendant reservation, queued wake and deferred publication
errors retain their existing owners/order. Claims grant no Tool permission.

Frozen `claimed-child-20261021-r2` passes both actual unchanged architecture
targets (28 dependency / 4 composition), all 86 Turns library tests, all 145
coordinator contracts and six real filesystem contracts. New cases cover stale
identity before and after cached replay, cancellation during profile preparation,
expired parent claims and reopen/exact replay. The actual PostgreSQL test drives
real thread input, coordinator, lifecycle wrapper, aggregate admission, reopened
store, lineage and cancellation (4.39 s including launcher). Ordinary global
Monty passes (52.60 s); affected/root PostgreSQL + Bedrock test Clippy passes with
`-D warnings` (120.38 s). Receipt:
`/private/tmp/brassclaw-claimed-child-accepted.json`. Exact frozen inputs and
preserved binaries are retained; the unchanged worker hash is checked.

The unused status-string helper, its dead-code allowance and its sole `heck`
dependency were removed. Filesystem test fixtures now own TempDir cleanup through
the whole test instead of leaking it. R1 did not compile: its frozen lock selected
the peer's local onig patch while its frozen manifests still selected registry
onig. R2 projects the preceding accepted graph with only the unused Turns edge
removed, and preserves the separate current-root prefix module. No peer source
or dependency selection was reverted. Final combined-root acceptance remains.

This closes the trusted coordinator/store prerequisite, not R09's actual native
Tool dispatch, durable child-admission provenance, fresh child bindings against
inherited selections, explicit Recipe wait/results or unresolved cleanup/recovery.
The legacy child submission API remains compatibility-only and is not made safe
by this new entry. No bundled approval, child execution or whole-plan acceptance
is inferred. The separate pending host-fence ceiling repair is outside this
accepted source snapshot.


### Remove the second host-call ceiling (2026-10-21; r3 accepted)

The per-attempt Monty cancellation barrier no longer imposes a separate fixed
64-call limit. The existing acknowledged global `max_pending_host_calls` remains
the service admission owner, with its existing WebUI control and EN/DE help.
Call-address representation exhaustion remains an explicit technical error.
The barrier retains every started call and late answer; it does not grant Tool
permission or reserve a second capacity pool. No pressure measurement was added.

Frozen `fence-capacity-20261021-r3` passes the actual unchanged architecture checks
(28 dependency / 4 composition), all three barrier units and all six actual Reborn
Monty-host contracts. The new caller regression admits 65 independently prepared
calls, observes 65 pending at fencing, then checks every original withheld answer,
unique identity, settled acknowledgement and refusal of further dispatch. It uses
the existing recording gateway; it does not claim 65 actual provider calls. Prompt
bundles retain their single-use authority: each is consumed before issuing the
next bundle, while the already admitted waits overlap.

Ordinary global Monty passes (57.28 s), actual real-model native answer custody
passes (37.47 s), and affected/root PostgreSQL + Bedrock test Clippy passes with
`-D warnings` (120.39 s). Receipt:
`/private/tmp/brassclaw-fence-capacity-accepted.json`; exact frozen inputs and
preserved artifacts remain in the corresponding validation directories. The
architecture binaries were reused after exact source/dependency applicability
checks and executed against the new checkout; the production consumers were
rebuilt. Worker bytes are unchanged and checked.

R1's wrong test crate name and R2's incorrectly pre-issued prompt grants remain
failed historical fixtures. Neither prompt authority nor architecture rules were
weakened. Only the corrected R3 scope is accepted. The new child actor/CAS safety
repair is a later unaccepted scope; R09 native admission/provenance, inherited
selection, explicit waits/results, durable recovery, full-plan/platform/combined
prefix/release acceptance remain open.


### Claimed child actor and backend CAS safety (2026-10-21; r1 accepted)

The new claimed child entry now verifies the actual stored parent's actor before
admission and cached replay, including after async preparation. This preserves
the original tree's cancellation identity; it adds no Tool grant or feature-role
check. The existing malformed mixed-actor compatibility-tree test established
why this matters. The legacy unfenced submission contract is unchanged.

Filesystem claimed admission now requires the requested backend version CAS.
It never substitutes an unconditional write after Unsupported. Its positive
reopen/replay fixture uses actual CAS-capable InMemoryBackend; a separate real
LocalFilesystem negative case proves refusal leaves the complete persisted
snapshot unchanged. The older byte-only compatibility operations remain explicitly
process-local and are not production authority evidence. This supersedes the
previous positive LocalFilesystem claim fixture for the stricter contract.

Frozen `claimed-child-safety-20261021-r1` passes actual architecture checks
(28 dependency / 4 composition), all 86 Turns library tests, 145 coordinator
contracts, seven filesystem contracts, actual PostgreSQL actor/claim/reopen/tree
cancellation (4.47 s), ordinary global Monty (54.55 s), and affected/root
PostgreSQL + Bedrock test Clippy with `-D warnings` (119.57 s). Receipt:
`/private/tmp/brassclaw-claimed-child-safety-accepted.json`. Exact sources and
preserved binaries scope the result; peer changes remain excluded and untouched.
No model path changed; real-model custody retains its separate earlier scope.
R09 native dispatch/provenance, inherited child selection/waits and durable
uncertain cleanup, plus full-plan/combined-root/platform/release acceptance,
remain required. No whole-plan completion is claimed.

### 2026-10-21 — Matcher response count follow-up (r1 locally accepted)

The neutral matcher's fixed `MAX_DISAMBIGUATION_CANDIDATES = 3` and SQL LIMIT
are removed in source. Disambiguation keeps all distinct workflows satisfying
the existing rank/spread rules; aliases remain deduplicated. The ordinary catalogue
still uses its acknowledged admission capacities. Native routing/facade regressions,
all six existing dependency cases on native PostgreSQL, ordinary global Monty,
architecture, strict affected root lint and independently locked Monty journal
and lint pass in `retrieval-integrity-20261021-r1`. Receipt:
`/private/tmp/brassclaw-retrieval-integrity-accepted.json`. It does not remove the separate parser/store/heap
limits listed above or change the disabled-token-budget contract. Existing score
normalization, spread and score-update rate/bucket constants remain separate
algorithm/policy values to reconcile; they have not been exposed as live settings
by this change. No claim that all fixed values have been removed is warranted.


### 2026-10-21 coding primitive follow-up (source only; live acceptance pending)

Root source replaces coding's read/write/patch, default read lines, directory
entry/default-depth, glob result, grep output/default-page/scan, visited-entry and
diff input/preview/context constants with fourteen acknowledged execution settings.
EN/DE labels/help identify units, zero-pagination semantics, per-operation capture
and independent task/IPC/kernel constraints. The shared coding input/output quotas
and static manifest ceilings are removed so raised values can actually take effect.
Bounded backend reads replace stat-then-unbounded reads; grep also bounds actual
stream bytes by the remaining scan allowance and accounts for growth. Maximal
explicit grep context uses saturating EOF arithmetic. V175 fills missing policy
keys with existing defaults while preserving explicit values and retained manifest
bytes. Native previous-row/default-evaluation/invalid-value/overflow checks and a
real WebUI-facade save → PostgreSQL → acknowledged primitive → retained native
Tool regression are prepared, including a larger-than-former-quota write/read and
rejected-save rollback. These root changes are not covered by the safety-only
`patch-expansion-20261021-r1` snapshot and are not yet accepted. Faithful raw
snapshots, remaining primitive/parser caps and whole-plan cutover remain open.


The fourteen coding settings above are now locally accepted in
`coding-policy-20261021-r3`; receipt
`/private/tmp/brassclaw-coding-policy-accepted.json` records all twelve successful
checks, exact frozen inputs and verified artifacts. Actual native WebUI-facade
saves, rejected-save preservation, larger-than-former-quota file operations,
old-policy restoration, ordinary global Monty, kernel policy and strict lint pass.
This supersedes the pending status for that exact scope only. Actual browser,
merged peer prefix/local-onig, platform and release acceptance remain open.
The subsequent `coding-materialization-20261021-r3` projection closes that
scope's remaining full-read vectors and core bounded-directory forwarding.
Walkers request the remaining visited allowance plus one overflow witness;
Composite forwards, PostgreSQL limits distinct direct children before transfer,
and in-memory listing retains selected metadata. Generic backend compatibility
defaults may still materialize; partial windows do not promise global ranking.
Receipt `/private/tmp/brassclaw-coding-materialization-accepted.json` records
ten successful checks including actual native PostgreSQL, live settings and
global Monty, plus rebuilt filesystem tests and strict affected test lint.
Native execution uses the explicitly identified R2 projection; R3 fixes only
test-helper visibility and a test label after the R2 lint failure, with exact
production-input equivalence. No warning is suppressed. This adds no periodic
pressure measurements or hidden quota, and establishes no RSS/speed, raw snapshot,
merged peer, platform, full-plan or release claim.

### 2026-10-21 — Model preparation and exact primary budget release

`model-preparation-20261021-r2` passes all sixteen checks in
`/private/tmp/brassclaw-model-preparation-accepted.json`. Mutable provider roles
are retained before budget waits while dispatch still reads live technical
limits. The ordinary WebUI provider-change regression and explicit neutral test
lint also pass in `/private/tmp/brassclaw-model-preparation-runtime-accepted.json`.
A native Sempai preflight failure proves the separate primary model was not
submitted; only that proof permits releasing its exact reservation. Actual
PostgreSQL rejection of the release preserves the hold and surfaces failure.
Provider-entry uncertainty and independent Sempai spend remain retained.
The subsequent local provider-preflight proof is accepted separately in
`provider-preflight-20261021-r1`:
`/private/tmp/brassclaw-provider-preflight-accepted.json` records all eight
successful checks, including the actual typed non-submission outcome before
HTTP, retained actual responses after provider entry, ordinary live role/global
runtime and strict affected lint. No new limit, pressure counter or setting is introduced.
The boot-derived price table and qualified/durable pricing remain open; these
results do not establish that the dispatched model's original price is used.


### 2026-10-21 — Numeric defaults and per-work costs (locally accepted)

`accounting-representation-20261021-r2` passes all fourteen recorded checks in
`/private/tmp/brassclaw-accounting-representation-accepted.json`, including
actual PostgreSQL concurrent writes/imports, live quotes, real-model response
custody, global runtime, and affected/independent strict lint. Unrepresentable
positive defaults cannot silently become unlimited zero or a replacement factor.
Malformed complete budget documents fail before mutation. Checked per-work costs
retain original response/meter and holds when pricing cannot be represented.
These are technical representation checks, not new operational caps or completed
pricing provenance. Live legacy-default WebUI, merged peer and full-plan claims
remain open.

### 2026-10-21 — Aggregate counters (r3 locally accepted)

The governor's Decimal/u64/u32 representation boundaries are technical types,
not new configurable quotas. The accepted path rejects aggregate overflow and lost
USD contributions before publishing any cascade balance, including when token
budgets are disabled. No valid operational limit is silently clamped. Failed
settlement retains original holds; inconsistent stored active balances remain
unchanged and visible as errors. The exact `resource-ledger-20261021-r3` projection passes all fourteen checks in
`/private/tmp/brassclaw-resource-ledger-accepted.json`, including actual native
PostgreSQL, rebuilt worker, real-model custody, global runtime and strict
affected/independent checks; retained artifact hashes are verified.
The existing configurable limits, portable memory mode and explanatory WebUI
requirements retain their prior scope; this change adds no monitoring loop.


### 2026-10-21 — Internal model preparation (r2 locally accepted)

Internal inference now has a candidate non-I/O retention boundary before policy
and accounting waits. It pins the actual selected gateway to the complete typed
request, preserves the guard wrapper and keeps current live model limits. A
native regression is prepared using real inference and a real PostgreSQL governor
with a transparent policy pause. Its exact `system-preparation-20261021-r2` projection passes all eleven checks in
`/private/tmp/brassclaw-system-preparation-accepted.json`, including actual model,
PostgreSQL, global runtime and strict affected/independent lint; retained artifact
hashes are verified. No new
cap, pressure sampler, billing provenance or whole-plan completion is introduced.


### 2026-10-21 — Structured-entry metadata boundary (locally accepted)

The PostgreSQL stat classification repair is accepted locally before extending
raw document acquisition. It aligns structured-record protection with the in-memory backend;
ordinary opaque files and current configurable coding limits retain their
semantics. The actual six-Tool/kernel regression on PostgreSQL and in-memory
backends, live coding settings, global Monty and strict affected/independent
checks pass in entry-sensitivity-20261021-r2 (R3 validation). Existing ordinary
parent directories also avoid a redundant unsupported legacy creation call.
No new numeric cap,
pressure measurement or limit setting is introduced, and R10 raw fidelity and
atomic source identity remain open pending their separate implementation.

### 2026-10-21 — Internal inference non-entry accounting (locally accepted)

A Rust-only system-inference submission result preserves native evidence that a
provider was never entered. The guarded accountant can then release the exact
reservation; timeouts, uncertain provider outcomes and failed release
acknowledgements keep their conservative treatment. This introduces no new cap,
pressure sampling or user setting. The actual PostgreSQL/live-provider
regression passes in system-nonsubmission-20261021-r1, alongside all shared
contracts, actual model custody/global callers and strict root/independent lint.
No full-system custody, pricing, activation or platform acceptance is implied.

Source inspection on 2026-10-21 finds all 62 MontyExecutionLimits fields in the
WebUI execution form, with nonempty labels and short-help keys for every field
in both English and German. All five memory-help keys are present as well.
This is a source correspondence audit, not rendered-browser acceptance, and
does not claim that the remaining fixed values above are already configurable.

### 2026-10-21 — Model-name lifetime limits (locally accepted)

The fixed 1,024-entry/256-byte lifetime interner and overflow placeholder are
removed in source in favor of owned/refcounted metadata. Immutable providers
can borrow names; mutable snapshots own them and release replaced allocations.
This needs no cosmetic WebUI quota or pressure sampler. Protocol/model-identity
validation remains separate. The 2,048-replacement metadata lifecycle regression
passes in model-name-owned-20261021-r1, with all 812 provider units, 60 gateway
contracts, actual selected-model/PostgreSQL custody, ordinary live-role/global
runtime and strict root/independent checks. This is metadata ownership, not
pricing provenance, full catalogue activation or platform acceptance.

### 2026-10-21 — Opaque-read classification and large ceilings (locally accepted)

Structured-kind checks now follow the actual entry/body read, and PostgreSQL
never sends a structured body from its opaque bounded-read query. A larger
caller ceiling no longer wraps negative when represented by the signed SQL
parameter; this is an equivalent comparison over backend lengths, not a new
quota or truncation. Actual kernel/backend race, full-ceiling and empty-file
cases pass in opaque-read-20261021-r2, including live coding/global callers and
strict affected/independent lint. All 16 recorded results and retained artifact
hashes were verified. No new setting or pressure sampler
is introduced. Atomic writes, raw source identity and containment remain open.

### 2026-10-21 — WebUI/native execution-limit ranges (locally accepted)

The WebUI scalar parser now uses Rust's actual u32 fields and the u32 IPC frame
limit; the browser polling timer retains its stricter signed limit. HTTP default
timeout/body ceilings are u64 and no longer have an unrelated u32 form ceiling.
All numbers still require exact JavaScript safe-integer representation, and the
server retains platform/clock/body-allocation feasibility checks. Previously
missing catalogue/value-depth u32 guards now reject invalid form values early.
The existing ordinary global-runtime regression adds HTTP defaults above u32
on 64-bit hosts and checks the same persisted/effective revision in the HTTP
cache and Monty without replacing the VM. The exact scalar parser boundary
checks and JavaScript syntax pass. The frozen http-ui-ranges-20261021-r1
ordinary PostgreSQL/global runtime case passes (53.070 s), as does strict
affected test lint (98.126 s); all three results and artifact hashes are
verified in /private/tmp/brassclaw-http-ui-ranges-accepted.json. No rendered
browser, 32-bit execution, merged-peer or full-plan acceptance is implied. No
new cap, setting, pressure sampler or completion claim is introduced.

### 2026-10-21 — Atomic opaque coding writes (locally accepted)

The existing write/patch usages need a native storage guard: a trusted conversion
between preflight and mutation could otherwise be overwritten through Any. One
unified `put_opaque` operation now checks the target kind atomically with native
CAS/namespace mutation. Ordinary trusted `put` conversion remains available.
PostgreSQL predicates and in-memory mutex protection share their existing put
implementation; no second filesystem trait or workflow Tool is introduced.

A declared capability, scoped permission forwarding and composite mount routing
cover the operation. Local files preserve supported Any/Absent behavior and
reject unsupported version CAS before effects. Byte-only memory adapters preserve
prompt safety/schema/event handling and reject unsupported CAS without entering
that path. Coding Tools transfer the bytes to this guard; new write buffering
uses fallible reservation. Native writes create/infer missing parents after the
ordinary-directory preflight instead of a redundant legacy mkdir.

The actual kernel/backend regression covers both write and patch conversions
at the native mutation boundary, all record CAS modes and missing-parent creation
on PostgreSQL and memory, alongside the prior read race. Scope/mount routing,
real memory-repository adapters and existing Local coding contracts are included.
All changed sources were reviewed before compilation. Frozen
opaque-write-20261021-r1 passes all 18 recorded checks: native race (9.746 s),
namespace/CAS (2.953 s), live coding (28.251 s), ordinary global Monty (53.052 s),
strict affected root PostgreSQL/Bedrock lint (181.663 s), independent journal
(18.546 s) and lint (35.769 s), plus shared contracts. Receipt
/private/tmp/brassclaw-opaque-write-accepted.json and all retained artifact hashes
were verified. A validation-harness disk guard interrupted after 15 successes;
the initial log/checkpoint is retained, mandatory own-target cleanup was done,
and only the remaining checks resumed. Three baseline Docker host cases remain
ignored; no product failure or warning was suppressed.
No new numeric cap, setting, pressure sampler or installed-instance approval is
introduced. Full patch content/version CAS, raw snapshot fidelity, legacy byte
migration, hostile local containment, catalogue/platform/full-plan acceptance
remain separate required work.

The native write audit also removes saturating record-version/event-sequence
advancement. `checked_next` returns None at representational exhaustion; memory
put/opaque-put/byte-append/event-append reject before publishing changes. Native
PostgreSQL's signed version overflow aborts and rolls back. Actual maximum-state
regressions check retained body/version/events. This is the storage integer
representation, not a configurable resource quota; no counter can be reused.

### 2026-10-22 — Retained worker claim handoff (locally accepted)

The child-task bridge needs the actual worker claim. Ordinary worker handoff now
retains it as an Arc through the existing global Monty task host, with consistent
scope/actor/message/run/profile/internal-source checks. Tokens stay Rust-only;
they are not Python/model inputs or Tool grants. Existing explicit-attempt
callers carry no full claim rather than reconstructing one from IDs. Durable
child admission must still recheck the original claim and preserve the captured
catalogue, waits, cancellation and complete results; those R09 steps remain open.
The metadata regression checks Arc identity, nine mismatches and absent-claim
behavior without invoking or simulating work. Existing worker fixtures now model
the actual Queued-to-Running claim transition and complete host metadata. Actual
PostgreSQL/global-path and affected caller validation passed in frozen
claimed-handoff-20261022-r1: Turns units (0.238 s), worker contracts (3.004 s),
ordinary global Monty (56.533 s), strict affected root PostgreSQL/Bedrock test
lint (171.959 s), independent journal (18.289 s) and lint (34.888 s). Receipt
/private/tmp/brassclaw-claimed-handoff-accepted.json and all retained artifact
hashes were verified. The worker contract fixtures establish transport/identity;
the native case exercises the actual product worker with PostgreSQL. This is
not child execution, catalogue inheritance, merged-peer or full-plan acceptance.
No numeric cap,
setting, pressure sampler, Rust workflow fallback or activation is added.


### 2026-10-22 — Task-bound native child service (locally accepted prerequisite)

R09 infrastructure now has a Rust-only typed child workflow/input/outcome port
and a narrow retained-native binding constructor for builtin.spawn_subagent.
Binding preserves the original handler, exact image/schema identity, planner,
substrate and supplied live authorizer. Only the matching native capability gets
the task-bound service after the original resolver succeeds. Default services
have no child port. Claims, catalogue provenance and workflow selection cannot
be supplied as model-authored authority fields. Complete result text is distinct
from admission, failed/cancelled/recovery-required outcomes and unknown effects.

This is a service-binding prerequisite, not an activated child profile. The
existing authorization-only builtin remains unchanged; no Recipe may treat its
authorized:true receipt as child success. Actual claimed admission, private
parent/child linkage, original catalogue inheritance, dependent scheduling,
explicit wait/full-result handling, cancellation and bootstrap qualification are
still required before the five subagent Recipes can activate. The new boundary
regressions inspect binding without invoking child work and exercise real native
identity retention and kernel denial, never fabricated successful children.
Frozen child-binding-20261022-r2 passes all eight recorded checks: service
binding (0.456 s), original service resolution (0.024 s), actual native identity/
kernel denial (0.714 s), retained live policy (0.041 s), ordinary PostgreSQL/
global runtime (54.422 s), strict affected root/PG/Bedrock lint (129.185 s),
independent native invocation journal (20.392 s) and its strict lint (38.394 s).
/private/tmp/brassclaw-child-binding-accepted.json and retained executable hashes
were verified. R1 failed only on the new module's wrong LoopMessageRef import;
its diagnostics remain archived. R2 corrects that root export. The marker port
never produces an admitted/completed child. No R09, merged-peer, platform or
full-plan acceptance follows.


### 2026-10-22 — Typed input and native answer custody (scoped acceptance)

The existing JsonValue owner replaces recursive selected-input copies; native
answers retain the actual original journal handle and immutable answer rather
than a second complete serialized JSON tree. Original captured data limits still
apply. Receipt acknowledgement cannot replay native effects or change a failed
Recipe to completed. Frozen task-native-custody R3 passes the real native
COMMIT-failure regression, independent journal and both affected strict lint
graphs; unchanged R2 evidence includes ordinary global execution and the actual
operator-selected model. The exact receipt is
/private/tmp/brassclaw-task-native-custody-r3-accepted.json. No whole-plan, model
policy or resource-setting cutover claim follows.

Exact claimed-child reservation release is separately accepted in frozen
child-reservation-20261022-r3: ten checks and eight retained artifact hashes,
including a real PostgreSQL COMMIT-response loss, exact repeated release without
consuming a sibling, filesystem CAS/reopen and pending-record pruning. Its
receipt is /private/tmp/brassclaw-child-reservation-accepted.json. The marker is
idempotency metadata, not a new resource quota, sampling counter or Tool grant.

The accepted child-observation-release-20261022-r3 successor releases redundant Rust answer payloads immediately after
the original journal acknowledgement, retaining the caller-owned return value
and immutable invocation metadata. Failed/unknown acknowledgements keep their
complete payload. Read-only weak witnesses extend the actual COMMIT regression;
they are compiled only for tests, do not retain payloads or supply outcomes, and
add no production measurement loop. Its actual COMMIT regression and child
cancellation/received-result cleanup, native model-wire/ordinary global callers,
independent entry/journal/factory cancellation and both strict lint graphs pass.
Nine recorded checks and five retained artifact hashes were verified in
/private/tmp/brassclaw-child-observation-release-r3-accepted.json. This does not
certify unknown admissions, parent export/bootstrap activation, legacy retirement
or full-plan/platform acceptance.

Still-open cap finding: process.rs MAX_TRANSPORT_DEPTH=64 remains a protocol
framing guard independent of configured max_value_depth. Do not claim that every
runtime bound is controlled by WebUI until guarded framing/admission semantics
and the actual effective settings/UI help agree. Removing it requires safe late
error cleanup and metadata decoding, not simply disabling Serde's recursion guard.

The turn-state snapshot stores also still have fixed optimistic CAS retry counts:
PostgreSQL Turns `PG_CAS_RETRIES=12`, PostgreSQL Threads `apply_optional`
with 12 attempts, and filesystem `FILESYSTEM_CAS_RETRIES=8`. These are
conflict retry policy, not permission to replay a Tool or child effect. Their
instance-wide configuration/live uptake and explanatory controls remain open.


### Complete raw file capture — 2026-10-23

`builtin.read_file` now supports `representation="raw_utf8_preserved"` under the
same live `max_coding_read_bytes` setting and current kernel/mount rules. It
forbids ranges and returns a complete UTF-8 snapshot or an error; the preview's
line default never silently cuts a raw result. No extra fixed read quota or
operator control is added. [Actual native acceptance](validation/2026-10-23-raw-file-snapshot.md)
covers WebUI/PostgreSQL byte-limit uptake, retained-handler policy blocking and
structured-entry races. Canonical raw component and document-workflow
qualification remain separate open composition/validator work.
