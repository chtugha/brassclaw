# Path to v3.5 — qualified Recipe-first execution

**Date:** 2026-10-17. **State:** consolidated design and roadmap; implementation, qualification, publication and deployment remain future work. **Execution checklist:** the Zenflow To-do tab for task `6326cc40-3f63-4867-8a8b-9f4f3455ded5`, using stable work IDs `V35-01`–`V35-30`. This document is the substantive architecture/acceptance artifact, not a second checklist.

## Purpose, authority and scope

The reference workflow is a Recipe: **read one bounded text preview → select displayed lines containing a literal → format a coverage-aware answer → post one reply**. Its independently reusable components, exact qualification and ordinary publication demonstrate the architecture. A read-only log-inspection Recipe reuses the same selected filter/formatter identities. No new `read_filter_reply` Rust Tool is warranted.

The broader correction is responsibility ownership, not the mere existence of Recipe rows. PythonCode/Recipes own task-data transformation, domain interpretation, business sequencing and completion. Rust retains storage integrity, authenticated correlation, transport, authority, fencing, interpreter hosting, effect custody and actual resource accounting. A genuinely missing primitive is distinct from a shared infrastructure repair. Neither category authorizes hiding a workflow behind a Tool.

Binding authorities are `./recipe.md`, `./skills.md`, `./tools.md`, `./toolskills.md`, `./simplified_v3.md` §§1.1/9/10 and `./docs/development-policy.md`. Older examples and status snapshots are subordinate. Requirements and the technical design are retained in `./.zenflow/tasks/strictly-follow-the-following-wo-6326/requirements.md`, `./.zenflow/tasks/strictly-follow-the-following-wo-6326/source-inventory.md` and `./.zenflow/tasks/strictly-follow-the-following-wo-6326/spec.md`. Both workflow attachments were read; requirements → specification → planning → verification/report governs this delivery. Zenflow replaces their file-checklist instruction.

This roadmap absorbs all requirements of `./docs/plans/sempai_v3.5.md` and `./docs/plans/simplify_connections_v3.5.md`. **Neither source plan is obsolete now.** Retirement requires all mapped implementation, safety, reuse, cutover and compatibility acceptance, not this document. The four ongoing v3 plans keep their other open work. This delivery changes no runtime, seed, migration, database, configuration, provider or deployment.

## Evidence baseline and support truth

The inspected working tree has HEAD `6a36dfc5b225128512ff761c076620866baea163` and substantial pre-existing edits. Eight drift keys from the specification were rehashed and unchanged at roadmap preparation: shared PG connection policy, authored catalogue, retained inputs/executor, review backend, compiler approval writer and both prefix/simplified journals. Source consumers were reinspected for the boundaries below. This is source/document evidence, **not a new runtime test**. Historical receipts are attributed and require byte/configuration applicability review before reuse.

| Evidence class | Meaning and promotion requirement |
| --- | --- |
| Binding target | Canonical requirement; not proof of enforcement. |
| Source-supported | Named actual writer/reader/caller supports the bounded behavior; no deployment claim. |
| Locally accepted | Named frozen caller/features/artifacts/configuration have recorded results; applicability to current bytes must be checked. |
| In flight | Implementation or evidence producer exists, but merged applicability/acceptance is not established here. |
| Deployed | Explicit installed-instance evidence; none was collected for this task. |

The future support index owns navigation, not semantics or a new registry. Each entry records owner, caller, exact source/artifacts, receipt date/scope, current applicability, remaining gate and consumers. Journals retain history. A newer Markdown paragraph does not invalidate unrelated passing evidence, and an untracked implementation file does not prove acceptance.

| Area and actual source seam | Current disposition | Remaining work / evidence owner |
| --- | --- | --- |
| `GlobalMontyOwner`, `./crates/brassclaw_reborn_composition/src/global_monty_startup.rs`, ordinary `./crates/brassclaw_reborn_composition/src/runtime.rs` | Source-supported global startup/service; scoped ordinary-path results recorded in simplified journal | Preserve one supervised VM and Ready-before-producers ordering; `V35-01, V35-24, V35-27, V35-29`, simplified-v3 lifecycle owner. No second VM/loop. |
| `build_ordered_instruction`, `./crates/brassclaw_engine/src/memory/instruction_builder.rs`; retained selection and `compose_typed_program` | Strict one-UUID building, typed inputs/results and pinned selection exist | Reconcile legacy descriptions, not replace IBS: `V35-02, V35-05, V35-13, V35-14`. |
| `./crates/brassclaw_engine/src/executor/retained_preload.rs`, `./crates/brassclaw_engine/src/executor/retained_source.rs` in that same directory | `python-preload/2`, inspected definitions, export selection and fixed invocation exist | Actual language restrictions still apply; no new loader: `V35-03, V35-05, V35-17` with Monty owner. |
| `./crates/brassclaw_reborn_composition/src/authored_monty_catalogue.rs` | Activated execution requires qualified preload invocations; exact usage closures plus narrow canonical-generation coverage populate approval coverage | General standalone pure execution approval is a separate gap: `V35-03, V35-04`. A helper in an approved usage closure is not an independently approved pure step. |
| `./crates/brassclaw_reborn_composition/src/component_deployment.rs`, `./crates/brassclaw_reborn_composition/src/pg_review_submission.rs` in that same directory; `./docs/reborn/component-deployment.md` | Complete subjects, read-only preparation and serialized activation/CAS exist; recorded local PG/operator/global-runtime acceptance | Extend this path, including new pure coverage, do not invent deployment service: `V35-04, V35-06, V35-07`. Recorded acceptance was not rerun. Fixed 512-subject prose is stale relative to acknowledged catalogue bounds. |
| `./crates/brassclaw_reborn_composition/src/completed_turn_review/backend.rs`, `Backend::run/qualify` | Rust still selects/composes evidence, interprets proposals, loops over submissions and classifies business receipt | Conversion remains open despite real wrapper Recipe/native reconciliation: `V35-09`–`V35-16, V35-28`. |
| `./crates/brassclaw_engine/src/memory/retained_inputs.rs::monty_flow`, `./crates/brassclaw_engine/src/executor/retained_recipe.rs::run_step` | Flat `recipe-flow/1`; begun step IDs and out-of-order flat occurrences rejected | Python flow syntax alone is insufficient; qualified flow and durable per-item occurrences: `V35-13, V35-14`, needed by `V35-15`, not by reference reuse. |
| `./crates/brassclaw_pg/src/connection.rs::prepare/tls_connector/connect`, `./crates/brassclaw_pg/src/pool.rs` | Shared parsed URL/keyword endpoint/TLS policy and dedicated sessions already exist | Reconcile historical divergence, audit remaining callers/capacity/lifetime and real TLS acceptance: `V35-01, V35-21`. Do not rebuild this module. |
| `./crates/brassclaw_reborn_event_store/src/lib.rs`, CLI migration/config callers | `SharedPool`, delegated standalone builder and shared size-4 CLI builders exist | Verify aggregate pools, isolated credentials, LISTEN/admin reservations, embedded bootstrap exceptions and shutdown: `V35-21, V35-29`. |
| `./crates/brassclaw_reborn_composition/src/pg_retained_q1.rs`, `./crates/brassclaw_reborn_composition/src/pg_workflow_review.rs`, `./crates/brassclaw_reborn_composition/src/pg_association_review.rs` in that directory | Trusted review/evidence infrastructure exists | Eight real class routes are not proven by helper/old nonempty-body Q1 runner: `V35-17, V35-18, V35-19`. |
| `./crates/brassclaw_skills/src/prefix_compiler_components.rs`; `./crates/brassclaw_reborn_composition/src/bootstrap_compiler_approval.rs` and sibling compiler validation/fixture modules | Multi-step compiler and qualification writer exist; latest journal/receipt record a locally accepted **closed source-only installation** slice, not merely an untested writer | Reconcile frozen evidence and precise Generate/activation boundaries: `V35-20`. Closed compiler coverage is not general pure approval. |
| Latest `./docs/plans/simplified-v3-implementation.md` resource entries | Attributed local streaming/dispatch-output results, including ordinary callers; current source repairs | Strict parsing, public deep DTO ownership, recovery/child/wait/live profile and installed-platform gates remain; `V35-01, V35-14, V35-24, V35-27`, no premature guard removal. |

No automatic crash resume, exactly-once external execution, model quality, speedup or full-v3 completion follows from these observations.

## Decision ownership and interfaces

| Decision or enforcement | Required owner | Boundary to inspect during acceptance |
| --- | --- | --- |
| Select/sequence domain steps and item/branch; transform data; interpret proposal; decide business completion | Recipes and small pinned PythonCode exports | Inspect actual method bodies and execution traces, not wrapper count. Domain-specific policy components are legitimate. |
| Resolve coherent active approved graph, exports, layouts, approvals and actual implementation artifacts | Existing catalogue/IBS/retained selection infrastructure | Match and assembly share one generation; no latest lookup mid-task, silent older version or Tier-2 replay. |
| Bind Tool usage | ToolSkill + existing retained binding reader | Metadata executes nothing/grants nothing; binding precedes matching execution, one UUID per step. |
| Dispatch authority and technical constraints | Kernel, current instance-wide stable Tool policy | All aliases/retained versions checked per real dispatch/retry/wait; missing/conflicting mapping fails closed. |
| Authenticate records, check checksums/subject bases, transact one write | Existing store/native primitive | Python selects requested work; native integrity and conflicts cannot be caller assertions. |
| Provider/model/prefix/instruction pins, request bytes, native settlement/accounting/release | Existing admission/transport/delivery owners | A business result, observation or reconciliation row alone proves no release. |
| Protocol auth, signature, ack timing and encoding | Thin protocol adapter | External sender is not instance operator; common admission does not erase protocol checks. |
| Pool lifecycle, worker pipes/credits, event replay and revision-acknowledged live settings | Their existing shared infrastructure owners | One authority per common responsibility, not one universal table/protocol/broker. |

Runtime text remains typed data, never Python source/prose execution. Recursive schemas declare items/fields/extras/null/defaults/bounds; defaults apply only to missing consumer inputs. Monty owns isolated task/attempt/invocation results and mutable state; immutable code/constants may be shared. Current policy is independent of pinned approval. Only actual No-Match enters Tier 2; matching/DB/assembly errors and begun Recipe failure never do.

### Two distinct shared-support gaps

**Standalone pure approval (`V35-03, V35-04`).** The existing review/revision/activation owners need a concrete trusted evidence schema, immutable writer and checked reader for an exact pure component/interface/source/dependency closure and behavior. Authored evidence requires Q1/behavior/human Q2; verified noncircular installation bootstrap uses integrity/Q1/behavior without extra instance Q2. Activation, restoration and task capture must check exact coverage and retain old evidence. No accepted field name is invented here. Preserve the exact `skill-association/1` and `skill-association-approval/1` formats; they describe real Tool usages, not zero-Tool logic. Do not widen the compiler graph exception, exempt arbitrary dependencies or hide filter/format inside file-read to evade the gap.

**Dynamic occurrence support (`V35-13, V35-14`).** The owning flow schema/IBS/qualifier must represent checksummed legal branches, item bindings/results and repeated selected usages. Monty chooses the next branch/item. Rust verifies selected membership, typed edges, freshness and unique occurrence identity, separate from task attempt and service generation. Journal intent/count precedes every actual effect; retain original arguments/key/checksum, exact answer and confirmed/unresolved state. V129's `(attempt_id, operation)` cannot represent repeated submissions. A successor migration/compatibility reader must preserve V122/V127/V129/V133 bytes and old flat tasks. Merely removing `started.contains(step_id)`, trusting a model loop index or adding unrecognized Recipe fields is unsafe. Initial conversion stays stop-only, not general crash replay/resume. Reference reuse and pure extraction can progress independently.

## Qualified reference Recipe contract

### Semantics and exact graph shape

The command and output explicitly say **bounded numbered-display preview**. The current `builtin.read_file` implementation in `./crates/brassclaw_first_party_extensions/src/coding/file.rs` accepts `path`, optional `offset` and `limit`, checks filesystem/mount/sensitive/binary/size limits, and returns `content`, `total_lines`, `lines_shown`, `truncated_by_default`, `path`. The content is numbered display text from one read. Registration in `./crates/brassclaw_host_runtime/src/first_party_tools/mod.rs` is not proof of an already qualified retained binding/Skill. Legacy `range`/`line_count` assumptions do not match by name.

The baseline variant supplies a validated nonempty path and a string literal (including the empty string), uses the existing default bounded preview (does not promise whole-file coverage), and preserves its returned metadata. Empty files/EOF are success distinct from denial/error. A future interval variant must qualify actual offset semantics; a raw/whole-file search requires a separately complete/raw or grep contract, not altered prose.

| Persisted order | Component role | Typed handoff and result |
| --- | --- | --- |
| 1, `knowledge: rust` | One actual read ToolSkill UUID | Effect-free availability of matching retained adapter/artifact. |
| 2, `knowledge: orchestrator` | Associated read PythonCode UUID/export | Local validated path → actual read object; one Tool invocation. |
| 3, `knowledge: orchestrator` | Independently qualified pure selector UUID/export | `content`, literal `substring`, missing-only default `invert=false` → ordered strings. Case-sensitive literal containment, no regex; empty substring retains all returned lines; inversion complements that selection. |
| 4, `knowledge: orchestrator` | Independently qualified formatter UUID/export | Selected strings + coverage metadata → nonempty answer, including explicit no-match/empty and preview/truncation labels. Preserve order, Unicode and displayed numbering. A small declared pure dependency demonstrates dependency-first reuse. |
| 5, `knowledge: rust` | One qualified reply ToolSkill UUID | Effect-free binding, no cached grant. |
| 6, `knowledge: orchestrator` | Associated reply PythonCode UUID/export | Formatted answer → qualified `host.post_reply(answer=...)` and actual correlated receipt. |

The admitted task's verified history/finalization owner completes the turn without a second root reply. No artificial binding for pure logic; preload is not an effect step. Independent read/reply calls stay separate. The supported dependent-chain exception is not needed.

Persisted JSON uses `knowledge`, `stepnumber`, one UUID per `include` and explicit `step_link`. `recipe-input-layout/1` under `input_layouts` maps task inputs, earlier result paths and constants to step-local `inputs`. Actual `python-preload/2` and retained `invocations`/default exports determine callable selection. Definitions declare exports/signatures/private helpers/constants/dependencies, load dependency-first without effects and reject cycles/conflicts/effectful initializers. Recipes reference stable identities; task selections retain exact versions/checksums/associations/artifacts. Actual UUIDs, complete envelopes and records are resolved/generated from the qualified fixture, **not fabricated here**.

A second genuine read-only log-inspection Recipe consumes exactly the same filter and formatter UUID/revisions and pure dependency, with independent inputs/results. It does not mutate the log and is distinct from the read-only completed-turn audit below. Reuse acceptance records both selections and checksums, not copied code or matching names.

Public variants need at least ten genuine intent examples, exact command grammar, separator/escaping rules and malformed/reordered/missing-input negatives. Values containing separators either use qualified escaping or reject before effects. Captures remain strings unless an approved explicit conversion exists. Deterministic eligibility requires observed **zero model calls**; no author flag suffices. Shell/subagent workflows are always Tier 1; neither is needed here.

### Qualification, publication and successor behavior

`V35-05, V35-06` produce real fixture-backed records and checked displayed examples, not another symbolic-only happy path. Exact usage combinations, independent pure steps, exports/dependencies and actual native registration/artifacts need qualification before publication. First-party package qualification is noncircular and installation-owned; installed authored edits use their own Q1/behavior/human-Q2 path. Deliberate operator overrides and original active graphs remain intact.

Reference acceptance is part of publication, not a detached future test phase:

| Acceptance obligation | Evidence and work IDs |
| --- | --- |
| Ordinary intent and exact variant reach the Recipe | Retained task selection proves Recipe/variant/generation; deliberate No-Match remains distinct. `V35-05, V35-06`. |
| Same helpers used by two workflows | Exact UUID/version/checksum/export/dependency selection from both actual executions. `V35-05, V35-06`. |
| Preview correctness | Empty, EOF, no literal matches, empty literal, inversion, default-capped file and Unicode/order; output never asserts raw/whole-file absence. `V35-05, V35-06`. |
| Hostile data and input/output contracts | Quotes, newlines, source-like text, separator values, null/missing/wrong types and malformed producer outputs; no source mutation, effects only after valid inputs. `V35-05, V35-06`. |
| Effect-free preparation and live authority | Bind/preload performs no Tool call; each read/reply checks current policy, including block after binding/wait. `V35-04, V35-06, V35-23`. |
| Isolation and correct result edges | Concurrent chats/attempts do not share mutable inputs/results/defaults; dependency preload and prior-result handoffs are actual. `V35-05, V35-06`. |
| Exactly owned reply/history, late failure | One correlated reply/history/final reference; bad output, failed reply posting or later history failure never repeats completed effects. `V35-06, V35-26`. |
| New activation versus old selection | New tasks select successor; suspended/captured tasks retain old exports/artifacts/approvals, including withdrawal/restart. `V35-04, V35-06, V35-27`. |
| Truthful Tier 0 | Actual ordinary executions show zero model calls and no fallback. `V35-06`. |
| Documentation is executable guidance | Generated/extracted record examples match accepted fixtures; constructor/schema/links checked. `V35-06, V35-07, V35-30`. |

## Concrete submission, deployment and discovery

The supported operation path is **immutable subject → trusted qualification → read-only preparation → explicit activation → ordinary matching → separately qualified discovery**. Each transition has its own evidence. A submitted `unreviewed` receipt is never rewritten to claim approval/deployment.

| Boundary | Existing concrete interface and required behavior |
| --- | --- |
| Immutable component revision | `ComponentRevisionDraft::from_json`, `./crates/brassclaw_skills/src/component_revision.rs`: `component-revision/1` fields `format`, `uuid`, `class_code`, `document`, `dependencies`, `association`. Do not add fictional fields to constructors. |
| Retained submission | `POST /api/webchat/v2/component-review-submissions`, `SubmitComponentReviewRequest` in `./crates/brassclaw_product_workflow/src/recipes.rs`: `submission_id`, `candidate_bytes`, optional `expected_previous_version`, explicit `base`, exact `dependencies`. Route/handler: `./crates/brassclaw_webui_v2/src/descriptors.rs` and `./crates/brassclaw_webui_v2/src/handlers.rs`. Actor/provenance is server-derived; preserve request bytes/identity after unknown commit. Store entry points are `./crates/brassclaw_reborn_composition/src/pg_review_submission.rs` and `./crates/brassclaw_skills/src/review_submission_store.rs`. |
| Trusted qualification | Existing retained Q1/workflow/association review owners retain exact complete subjects, rules/behavior and human Q2 where authored. Labels, a task manifest, structural parsing or unrelated individual approval cannot establish combination/pure approval. Submitted subjects alone do not schedule/complete review. |
| Inspect and prepare | Inspect `GET /api/webchat/v2/catalogue` and `GET /api/webchat/v2/component-review-submissions/{submission_id}`; `POST /api/webchat/v2/component-deployment` takes `activation_id`, `expected_generation`, complete Recipe subject checksum pins, workflow class and existing exact approval map. Repeatable-read preparation returns activation, preserved/replaced IDs, `catalogue_activated:false`. No execution or new approval. |
| Explicit publication | Send unchanged prepared activation to `POST /api/webchat/v2/catalogue`. Existing serialized generation CAS rechecks exact subjects/closure/base/approvals/registered artifacts. Preserve unrelated active Recipes and routing disables. Coherently replace affected shared graphs; never secretly update others. Raw withdrawal/whole replacement and routing changes are separate management actions. |
| Unknown activation commit | Preserve original ID, generation and bytes; exact retry/readback resolves it. Only known stale-generation rejection permits reprepare. Historical receipt readback never republishes a withdrawn generation. Submission uncertainty similarly never creates a new candidate or model call. |
| Ordinary execution | One coherent active approved generation, exact retained Recipe/variant/step link/layout/exports/dependencies/approvals/artifacts. Running work does not rematch; publication grants no Tool permission. |
| Authoring discovery | View derived from catalogue/source-export/review owners: active approved, retained-only, draft, legacy and unsupported; exports, recursive contracts/defaults/effects, binding/Tool/adapter/artifact, coverage, command and actual consumers. No second authoritative registry. |
| MCP discovery | Only approved available canonical `mcp-call-skill-recipes` with full-command qualification observations; active rows/raw Skills are insufficient. Internal validator/review usages remain private. |

Discovery also uses normal-task settlement observations where the existing owner qualifies commands. At ordinary command task capture the durable command receipt must agree with the advertised contract. A changed/withdrawn command before capture fails before effects, not by selecting a different Recipe or Tier 2; a captured task retains its selected graph across later activation.

`V35-06, V35-07` make the existing Settings → Validation queue → Deploy reviewed components walkthrough concrete using actual generated payloads and receipts, with UI/API agreement. Existing bearer/origin/CSRF/body/rate/audit boundaries remain. Current acknowledged `max_catalogue_recipes`/`max_catalogue_bytes` and independent HTTP limits replace stale fixed-limit wording only after source/setting verification. Oversized evidence/graphs reject whole, never truncate. Negative cases cover forged/stale/missing evidence, uncovered pure code, invalid export/adapter, changed pins/base, simultaneous CAS, disabled routing, unknown ack and restoration after withdrawal. No direct DB patch or alternate deployment service is prescribed.

## Sempai conversion and reusable library

### Consuming workflows and technical boundaries

The successor private Tier-1 Sempai Recipe uses explicit readers → evidence policy → complete/incomplete branch → shared message construction → one pinned model call → strict interpretation → all-subject preflight → per-item retention → receipt aggregation → result persistence. Its **read-only completed-turn evidence audit Recipe** shares the readers/transcript/correlation/envelope exports, calls no model and writes no candidates. Deterministic eligibility for the audit must be observed. A **third, genuine non-Sempai reviewer** shares message construction and the one-call model usage; the model-free audit cannot demonstrate that reuse.

| Shared library family | Component behavior | Native operation retained; genuine other consumer |
| --- | --- | --- |
| Immutable record/history/exchange/effect reads | Select allowed references/bounds, not transcript policy | Existing store queries with access/correlation/checksum integrity; audit/incident/reconciliation inspection. |
| Transcript boundary and correlation diagnostics | Original accepted/finalized boundary, preceding history, completeness and typed envelope, exact raw bytes | Readers authenticate facts; read-only completed-turn audit shares exact exports. |
| Message/envelope preparation | Explicit roles/order, pinned domain instructions as data, untrusted evidence separation | Admission pins sources; a non-Sempai reviewer supplies different approved instructions. |
| One selected model call | One Skill/export invokes once with the prepared request | Shared provider gateway owns transport, limits, claims, original/effective retention and usage. |
| Strict JSON and candidate preparation | Duplicate-aware parsing plus pure Sempai envelope policy and generic candidate preparation | Strict parser/integrity/host identity where required; other authoring/protocol consumers. |
| Candidate graph preflight | Recipe preflights every subject before first write | Existing read-only structural/base/dependency checks, reused by operator authoring/validators; not Q1/activation. |
| One candidate retention | Independent usage/occurrence per subject | Existing exact-subject transaction/receipt; human/other authoring consumer. No list/response batch Tool. |
| Actual receipt aggregation and result record | Pure business classification and explicit result choice | Native append validates provenance/immutability; other review/authoring workflows can aggregate receipts. |

Every family has coherent recursive contracts, missing/null/default behavior, exports/private symbols and exact dependencies. A real Tool-usage Skill has its matching association/binding and canonical execution Recipe/variant; pure PythonCode is not a zero-Tool Skill. Reuse may be across Recipes, as a qualified pure dependency, or as repeated selected occurrences. Only the third needs dynamic-effect support. Small useful components replace neither seven Rust wrappers with one giant Python method nor the global executor with a specialized workflow engine.

### Evidence, response and effects contract

Only actual original No-Match completion produces the existing learning handoff. Original turn/model/Tools/reply/history are never rerun to reconstruct evidence. Preserve run/turn/conversation and accepted-message/finalized-reply identity, preceding available history, failures after publication, missing/withheld/redacted records, capture/dispatch counts, multiple original model/Tool exchanges, provider replay arguments, final model versus posted reply and unresolved effects. Exclude later turns; daily memory is not a complete transcript.

Readers return authenticated correlated records. Pure policy returns typed complete/incomplete diagnostics; retain the actual bundle and actual selected qualification result with source/validator/occurrence/checksum provenance. Caller/model `evidence_complete:true` is not attestation. Incomplete evidence is retained and stops **before model entry**.

Admitted review pins provider/model, prefix/design/content, instructions and full component graph. New settings/revisions affect future work only. Shared request construction retains original host messages and effective provider-transformed layout (including adjacent system merging), response/failure bytes, replay identity and actual usage. No mutable seed fallback, silent evidence truncation or model output interpreted as code.

The entire proposal response rejects on any invalid item, duplicate JSON key/candidate identity, ambiguous/missing field, empty analysis, invalid allowed class or dependency graph. Preserve exact `candidate_bytes`, explicit `base` including null and exact dependencies. Generic candidate preparation is separate from Sempai envelope interpretation. Preflight **all** items before the first write and retain the complete validated list/response checksum; transactional integrity rechecks still apply at each write. Host-issued IDs preserve the domain-separated attempt/response/index/candidate-checksum algorithm or an explicitly versioned compatible successor. Models cannot select an old submission identity. Proposed code is never executed here.

Zero proposals mean no-candidates, not an export. Each nonempty item invokes one selected retention usage with its own host-verified occurrence/intent/count/answer. Earlier receipts survive later failure. Unknown commit uses original-ID readback, never another model call, new ID, candidate replay or whole-Recipe rerun. Pure aggregation preserves proposal/receipt order and linkage; completion distinguishes no-candidates, submitted-unreviewed, incomplete, partial, failed and unresolved outcomes without overloading legacy receipt meanings. A result-storage primitive validates integrity; Python chooses the business classification.

### Native reconciliation, qualification and future cutover

`./docs/reborn/post-turn-review-reconciliation.md` and `./crates/brassclaw_reborn_composition/src/completed_turn_review/reconciliation.rs` own the existing native follow-up. Real TaskControl root/child/host-future settlement, accounted exact native answers, confirmed provider outcome/usage, exact immutable candidate-ID readback and persisted/read-back proof with a retained owner are required before credit release. Unknown spend, lost owner, unsupported accountant, withheld answer or conflicting records remain unresolved. Operator observations and Python completion do not establish quiescence, zero spend or release. A recorded proof may exist without this call releasing retention. Failed/incomplete business outcomes remain unchanged.

`V35-16` qualifies and retains the successor exact graph without switching current review delivery selection with noncircular system-seed integrity/Q1/behavior, preserves overrides/old artifacts, and exercises all three consumers through real PG/kernel/global Monty. Installed edits retain authored review. Private review/validator usages stay out of public matching/MCP; public shared usages need real commands and qualified discovery. Native reconciliation and the already existing component-deployment bridge **do not complete conversion**.

Safety acceptance includes complete/missing/redacted/later-turn/failure-after-publication/multi-exchange/unknown-effect evidence; malformed/duplicate/source-like data; zero/one/many and invalid-later proposals before writes; base/dependency conflict; later write failure/unknown commit; duplicate/stale/cancelled occurrences; live block after prior item/wait; replacements during admitted work. Interruptions at intent, provider entry, response retention, candidate commit, acknowledgement and root settlement must never reset counts or replay original turn/model/writes. Inspect old and new evidence with the actual operator facade. Provider fixtures do not certify real-model quality.

`V35-28` switches only **future unreserved** source events, preserving the original unique event-delivery identity and atomic no-dual-consumption fence. Old V129 reservations/quarantine keep their selected graph/handlers. Comparisons before cutover are pure read-only retained-evidence transforms, never shadow model/effect calls. Rollback changes future selection only. Legacy wrappers/registrations/artifacts remain until no selected task/checkpoint/quarantine/native owner needs them.

## Connection and shared-facility consolidation

Single ownership is per common responsibility, not a universal ledger/protocol or removal of repeated checks at different trust boundaries. New shared durable APIs require **two real consumers with the same contract**, not similar record names. Work is an extension of existing infrastructure, not a Rust reliable-workflow Tool.

| Responsibility | Existing owner and consolidation contract | Required evidence / work |
| --- | --- | --- |
| PG parse/TLS | `./crates/brassclaw_pg/src/connection.rs`; shared pool and dedicated sessions | URL/keyword/socket/hostaddr/mixed/query-host/cardinality, conservative locality, remote Disable rejection/Prefer→Require, explicit local TLS, trusted/untrusted/wrong-host certificates, native/bundled-root handling; no certificate bypass. `V35-21`. |
| PG capacity/lifecycle | Instance composition injects compatible clones; event `SharedPool`; dedicated LISTEN session owner | Separate credentials/DBs stay isolated but share policy. Aggregate ordinary/admin/migration/dedicated reservations versus embedded capacity (16 default pool against 20 is not a whole-instance budget). Standalone utilities need not boot Monty. Startup Ready ordering, drain-before-PG-stop, bounded pressure/loss and no alternate authoritative file/in-memory store. `V35-21, V35-29`. |
| Authentication/admission | Existing operator boundary, `NativeProductAdapterRunner`, `ProductWorkflow`, durable idempotency ledger and turn store | Inventory CLI/WebUI/non-webhook/feature-gated adapters/triggers separately. Preserve signatures, external identities, origin/CSRF/body/rate/egress/secret/sandbox/network rules and ack timing. Inbound dedup is not downstream effect dedup. `V35-22`. |
| Live dispatch policy | `./crates/brassclaw_authorization/src/instance_policy.rs`, stable policy and retained capability boundary | Native/retained/extension/applicable administrative calls reach one current stable Tool rule across all versions/aliases. Bind/approval/claims grant nothing. Remove legacy role/operation leases only at coordinated v3 cutover; keep technical and external auth/Q1/Q2. `V35-23, V35-29`. |
| Worker communication | `./crates/brassclaw_monty_host/src/transport_actor.rs` and supervisor | One correlation/inbox/receipt/frame/credit/control-lane/stop-deadline owner. Lost observer, abandoned caller, transport loss, worker failure and persistence outage have distinct continuation/cancel/recovery outcomes. Socket loss is not cancellation/no-effect proof. `V35-24`. |
| HTTP/network | Existing `ReqwestNetworkTransport` and product egress | Share compatible clients, preserve protocol-specific auth and DNS/destination/secret/response-size enforcement. Reconnect never authorizes body/mutation replay. `V35-24`. |
| Observer events | Existing durable event/projection logs | Retained cursor replay or explicit gap/snapshot-rebase, zero task resubmissions. Observation logs are not Tool-effect ledgers. `V35-25`. |
| MCP | Existing server/catalogue/chat bridge/provider gateway | Server always running; connect/advertise after final prefix addition immediately before provider send; disconnect only this request on complete answer or reconciled failure/cancel, not streaming progress. Refresh on startup/restart/qualified catalogue changes; retain in-flight advertised contract. Listed completed command → new ordinary chat → correlated terminal result → supported close; no Python/direct execution/per-call components or Q1/Q2. `V35-25`. |
| Durable common mechanics | Existing inbound/turn/Monty invocation/review stores | Guarded reservation, exact answer retention/readback/CAS only where genuinely shared; domain tables/identities/settlement/release remain distinct. Positive counts and intent before effects, immutable selections/arguments/key, private references, monotonic settlement. Lease expiry permits inspection, not redispatch. `V35-26`. |
| Live settings/provider data | Existing revision persistence + actor acknowledgement; `PgProviderRepo` under simplified-v3 §10 | Desired/effective/pending/failed uptake visible; provider definitions DB-only, no env/JSON registry/reactivation. Compute time, external timeout, transport deadline, claim lifetime and memory/frame/model limits remain independent. Remove only truly duplicate constants. `V35-27`. |
| History/upgrade/recovery | Existing migration/supervisor/retained native owners | Successor migrations preserve IDs/counts/raw bytes/outcomes/uncertainty; never edit applied migrations. Keep compatible artifacts/observers. Fence fatal old generation, close admission and reconcile before supervised replacement; no parallel VM/per-chat/Tier-2 fallback or whole-Recipe replay. `V35-26, V35-27, V35-29`. |

Retry follows the exact Skill failure contract: only explicitly eligible `confirmed_no_effect_transient` or `unknown_completion` with trusted read-only/durable dedup evidence; original arguments/key and positive cumulative counts survive waits/reclaims; recheck policy/cancellation/freshness/resources. Timeout alone proves nothing. Completed effects survive malformed outputs, later errors and reply failure. Operation-specific inspection/reconciliation remains an explicit usage/Recipe decision with native proof/accounting enforcement.

Connection acceptance (`V35-21`–`V35-29`) covers fresh startup/shutdown; all policy forms/certificates; pool pressure/loss; committed-but-lost-ack exact readback; duplicate inbound/expired reservation; multiple protocols performing same action; reconnect/gap with no resubmission; lost caller/worker answer and withheld capacity; live block; stale/cancelled/competing owners; allowed/rejected retry; post-effect malformed output/reply failure; fatal generation fencing; catalogue/provider/settings edits during work; exact MCP lifecycle; old/unresolved records after upgrade/rollback; native-accounting reconciliation. CLI, WebUI and external real callers are separately identified where paths differ.

`V35-29` converts one coherent responsibility/all affected consumers at a time, compares pure/read-only decisions only, switches future admission with no dual owner and preserves in-flight identity/custody. Rollback changes future admission/configuration, never replays uncertainty or resets counts. Duplicate builders/retries/policy resolvers/buffers/constants/recovery wrappers are deleted only after a retained-consumer/contract map proves safe removal; justified exceptions remain documented. Reduction is measured in competing ownership/policy implementations, not Recipe step count. Performance needs representative measurements.

## Ongoing-plan handoffs, not parallel implementations

| Owning plan | Input/output handshake and required gate | This roadmap's boundary |
| --- | --- | --- |
| `./simplified_v3.md`, latest `./docs/plans/simplified-v3-implementation.md` | `V35-01` reconciles exact current source/receipts; `V35-14, V35-23, V35-24, V35-26, V35-27, V35-29` use its global lifecycle, authority, resources and Phases 0a/3a/7/8 production/cutover gates | No new VM/admission/settings/provider/recovery owner. Default token budgets remain disabled; 512 MiB startup/manual floor and live desired/effective revisions remain; task compute default 600 executing seconds excludes idle/waits and global VM lifetime. Allocation-count retirement follows actual Monty migration, not silently ignored settings. |
| `./validator_v3.md` | `V35-03, V35-04` coordinate pure approval schema/writers/readers; `V35-17, V35-18, V35-19` supply actual class routes/full subject/evidence/ordinary admission and activation handoff | Eight routes **0,1,2,3,13,21,22,23**; roots 10/50 separately protected. Full candidate/base/dependency subject differs from selected validator manifest. Each required rule exactly once; unknown/skipped/interrupted never pass. Human Q2 cannot be model-written; bootstrap cannot self-approve. |
| `./docs/plans/monty.composition.md` §35 and its implementation/coverage/recipe/skill-recipe appendices | `V35-01, V35-05, V35-09, V35-13, V35-14` resolve current canonical identities, exported usages/commands, adapters, typed layouts and flow gates | §35 supersedes early activation-absent snapshots. Templates are not allocated records; old reply/history warnings need exact successor evidence. No second loader, manifest store or migration. |
| `./docs/plans/prefix_v3_upgrade.md`, `./docs/reborn/prefix-source-compiler.md` | `V35-20` reconciles current compiler/bootstrap/behavior writer/reader bytes, ownership and applicable receipts; required shared-source/model/pure approval handoffs return to original owner | Separate quarantined source-only compile, installation qualification, durable Generate, model design/tokenization, validation/publication/activation, quality/cache. Source selection/preparation → compile orchestration → validation → publication are components. A bounded compiler operation returns artifacts, not a generation workflow. Distinct flat hash stages do not prove foreach. |

The latest Phase C continuation records the accepted closed installation slice in `./target/validation/prefix-registry-artifacts-p6m1aifn/accepted.json`: exact packaged graph/native/worker integrity, committed intent, actual eight-case behavior, coherent readback, canonical Retain approval reuse, atomic hash-usage approval, zero-dispatch proof reuse and incomplete-intent rejection. Delivery review found that local receipt present with `result: passed`; this is attributed frozen acceptance, not a rerun or certification of current merged bytes. Older writer-under-acceptance notes in the inventory/specification are superseded for this bounded status, not erased. `V35-01, V35-20` must reconcile applicability before further work. Production durable Generate/internal-completion admission remains the existing owner’s next distinct gate.

The prefix owner retains six-profile/custom-profile Generate, Sources, Redesign/Create/Model Designs and real serving/cache acceptance. This roadmap does not take them over or claim them complete. Missing model readiness cannot block a valid model-free source artifact; source-only acceptance cannot certify Generate activation. Retain immutable source/model originals and exact generation receipts. Shared serializer/iterative-value and kernel-output local results do not justify lifting strict parser/depth guards or skipping public DTO/journal/child/wait/live-profile/platform checks.

## Guidance reconciliation and semantic review

`V35-02` shortens entry routing in `./AGENTS.md`, `./CLAUDE.md`, `./README.md`, `./.claude/rules/tools.md` and `./crates/AGENTS.md`; canonical semantics stay in the four guides and development procedure in the existing policy/skill. Local applicability labels distinguish retained ordinary execution, compatibility `compose_program`/`PgCompositionPort`, historical evidence and target gaps. Source substitution, first-body selection, fresh-step state, empty bindings, retired intrinsic dispatch, filesystem Skill loaders, Domain-Skill hierarchy and invocation approval leases cannot remain default prescriptions behind distant banners. Preserve real compatibility warnings/history. Remove `./docs/plans/zencoder-extension-plan.md` from the default example route until migrated. Source comments in `./crates/brassclaw_engine/src/types/ibs.rs` and `./crates/brassclaw_engine/src/memory/composition.rs` identify the actual API path. Keep `./.agents/skills/reborn-feature/SKILL.md` and `./.claude/skills/reborn-feature/SKILL.md` byte-identical if changed.

The main `./recipe.md` example becomes the checked reference after `V35-06`, with actual preload/layout/association/publication records and fixture/prose drift checking. Before then it must be honestly labeled design-only, not falsely upgraded to qualified. Rust seeding declarative data remains legitimate. Optional seed-asset externalization is not a prerequisite; if undertaken separately it retains package integrity, noncircular qualification, upgrades/overrides and old artifacts.

`V35-08` installs a semantic review record for new Tools **and changed workflow handlers**: candidate Recipe, actual reuse search/identities, exact primitive versus shared-support gap, per-responsibility owner, retained identity/binding/approval route and ordinary-caller refusal/failure/no-replay evidence. Existing CI/architecture checks may flag missing records; regex, host-call counts and a Recipe row cannot certify semantics. Final review explicitly rejects successor Rust transcript/bundle policy, proposal-processing policy, multi-candidate effect loops, business completion decisions and hardcoded seven-operation enforcement, while verifying real native integrity/authority/accounting remain. A giant Python workflow or execute-anything Tool is not an acceptable replacement.

## Source-to-work coverage crosswalk

Work IDs below resolve to detailed executable steps in Zenflow, **not completion claims**. Dispositions: **E** existing bounded support to verify/reuse; **I** implementation/repair; **H** handoff to an existing owner; **C** retained constraint/acceptance. Dependencies are stated in each structured step; shared-file work must serialize even when the logical graph permits independence.

| Audit source | Disposition and work/evidence destination |
| --- | --- |
| F1 legacy path described as current | E/I `V35-01, V35-02`: retained-vs-compatibility source table, local passage fixes and scoped status links. |
| F2 incomplete main example | I `V35-03, V35-04, V35-05, V35-06`: general pure approval, real six-role graph, checked records, qualification/publication walkthrough. |
| F3 Tool guidance defaults to Rust/leases | I/C `V35-02, V35-08, V35-23`: three-way test, local legacy labels, no technical-policy bypass. |
| F4 repeated banners/stale bodies | I `V35-02, V35-06`: short routing, local history quarantine, source comments and synchronized skill copies. |
| F5 migration infrastructure exception | C `V35-02, V35-08`: distinguish task components, shared support and genuinely missing primitive; legitimate Rust seeds retained. |
| F6 status contradictions | E/I/H `V35-01, V35-20, V35-30`: owner-derived status/evidence index; latest appendices/journals, no duplicate status authority. |
| F7 wrappers hide Rust workflow | I/C `V35-09`–`V35-16, V35-28, V35-30`: real responsibility conversion, not native-reconciliation/deployment relabeling. |
| F8 reuse discovery/review gap | I `V35-07, V35-08`: owner-derived selectable inventory and semantic review gate. |
| P0 all six corrections | `V35-02` entry/rules/README/comments/history/skills, `V35-06` fixture-backed main example and actual authoring route. |
| P1 inventory/walkthrough/seeds/examples/readiness | `V35-03, V35-04, V35-05, V35-06, V35-07`; verified seed exception, exact authored review and supported adapter boundaries. |
| P2 specific shared support | `V35-03, V35-04` pure approval; `V35-13, V35-14` occurrences; `V35-09`–`V35-16` Sempai; `V35-17, V35-18, V35-19` validators; `V35-20` prefix handoff. |
| P3 six-part semantic enforcement | `V35-08`, ordinary-caller acceptance in each changed surface, final method/export review `V35-30`. |
| Reference §6 full acceptance | Every obligation maps in the reference acceptance table above; `V35-05, V35-06` owns the full ordinary-path test matrix, not just fixtures. |
| Audit §7 non-goals | C throughout: no Rust-data ban, trust removal, replacement global stack, prose execution/fields/multi-UUID shortcuts, relaxed qualification or deployment-by-document. `V35-30` rechecks. |

| Sempai source requirement | Disposition and work/evidence destination |
| --- | --- |
| §1 objective, three reuse forms, real additional consumers | I/C `V35-09, V35-10, V35-11, V35-14, V35-16`: stable shared identities, pure dependencies, repeated occurrences; no step minimization/giant workflow. |
| Step 1 baseline, seven operations, registrations/effects/reuse | E/I `V35-01, V35-09`: caller/contract/effect map and exact retain/successor/missing decisions. |
| Step 2 two Recipes, recursive schemas, bindings and tier | I `V35-09, V35-10, V35-16`: private Tier-1 review and model-free audit; actual IDs/layouts/failures. |
| Step 3 readers/model/single subject/parser/identity/policy | E/I `V35-09, V35-11, V35-12, V35-23`: existing stores/gateway, generic one-operation boundaries and preserved native checks. |
| Step 4 schema/IBS/occurrence/journal/isolation | I/H `V35-13, V35-14`: qualified flow storage and real executor/intent journal, legacy links and no duplicate-check shortcut. |
| Step 5 all transcript/correlation/raw evidence/completeness provenance | I/C `V35-10`: pure policy plus authoritative readers/storage, original boundary and audit reuse. |
| Step 6 claims/incomplete branch/shared requests/pins/layout/usage | I/C `V35-11`: shared construction and one-model usage with a genuine non-Sempai reviewer. |
| Step 7 strict whole-response parsing/generic preparation/all preflight/IDs | I/C `V35-12`: duplicate-aware rejection, exact raw list/checksum/base/dependencies, host identities, no proposed-code execution. |
| Step 8 independent item effects/partial/unknown/empty/ordered receipts | I/C `V35-15`, gated by `V35-14`: one candidate per occurrence, exact readback/no model replay. |
| Step 9 business versus native completion/Q1/Q2 | I/C `V35-15, V35-26`: pure result, immutable append, actual native custody and inactive candidates. |
| Step 10 identities/preload/approvals/seeding/private exposure | I/C `V35-16`: stable compatible successors, exact closure/artifacts, noncircular bootstrap, overrides/old pins. |
| Step 11 all positive/negative/interruption/operator evidence | I/C `V35-16`, tests also embedded in `V35-09`–`V35-15`; real PG/kernel/global ordinary caller and truthful model limits. |
| Step 12 pure comparison/future-unreserved switch/fencing/compatibility/rollback | I/C `V35-28`: unique event identity, no dual delivery or old-work re-admission, retained artifacts until no owner. |
| §3 all preservation requirements | C all Sempai steps; `V35-16, V35-28, V35-30` acceptance: one VM, genuine original No-Match, no original replay, exact pins, raw/effective bytes, typed data/live policy, durable counts/unknown effects, stop-only, unreviewed candidates, V122/127/129/133 retention, token-budget default and whole-value limits. |
| §4 all shared families | E/I library table above; `V35-09, V35-10` reads/transcript/envelope, `V35-11` messages/model, `V35-12` strict parse/preparation/preflight, `V35-15, V35-26` retention/aggregation/result append. |
| §6 final Rust/Python responsibility review | C `V35-08, V35-28, V35-30`: inspect every remaining method/export, preserve trust, eliminate five named hidden workflows. |
| §7 unchanged-by-plan | C this delivery; future steps remain pending, no runtime/deployment changes now. |
| Native reconciliation follow-up | E/C `V35-26, V35-16`: existing native proof/release path reused, lost/unsupported evidence unresolved; not conversion completion. |
| Component deployment follow-up | E/I `V35-04, V35-06, V35-07`: existing exact graph preparation/CAS reused; not Q1/activation by retention or Sempai conversion. |

| Connections source requirement | Disposition and work/evidence destination |
| --- | --- |
| §§1–3 one owner/three-way architecture/source targets | E/I/C `V35-01, V35-21`–`V35-27`: actual caller-to-owner map, preserve distinct auth/policy/claims/schema/clocks/protocol/effect meanings. Historical PG divergence reconciled, not reimplemented. |
| A all callers/builders/auth/claims/retries/deadlines/events/recovery | I `V35-01` global map, refined by `V35-21, V35-22, V35-23, V35-24, V35-25, V35-26, V35-27`; classify duplicate/conflict/justified boundary/gap and exact replacement evidence. |
| B parsing/TLS/certificates | E/I `V35-21`: existing implementation verified against driver semantics/all endpoint forms and real certificate outcomes. |
| B pools/CLI/capacity/reconnect/startup/shutdown | E/I/C `V35-21, V35-29`: shared injection/necessary isolation, aggregate reservations, dedicated sessions, no SQL replay/alternate store. |
| C adapters/workflow/auth/dispatch/cutover | E/I/C `V35-22, V35-23, V35-29`: real CLI/WebUI/non-webhook/feature coverage, signatures/ack/egress safeguards, aliases/live policy, coordinated legacy removal. |
| D actor/HTTP/disconnect/replay | E/I `V35-24, V35-25`: correlation/credits/control/stop owner, distinct disconnects, compatible client reuse and cursor/gap/no resubmit. |
| D MCP complete lifecycle and ordinary-chat boundary | E/I/C `V35-25`: one server, final-prefix advertisement, request-only cleanup, qualified refresh and immutable advertised contracts. |
| E shared primitives/two consumers/identity/settlement/retry | E/I/C `V35-26`: compare real stores, minimal shared mechanics with separate domain evidence/release, durable positive counts and exact retry contract. |
| E inspection/native accounting/fatal fencing | E/I/C `V35-26, V35-24, V35-29`: explicit Recipe decisions, retained real native proof, no note-based release/resume or replacement VM shortcut. |
| F components/snapshots/bootstrap/overrides | E/I/C `V35-03, V35-04, V35-16, V35-27`: exact exports/associations/task/artifact pins, distinct authored versus seed review and retained compatibility. |
| F settings/provider/migrations | E/I/C `V35-27`: acknowledged revisions/independent limits, DB-only provider owner, preserved original IDs/counts/uncertainty and successor migrations. |
| §5 acceptance startup/policy/pressure/lost commit | C `V35-21, V35-29`, real PG and product lifecycle. |
| §5 duplicate/expired/multi-adapter/observer | C `V35-22, V35-25, V35-29`, actual distinct callers, no downstream replay. |
| §5 lost caller/answer/live block/stale/cancel/retry/post-effect errors/fatal worker | C `V35-23, V35-24, V35-26, V35-29`, actual worker/kernel/DB and retained custody. |
| §5 edits/MCP/historical/native reconciliation | C `V35-25, V35-26, V35-27, V35-29`, pinned work/live policy/effective settings and unresolved native owners. |
| §6 cutover/deletion/completion/measurements | I/C `V35-29, V35-30`: all consumers per responsibility, read-only comparison, future-only switch/rollback, safe deletion map, two-consumer proof; no performance/exactly-once/resume/full-v3 inference. |

## Acceptance, dependency and retirement contract

| Requirements baseline | Work and delivery disposition |
| --- | --- |
| R1 coherent guidance/status | `V35-01`, `V35-02`, `V35-06`, `V35-30`; existing evidence is attributed, not relabeled deployed. |
| R2 qualified multi-step reference/reuse | `V35-03`–`V35-06`, reference acceptance table and successor/artifact checks. |
| R3 submission/deployment/discovery | `V35-04`, `V35-06`, `V35-07`, `V35-25`; existing store/facade/CAS path, no parallel service. |
| R4 actual workflow ownership | `V35-08`–`V35-20`, `V35-28`, `V35-30`; native trust retained, semantic review required. |
| R5 complete Sempai coverage | `V35-09`–`V35-16`, `V35-26`, `V35-28`, `V35-30`; all twelve steps/protections/follow-ups mapped above. |
| R6 shared connections/facilities | `V35-21`–`V35-27`, `V35-29`, `V35-30`; all A–F and acceptance/cutover distinctions mapped above. |
| R7 four ongoing plans | `V35-01`, `V35-03`, `V35-04`, `V35-13`, `V35-14`, `V35-17`–`V35-20`, `V35-27`; exact original owners/gates retained. |
| R8 safety/evidence/completion truth | All implementation steps own their tests; `V35-06`, `V35-14`, `V35-16`, `V35-23`–`V35-30` enforce the relevant ordinary callers and negative cases. |
| R9 absorption/retirement | Source-to-work crosswalk plus `V35-28`–`V35-30`; source plans are not obsolete during this delivery. |
| D1–D5 planning deliverables | Existing PRD/spec/inventory, this root artifact and Zenflow checklist; the separate delivery-review step still owns the final report. No runtime work is performed by this planning deliverable. |

The Zenflow implementation steps are intentionally **Pending** after the separate roadmap-delivery review, with auto-start disabled. Each carries prerequisites, affected files/outputs, contracts, positive/negative tests and evidence requirements sufficient for a fresh chat. `V35-01` rebases all handoffs before source edits. Guidance can become truthful early; the qualified main example waits for real acceptance. Pure approval precedes reference publication; flow occurrences gate candidate iteration, not pure extraction. Shared-file/approval/migration changes serialize with their existing owners.

All changed behavior is reviewed before executable checks. Each implementation owns its unit/regression tests, plus affected-package lint with correct features; persistence/security/lifecycle claims require real PG/native worker/ordinary product callers. Existing applicable results are reused with exact frozen source/dependency/artifact/configuration/migration/effective-setting identities and limitations. No empty/skipped test filter counts. No Cargo solely for this documentation. Before remote/provider work read `./LOCAL_TEST_ENV.md` if present; honor root disk/NVMe/cleanup/serialized build rules. Self-signed browser acceptance, when explicitly needed, is scoped to that browser context and never weakens PG/provider validation.

Final `V35-30` acceptance requires a caller-to-owner map with no unreviewed domain policy left in successor Rust handlers, actual shared consumers and immutable identities, functioning submission/qualification/publication/discovery, the accepted reference and Sempai workflows, all required validator/prefix handoffs, connection safety, future-only cutover/rollback, old/unresolved evidence/artifacts/native ownership preserved and unnecessary duplicate paths actually removed. Blocked shared prerequisites remain open with their original owner; they do not disappear because another workstream passed.

Only then may the two absorbed v3.5 plans be labeled obsolete with a link to this accepted successor and exact evidence. Historical receipts and intentional exceptions remain readable. The four ongoing v3 plans are not automatically closed. Performance, real-model teaching/cache quality, installed-platform acceptance, automatic crash resume and exactly-once external execution retain their separate proof obligations.

## Planning delivery review boundary

This artifact records source-grounded design and coverage, not performed runtime work. This step checks its paths, source assertions, crosswalk, structured-list agreement and whitespace. The next existing Zenflow delivery-review step independently verifies consistency and writes the delivery report; no future implementation is started by this planning step.

Planning-session checks on 2026-10-17: all thirty stable work IDs are present in this artifact; all cited existing file paths resolve and use the required prefix; no implementation checkboxes or competing checklist file were created; direct whitespace validation, `git diff --check` and the new-file diff whitespace check passed. Zenflow contains thirty added Pending milestones after delivery review; auto-start was explicitly set false and default executor selection was left untouched. Only this roadmap and structured plan were changed by this planning session; the earlier requirements-stage ignore-file update and all pre-existing runtime edits remain separate. No Cargo, database, provider, remote or runtime acceptance was run.

Delivery review on 2026-10-17 verified audit/proposal coverage, all thirty Pending structured milestones and the eight unchanged source drift keys. It clarified empty-literal semantics, component-slice versus full-flow acceptance, qualification versus future-only delivery cutover, and the latest closed compiler installation receipt. See `./.zenflow/tasks/strictly-follow-the-following-wo-6326/report.md` for checks, exact review changes and limitations. No future implementation was started; both absorbed source plans remain not-yet-obsolete.
