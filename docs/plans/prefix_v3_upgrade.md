# Prefix v3 upgrade: compiler, source catalogue and WebUI

> **Current status — 2026-10-27:** use [the combined implementation and ownership
> index](../implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


## Binding preloadable Skill interface (v3)

A Skill is one Tool-usage pattern with prose and explicitly associated PythonCode
exposing a preloadable function interface. Declare public names/signatures,
private helpers/constants and dependencies. Resolve one approved catalogue
snapshot; pin exact interface/code/association/artifact revisions and export
resolution. Load definitions in deterministic dependency-first order, rejecting
cycles/conflicts and effectful initializers. Invoke the pinned export on demand
with typed data; loading is not an invocation or a new Recipe effect step.

Preloaded code does not automatically enable a Tool. Its matching ToolSkill
binding and current kernel checks still apply before every actual dispatch.
Reusable code and immutable constants may be shared; mutable arrays, defaults,
closures, inputs and results remain isolated per task/attempt/invocation. Running
and resumed tasks keep their selected exports when new revisions activate.
A Skill needs no dedicated Recipe or matching command to exist or be complete.
Many Recipes may reuse the same approved usage. MCP exposure requires a separately
qualified MCP-call Recipe/variant; reference/test workflows do not imply ownership.
MCP tools/list derives from available approved mcp-call-skill-recipes, not raw
Skill rows. Keep the server always running; Kohai connects/advertises to the
provider after final prefix addition just before sending a prompt, then
disconnects that request on the complete answer. Refresh discovery at
startup/restart and qualified Skill/Recipe catalogue changes. Existing calls
keep their advertised contract and normal chat task snapshot.
MCP tools/list gives its exact sentence, variable positions/types, escaping and
valid examples; the model sends the completed command for intent matching.
MCP accepts the completed listed command, opens a new ordinary chat, sends it
as a user message, forwards the correlated chat result and closes the chat.
It accepts no Python and has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](../../skills.md#preloadable-function-interface-binding-v3-target).
Selected retained loader/store/runner support is implemented; this contract
does not certify every workflow or the complete migration. See
[the current implementation status](../implementation-status.md).
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


Status: **partially implemented; full product cutover is not complete**.
Current source/evidence reconciliation: **2026-10-27**. The workspace declares
`1.7.0-rc.4`; exact source/artifact manifests, not that version alone, identify tested
implementations. This reconciliation records scoped local implementation and validation. It does
not certify deployment, model-design acceptance, prefix activation or an
inference-service change.

The opening status and remaining-work sequence below govern what to do next.
Sections 4–10 define the required contracts; section 11's dated entries are the
implementation/evidence journal for their exact frozen inputs, not a list of work
that must all be repeated. Earlier “pending” statements in those entries are
superseded only where the current status explicitly records their successor.
A locally accepted prerequisite does not establish full-plan or deployed support.

### Binding operator correction — 26 October 2026

The [four binding rules](../reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26),
explicitly restated and clarified by the operator on 28 October 2026, govern every
generation, design, review and serving step below:

1. Generate the finished prefix specifically for the concrete provider/model pair,
   already formatted for that pair, and store it immutably.
2. Generator and Kohai consume the same stored, accepted design revision for that
   pair. Sempai consumes that revision too when the same pair is its provider/model.
3. Kohai first formats the dynamic Orchestrator prompt for the final target pair.
   If Sempai is connected/enabled, it receives that already formatted prompt for
   optimization/correction and returns the final dynamic prompt to Kohai. For its
   own call, Sempai uses its own prefix generated for its own target-pair design.
4. Kohai then inserts the intended finished prefix unchanged and exactly once
   before the final dynamic prompt. No formatting stage follows Sempai's return.

Existing acceptance requirements below must verify this order; historical
examples and receipts do not establish a competing contract.
Use the [explicit two-request handoff](../reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests)
for compiler, Kohai and Sempai integration. Sempai's own request framing never
introduces a second formatting pass over the returned final-target prompt.

**Request-owned role prefixes only.** The operator reports removing the global
Sempai `--chat-template` override on the Ornith test server. That model serves
multiple roles, each with its own prefix. BrassClaw production cannot access
provider configuration files; installing/changing a provider template or service
configuration is therefore **not a supported prompt-configuration option**.
The test target uses the model/provider's default template. This is the operator's
reported change, not a new API/runtime measurement or permission to access the host.

**Correct ownership: generate the prefix once; format only the dynamic prompt
at runtime.** The generation process may share reusable stages, but its output
is an already formatted, immutable **provider/model-specific prefix** for the
selected role/task/composite. It is not a model-neutral artifact awaiting runtime
formatting. Preserve its exact character sequence and UTF-8 payload hash.
Provider/model-specific prefix rendering remains part of the compiler contract;
the earlier instruction to remove it was an incorrect interpretation and is
withdrawn. No implementation may rely on that withdrawn instruction.

Kohai receives the Orchestrator's dynamic prompt, including the relevant history,
tools and task data. Kohai first formats **only that dynamic prompt part** for
the final provider/model pair using its stored accepted design. If Sempai is
connected/enabled, Kohai sends this already formatted dynamic prompt for
optimization/correction. Sempai uses its own prefix, generated for its target-pair
design, for its own call and returns the **final dynamic prompt** to Kohai.
Kohai does not format that return again. It then adds the intended already
generated matching prefix, unchanged and exactly once, using the
qualified target joining contract. This is the operation order; the prefix is
placed before the dynamic prompt at the contract's insertion point, not appended
as an afterthought at the end. Kohai must not regenerate, normalize, trim,
summarize, re-escape, reorder or reformat the prefix itself. Lossless transport
encoding is allowed only if the decoded/model-facing prefix characters are
preserved. Frozen final assembly, measurement and submission follow that join.

The generator, Kohai and Sempai whenever it targets that same pair must resolve
the **same stored, accepted, pinned design revision for the provider/model pair**
from the existing design store.
That design supplies generation-time prefix rules and the compatible runtime
dynamic-prompt format/join contract; recording a formatter identifier without
Kohai consuming this design before optional review is insufficient. Generator,
Kohai and Sempai share this accepted revision when they target the same pair.
Do not introduce a separate mutable Kohai format configuration. Sempai's own
review call independently uses its selected
provider/model pair's pinned design and finished reviewer prefix, with its own
complete-request guard. Its output can correct the dynamic prompt, not replace
the final target's design or prefix. Resumption retains both selections.

**Pair-owned design, role-specific content (operator clarification):** the design
primarily defines provider/model limits and formatting: prefix/prompt ceilings,
compatible prefix-generation and dynamic-prompt formats, supported documented
standards, framing/join rules and relevant tokenizer/cache facts. It is shared
by all roles using that provider/model pair. If Kohai and Sempai use the same
pair, they resolve the same accepted pair-design revision, not separate role-owned
format designs. Their content/coverage profiles and generated prefix artifacts
remain distinct; the Sempai prefix must first be generated specifically for that
pair just like every other prefix. Different pairs resolve their respective designs.
Limits remain manually editable through the canonical per-pair settings owner;
immutable design records reference that policy rather than own duplicate values.
Provider metadata/documentation can populate evidenced design facts through
supported APIs, or an operator can supply them; this never requires provider host
configuration access or a server template/service change.

**Default selection and content separation (operator contract):** `base-prompt`
is the normal prefix whenever the retained Recipe, Orchestrator instruction or
role specifies no particular prefix. No-Match therefore normally selects base;
base is not Tier-2-only. An explicit role prefix such as Sempai replaces that
implicit choice. Conflicting explicit selections require the existing declared
routing resolution or an error; do not invent an overriding priority. Missing or
incompatible explicitly selected content blocks the call instead of silently
selecting base. Base readiness is required for calls that actually select base,
not for unrelated model-free work.

Pair design, profile/content rules and finished generation are separate contracts;
see [the shared contract](../reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26).
A role/profile can reuse shared source knowledge or explicitly require a composite,
but does not automatically inherit the whole base prefix. All roles using the
same provider/model pair share the same accepted pair design and canonical current
limits; their content rules and generated text remain role-specific. Historical
profile-bound design-proposal schemas remain evidence/current storage facts, not
authority for a second mutable role-owned format/limits design. Extend the existing
owners with typed references and compatible readers where migration is needed;
do not assume these explanatory contracts are already accepted API/SQL fields.

Generate/Regenerate captures the role/task, explicit provider/model target,
model/tokenizer specification, accepted prefix-format/design revision, canonical
size policy, and complete required execution/authoring knowledge. Use the
connected target **or the model explicitly selected in the WebUI even when it is
not an active role selection**. The design provider is independent. Selecting
another generation target must not change active models or silently borrow their
format/limits. Use accepted target-specific formatting and whole-unit selection
to fit the fullest useful reviewed content, preserving mandatory knowledge and
originals. Do not add filler to fill a token ceiling.

The target-specific immutable prefix and the separately formatted prompt retain
their own identities. The joining contract pins insertion layer, role/protocol
framing and separator ownership, with no ad-hoc prefix edits. Validate no doubled
chat markers or duplicate reference insertion under provider defaults. Measure
the **actual combined request** after all required lossless serialization/default
render mapping, including both caps, framing, history/tools and effective output.
Counts of independently tokenized parts need not add up. The model-facing order
must preserve **fixed prefix followed by the already formatted Orchestrator prompt,
including its dynamic tools/history**. Do not apply another whole-request template
after joining that moves dynamic data ahead of the prefix or changes its text.
If a transport/default mapping cannot preserve this contract, that target path
remains unqualified/blocked until a correct adapter is available. Locating a
non-leading span is diagnostic evidence, not an alternate serving architecture.
The configured prefix ceiling (100,000 for the requested Ornith policy) remains
an allocation limit, not an LCP or a promise of cache residency. Neither reduce
that ceiling nor infer cache loss from the discarded whole-render interpretation;
actual token-prefix identity and cache behavior still require separate evidence.

Historical template-export artifacts and their receipts remain readable as
evidence; they cannot be newly installed/activated or offered as production
configuration. The old `inject_reference`/supplied-template measurement pipeline
and its six-context 2,179-token leading-prefix result describe the custom-template
path, not qualification of default-template request-owned serving. The physical
partition, leading-byte verification and policy arithmetic primitives retain
their actual mechanical scopes. They do not close this changed production path.
This correction governs the remaining steps and historical journal entries below.

### Current delivery status — 2026-10-22

**Worker evidence correction — 26 October 2026:** the historical 530-file
reuse inventory omitted actual local build dependencies. Its recorded hash
checks do not prove complete unchanged-worker source applicability. Preserve
observed historical results and immutable receipts, but do not reuse that
incomplete closure. The physical-partition successor built its worker freshly;
[fresh current native requalification](../validation/2026-10-26-prefix-leading-chat.md)
now covers the exact affected export/source/archive/selection/budget cases with
a newly built worker. This does not certify every older receipt or unexecuted
consumer. See
[the exact scope and required requalification](../validation/2026-10-26-prefix-worker-evidence-review.md).

| Area | Implemented and evidenced boundary | Still required for completion |
| --- | --- | --- |
| Profile registry / startup | Six first-party profiles, revisions/metadata and installation-owned registration in `prefix_profile_seeds.rs` / `InstalledMontyCatalogue`; operator edits survive reseeding. | Complete custom-profile creation and accepted compiler/design bindings. Metadata alone enables no workflow. |
| Legacy serving | Scope bundle, existing base-only assembly Recipe, fencing and compatibility reader remain. The ordinary list labels this `legacy-unverified`; other profiles stay blocked. | Replace ordinary generation/serving through the qualified named/composite path; remove compatibility fallback only after consumer cutover. |
| Offline compilers | Base and Prefix-Maker entry points exist, shared evidence/rendering helpers and deterministic source packages exist; domain/Sempai standalone citation, configuration and teaching helpers are retained. | Shared-engine capability/validation parity for Defensive, HA and tomedo; no blanket qualification of all standalone modes. |
| Source/catalogue capture | Owner-selected coherent export, exact component revisions/metadata and installation-retained core/declared-release documents; explicit private prefix workflow sources are included by supported export. [Acknowledged export capacity](../validation/2026-10-25-prefix-export-capacity.md) now uses the existing `execution_limits.max_catalogue_bytes` owner, counts complete serialized JSON without a second full buffer and rejects whole oversized exports; actual runtime/PG boundary and strict checks pass. | Prove complete installed library/private dependency coverage and eligible registered domain-source inclusion, with no row caps or omitted required records. Legacy sweep still has `LIMIT 1000` and summary preference. |
| Generation storage | V142 immutable originals/artifacts/maps and quarantined imports, generation list/detail/download and legacy archive support; existing generation-retention primitive. | Accepted target artifacts, lineage/invalidation, publication eligibility and consumer selections. Retention/import is not activation. |
| Recipe compiler / bootstrap | Reusable typed Python stages, shared segmentation/renderers, JSON digest usages and Retain; installation-owned exact graph/integrity/behavior qualification. | Qualify successor graphs/options in the same owner; no expansion of the closed compiler approval exception to arbitrary pure code. |
| Durable source-only jobs | V167/V171 metadata uses existing turn admission/global Monty/native custody; base/Prefix-Maker and v5 Sempai source-only capture/build/reconnect are locally accepted. | Ordinary one-click target-aware Generate, remaining profile adapters, parent/composite jobs and complete cancellation/recovery acceptance. No second queue/executor. |
| Stable presentation | Versioned literal/deduplicated/trailing/progressive/context-aware v1–v5 layouts, complete origins/maps and original bytes retained; v5 source-only native acceptance. | Accepted per-model budget-dependent teaching/selection designs and actual serving/quality qualification. Source-only layout options are not accepted model designs. |
| Model diagnostics | [Exact retained formatting sources](../validation/2026-10-26-prefix-format-sources.md) now expose complete standalone/named templates, tokenizer/special-token maps and declared memory fields through the same inspector/read owner (eight fresh Rust cases including actual native PG, six UI cases); this remains source-only evidence, without effective-template/default selection. V145 model/specification/proposal storage, V157 immutable local artifacts, bounded config inspector/API/UI and real retained encoding worker; full token/HF comparisons, exact template identities and durable measurements. [Versioned physical-span attribution](../validation/2026-10-26-prefix-token-partition.md) now has native/HF/bootstrap/journal evidence; supplied byte spans remain semantically unqualified. [Leading-chat rendering](../validation/2026-10-26-prefix-leading-chat.md) now verifies expected leading bytes in the complete render and binds ordered context identity, with real injection/HF/native/bootstrap/journal evidence. [Grouped quantization declarations](../validation/2026-10-26-prefix-grouped-quantization.md) also have nine-case/native-PG scoped acceptance in this inspector. | Verified deployed identity/geometry, semantic prefix/dynamic partition, accepted designs/bindings and runtime compatibility. Config declarations are not deployed facts. |
| Capacity observations | The latest [K1 complete Sempai diagnostic](../validation/2026-10-24-k1-prefix-request-identity.md) has a largest probe of 371,462 tokens and remains oversized/unactivated. Earlier native base 141,978, Sempai 345,129 and standalone Sempai 334,756 observations belong to different preserved captures. | A reviewed design that fits the chosen target; do not clip originals, treat counts as constants, or substitute config position capacity for prefix allowance. |
| Per-model hard settings | `ProviderDefinition.model_token_limits[model]`, existing provider definition/revision/CAS and GET/POST provider API; same editor/query in Inference and Prefix, stale-edit protection and model-selection refresh. Real PG service tests and 37 frontend helper/state tests pass. [Original model-policy integrity](../validation/2026-10-27-prefix-model-policy.md) now checks recorded model-snapshot protocol/context/limits against the exact historical provider revision on capture/read/retry/catalogue, caching each revision per read. Three native PG cases plus two existing declaration and two actual Pair regressions pass; historical absent limits stay Unknown. [Scoped final-SDK transport](../validation/2026-10-28-prefix-final-request-guard.md) now checks exact prepared-body custody, mandatory live comparisons and native no-send evidence, with sixteen actual regressions and strict checks. Its counters are declared test fixtures, not a real-model oracle. | Installed-instance API upgrade/write/readback, external CLI/server notification uptake, canonical planning/admission and both-target resolution, unconditional final dispatch guards. Original-snapshot integrity is not live dispatch/capacity approval. No duplicate mutable prefix settings store. |
| Pure budget planning | `model_budget_draft` / `prefix_model_budget.py` calculates effective budget/shortfalls from pinned explicit policy/reservations; actual Monty and typed-contract checks pass. | Canonical policy read/identity qualification and integration into generation/design/serving Recipes; exact rendered fit, quality and activation are separate gates. |
| Budget-dependent reference selection | `prefix_design_selection.py` and `prefix_design_components.rs` provide reusable namespace/whole-unit generation rules, separate rule/resolved-plan identities, mandatory coverage, dependency/priority candidates and exact-measurement selection through existing digest/encoding usages. New components can resolve under unchanged rules. [Installed captured-source execution](../validation/2026-10-27-prefix-captured-design.md) now retains/inspects both variants and runs the complete captured variant on the already-running global Monty service: actual core-source capture, native journals, independent every-band floor, selected/blocked outcomes and weakened-band rejection pass, with unchanged root identity. The [budgeted successor](../validation/2026-10-28-prefix-budget-generation.md) composes the existing pure planner into that same Recipe, resolves verified original model-policy inputs and exercises ten native singleton scenarios with optional token budgets disabled. See [the contract](../reborn/prefix-budget-design.md). | Trusted accepted-design/live canonical-policy admission bindings, complete source eligibility, workflow/bootstrap qualification, ordinary draft/Generate admission and retention, transport-sized batching, semantic envelope fit and quality/activation remain open. Private candidate execution does not grant approval or activate a workflow. |
| Sources management | V143 immutable registrations, used-generation inventory, pending registration API/UI and green/red/neutral presentation contracts exist; checks remain disabled. | Admitted checks on panel open, qualified observations/projection, collection/review and eligible next-build inclusion. Saving a URL does not add reviewed knowledge. |
| Source-check prerequisites | Existing HTTP-family successor, exact-content classifier and `builtin.prefix_source` read/append usages have native/preload/PG evidence. The [registered-source Recipe variant](../validation/2026-10-25-prefix-registered-source-check.md) now connects pinned read, stored URL, bounded fetch, hash/classification and immutable diagnostic retention in 16 explicit steps; seven fresh Rust cases and strict affected/default checks pass. | Qualify/seed the exact graph, integrate used-generation originals through their existing owner, admit durable source jobs and project trusted observations into the UI. Stored diagnostics remain **unqualified**, not availability/review approval. |
| Sempai reference | Installed v5 source-only selection retains binding/compatibility/procedure guides and pinned Monty model references; complete originals and no-replay native measurements retained. | Fit/qualify its actual composite, successor review/request contracts, both-target limits, production artifact validation and held-out reviewer/proposal behavior. |
| Sempai infrastructure | Existing post-turn consumer, immutable proposal/deployment and native effect/accounting reconciliation are reusable infrastructure; semantic conversion remains owned by `sempai_v3.5.md` / root `path_to_v3.5.md`. | Consume those shared reusable components; do not recreate their Rust workflow policy in prefix jobs or count infrastructure acceptance as reviewer quality. |
| Optional model documentation — scoped acceptance | [Model-card evidence](../validation/2026-10-27-prefix-model-card.md) permits complete optional `README.md` through the existing artifact importer/owner and exact null-pointer Pair references. Hash/UTF-8/byte integrity, JSON-pointer and template-promotion rejection, real HTTP/PostgreSQL readback, original-template/native comparison and durable no-replay outcomes pass. Browser helper/notice, artifact guide and shipped Prefix-Maker teaching reference are aligned. Six Rust/integration, three import and one offline compiler/export test pass on frozen current sources, with strict affected/default lint, format and whitespace checks. Failed Prefix/shared predecessors remain preserved. | Broader official provider/model documentation collection and trusted URL/revision/applicability/licensing provenance, content review and machine-ready design selection. Retaining a README does not verify its publisher/facts, effective cache geometry or serving identity, nor accept a model design or enable Generate/Redesign. Related quantized/upstream research originals are archived separately without automatic ingestion. |
| Shared pair construction drafts | [V180/source-bound pair drafts](../validation/2026-10-26-prefix-pair-construction.md) extend the same immutable proposal table with a role-independent format, exact original/template/evidence bindings, literal cases and operator API/UI import. Six fresh Rust cases (including actual public HTTP/PG and DB acceptance-FK rejection), eight UI cases and offline export pass. | Broader archived provider/documentation evidence, registered deterministic machine-ready construction options, independent tokenizer/provider case qualification, trusted acceptance/bindings and compiler/Kohai consumers. Drafts remain unaccepted; no generation/serving enablement. |
| Complete pair worked cases | [Canonical case export and comparison candidates](../validation/2026-10-26-prefix-pair-cases.md) preserve every saved context and expected string through the existing namespace-bound proposal owner/operator GET. Reusable PythonCode and a six-step Recipe reuse exact UTF-8 digest and original-template encoding usages. Four independent HF contexts match the real retained-artifact worker/Monty. The [scoped bootstrap successor](../validation/2026-10-26-prefix-pair-bootstrap.md) now qualifies the exact installation-owned graph. | The [ordinary private](../validation/2026-10-27-prefix-pair-private-admission.md) and [operator API/UI](../validation/2026-10-27-prefix-pair-operator.md) successors now connect this exact program to shared ResultOnly admission and public diagnostic submit/status. Accepted provider/model formatter, final wire, context/cache and answer-quality gates remain open. |
| Complete pair Recipe/journal prerequisite | [Closed whole-Recipe execution](../validation/2026-10-26-prefix-pair-journal.md) exercises actual global Python, retained preloads, native encoding, kernel and PostgreSQL journals. At that baseline the source-owned generic captured-case verifier was test-only; the [installed finite bootstrap verifier](../validation/2026-10-26-prefix-pair-bootstrap.md) separately qualifies its exact packaged workflow with independently known fixtures and durable observations. | The [ordinary private successor](../validation/2026-10-27-prefix-pair-private-admission.md) promotes/reuses the complete source-owned capture/journal checks, actual per-operation preparation acknowledgement and step bounds. The [operator diagnostic successor](../validation/2026-10-27-prefix-pair-operator.md) now supplies public submit/status; accepted designs remain open. |
| Shared pair job target metadata | [V181/explicit job targets](../validation/2026-10-26-prefix-pair-job-targets.md) extend the same immutable definition table and Rust owner with Profile/Pair targets. Only `check_cases` may omit both profile fields; captures equal the complete source-owned case packet. Retry/selection retention, changed-input and concurrent-target conflicts, namespace/partial/settings-copy rejection and historical DDL preservation pass in real PostgreSQL. The legacy Prefix flow passes again in the bootstrap successor. | The [private ResultOnly successor](../validation/2026-10-27-prefix-pair-private-admission.md) now passes ordinary admission/selection/settlement; the [operator successor](../validation/2026-10-27-prefix-pair-operator.md) adds pair-aware public diagnostic status/UI. Metadata alone grants no execution. Reuse the qualified exact program, without another queue, profile sentinel or copied limits. |
| Pair comparison bootstrap | [R4 scoped acceptance](../validation/2026-10-26-prefix-pair-bootstrap.md): the exact six-step packaged graph and shared workflow-evidence store qualify before ingress. Six independent fixture outcomes, actual native/PG journals, captured VM/step bounds, forged evidence/source and incomplete-intent rejection, retained no-replay read and the full legacy native Prefix regression pass. Three selected integration tests, strict affected/default lint and format checks pass; R1–R3 failures remain preserved. | The [ordinary private successor](../validation/2026-10-27-prefix-pair-private-admission.md) now qualifies admission/selector/settlement; the [operator successor](../validation/2026-10-27-prefix-pair-operator.md) adds public diagnostic UI. This qualifies one exact installation-owned workflow, not arbitrary pure code, an accepted model design, serving/cache/capacity or answer quality. |
| Private pair ResultOnly jobs — scoped acceptance | [R5 native acceptance](../validation/2026-10-27-prefix-pair-private-admission.md) reuses immutable definitions, the qualified program and ordinary private admission/selection/settlement. The complete source-owned verifier checks actual native journals and preparation acknowledgements. Selective trusted-source step capture supplies the required bounded observations. Positive and mismatch jobs complete/settle; missing leading prefix fails/settles. Original reconnect does not replay; conflicting/foreign inputs are rejected. Both Pair tests and the complete legacy Prefix regression pass, zero ignored, with strict affected/default lint and format checks. | The [operator diagnostic successor](../validation/2026-10-27-prefix-pair-operator.md) closes the public facade/status/UI source path. Interactive browser acceptance, accepted model designs and their compiler/Kohai consumers remain open. This private receipt alone makes no public, serving/cache/context or answer-quality claim. |
| Pair operator diagnostic API/UI — scoped acceptance | [R3 acceptance](../validation/2026-10-27-prefix-pair-operator.md) supplies authenticated body-free submit and read-only status over the existing qualified Recipe/immutable definitions. One discriminated Profile/Pair status retains existing profile JSON fields without a Pair sentinel. Each retained pair proposal has a comparison panel: original-ID reconnect, read-only refresh, stale-result fencing and explicit new-job action after durable settlement. Actual HTTP/PG/Monty positive/mismatch/encoding-failure cases, no-replay/conflict/input/auth rejection and byte-identical embedded assets pass. 92 Rust/integration plus 23 actual-source frontend cases pass; strict affected/default lint, formatting and whitespace pass. Failed R1 teardown compilation and R2 post-test capacity gate remain preserved; R3 completes missing checks without repeating native tests. | Interactive browser acceptance; model construction acceptance, registered machine-ready formatting, compiler/Kohai readers, final request guards, provider-wire/cache and quality gates. Comparing cases does not enable Generate/Redesign or activate a design. |
| Prefix-Maker / design workflows | Offline Prefix-Maker compiler/reference, registry metadata, immutable declarative proposals and Model Designs inspection/history exist. The [construction-teaching successor](../validation/2026-10-26-prefix-maker-construction-teaching.md) removes server-injection/custom-template assumptions and qualifies exact offline/native reference uptake. | Compatible runtime Prefix-Maker, accepted per-model designs and provider-assisted Redesign/model-design/Create Recipes, confirmed UI actions and exact candidate acceptance. Generate model design is currently disabled. |
| Answer/cache validation | Standalone source/citation/configuration/teaching helpers and historical Ornith/LMCache experiments exist; retained worker/native-C memory containment and runtime-discovery evidence are documented below. | Production citation/config checks, actual injection/resolver, current native/external restore qualification and held-out answer quality. Historical backport experiments do not certify the current server. |
| Deployment | Latest recorded remote discovery is read-only: app has no revisioned model-limit map and reports `active: null`; model endpoint advertises 131,072 context. | Upgrade the supported application path and verify policy readback before claiming limits are live. Requested Ornith policy remains 100,000 prefix / 14,688 other prompt tokens; `requested_change_applied` is false. No vLLM/Compose/venv/systemd change is authorized by this plan update. |

**Evidence anchors and applicability.** The dated journal carries earlier receipts;
these anchors establish the latest bounded successors used in this reconciliation:

| Receipt / source | What it establishes / limit |
| --- | --- |
| `target/validation/prefix-registry-artifacts-q9l8y7jj/accepted.json` | Installed Sempai v5 source-only/native/API/config-inspector slice; no accepted target design or serving. |
| `target/validation/prefix-registry-artifacts-m5jf5tis/accepted.json` | Real base/Sempai source jobs, independent HF/durable token measurements and ordinary single-root regression. Verified binary reuse is recorded, not a claimed rebuild. Historical snapshot, not certification of the newer merged tree. |
| `target/validation/prefix-registry-artifacts-u4w40vr0/accepted.json` | Strict affected-library lint/builds and seven focused native/Monty/PG cases for HTTP/classification/storage/settings/planning; no source job, deployed limits, activation or final request guard claim. |
| `target/validation/prefix-model-token-limits-ui-ld6sv9d2/accepted.json` | 37 actual-source frontend helper/state tests plus syntax, including selection across revision changes; no browser/external-CLI push acceptance. |
| `target/validation/prefix-model-config-ui-kk0ebg8y/accepted.json` | Six config-inspector source-state tests and syntax; no model/cache qualification. |
| `target/validation/prefix-token-settings-remote-d89w4l55/{observation,scope}.json` | Read-only old-application/model observation; explicitly records the requested change was not applied. |

Source comparison on 2026-10-22 found 67 selected prefix/model-limit paths in the
u4w40vr0 source inventory; only the two Model Designs frontend paths differed, and
their successor bytes match ld6sv9d2. This is a selected-file applicability check,
not a full dependency/toolchain or merged-production recertification. m5jf5tis is
older than the token-policy capture, planner and source-check additions; retain its
measurement facts without pretending it tests those successors. Preserve failed
snapshots/receipts, including gfygqf9w and txxdqobl, and all original packages.

### Remaining implementation sequence — full completion

The steps below are the current execution order for **remaining** prefix work.
They refine sections 4–12 rather than replacing their contracts or the shared v3.5
owners. Each step requires its stated production-path acceptance; code presence or
a new button cannot close it. Independent work may proceed when its actual inputs
are ready. Do not block model-free source builds on optional provider-assisted work.

1. **Reconcile applicable runtime gates (A/A1; shared owners).** Start from
   `InstalledMontyCatalogue`, retained IBS/preload, global admission, compiler
   bootstrap and native custody already implemented. Record exact applicable
   receipts for the intended successor graph. Obtain missing general pure-component
   approval from validator, qualified flow/occurrences from their v3.5 owners,
   and runtime/resource support from simplified only when the new graph needs it. Shared runtime/resource/recovery and provider-lifecycle work
   is implemented by [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben) and [simplified §10.1](../../simplified_v3.md#101-umsetzung-und-entfernung);
   consume those results rather than rebuilding the VM, provider registry or journal.
   Qualify each new exact dependency/native closure through the installation path or authored
   Q1/behavior/human Q2 as applicable. Exit: supported graph fails before effects
   on missing approval/artifacts/policy, retains selections and never falls to Tier 2.
2. **Finish authoritative source coverage (A/B/C).** Extend
   `prefix_source_capture.rs`, `catalogue_source_export.rs`,
   `prefix_reference_seeds.rs` and the release inventory through their existing
   owners. Inventory every required component/dependency/private graph/document,
   applicability, release, provenance and license; export coherent complete data
   without legacy row/heading cuts. Include approved dynamic catalogue changes and
   registered-source eligibility. Exit: >1,000-record and concurrent-activation
   cases, exact manifests, missing-required failures and override-preserving reseed.
   The [catalogue-export capacity consumer](../validation/2026-10-25-prefix-export-capacity.md)
   now captures acknowledged `execution_limits.max_catalogue_bytes` once through
   the existing preparation/settings owner after generation syntax validation and
   before activation/SQL locks. The old fixed 64-MiB duplicate is removed; no new
   configuration field/store/endpoint is added. Count the complete encoded JSON,
   including escaping, without another complete serialization buffer, and fail
   the whole projection on overflow. Native ordinary-runtime/PG/live-CAS checks
   pass at the exact byte limit and reject one byte less with token budgets both
   off and on. Raising the canonical limit restores identical complete source
   bytes; a rejected settings commit preserves the acknowledged limit and output.
   This closes that consumer/constant gap, not complete-library coverage, native
   artifact/IPC limits, model token-size guards or target-aware generation.
3. **Complete Sources execution (C/D; §8.1).** Reuse the existing HTTP/JSON and
   `builtin.prefix_source` usages. Compose registration read -> bounded fetch/read
   -> full identity/hash -> classify -> append diagnostic -> checked receipt, with
   separate actual Tool steps. Reuse the extended generation-read/source variant
   for originals; do not misuse registration foreign keys. Qualify/seed the graph,
   admit through ordinary jobs, project only trusted results and check on panel
   open/recheck. Add collection/archive/review and next-build inclusion without
   autoapproval. Exit: real HTTP/DB/artifact, policy/redirect/empty/change/failure,
   cancellation/dedup/reconnect/restart and pending-versus-used cases. No hidden LLM.
   The existing generation-read usage now has a candidate `source` variant via
   `generation_original_drafts`: one actual immutable-generation read followed
   by pure `project_prefix_generation_source`. Its input contract derives from
   the real reader result; `detail` binds to that result, never a task-supplied
   assertion. It returns the whole indexed UTF-8 original, original hash,
   disposition/reason and generation qualification. Legacy/no-index, different
   profile/generation, missing/duplicate references and malformed selected
   artifacts fail without current-file fallback. Python checks cover BOM/CRLF,
   literal code and a >1,000-entry index. [Scoped native acceptance](../validation/2026-10-25-prefix-archived-original.md)
   passes four integration cases in the joint Wallet run-5 receipt, including
   actual Monty projection and both retained read variants' assembly/preload;
   five Python checks are exact-source reused. This adds no store, registration
   FK, availability publication, public Sources job or approval shortcut.
   The [historical-index successor](../validation/2026-10-25-prefix-archived-original.md#historical-index-compatibility-successor)
   now accepts the native reader's `brassclaw-prefix-source-package/1` layout,
   including baseline rows without `artifact`/`kind`. It resolves originals by
   `raw/<sha256>` without following old input-package artifact names as
   generation/filesystem paths. Modern raw-pointer checks remain strict; missing
   required disposition/reason is an explicit error, never invented review data.
   Seven Python checks and all four native integration cases freshly pass with
   strict affected/default lints and source integrity. This differs from
   `legacy-bundle`, which lacks recoverable indexed originals. Qualify the exact
   component combination and execute the complete admitted Source workflow
   separately; these projection/assembly checks do not execute that workflow or
   a new PG read.
   A [registered-source candidate](../validation/2026-10-25-prefix-registered-source-check.md)
   now extends the existing HTTP diagnostic Recipe with `registered`: parse exact
   source pin/technical bounds -> native registration read -> resolve stored web URL
   -> reuse the complete eight-step fetch/digest/classifier -> pack metadata ->
   native unqualified append -> original receipt. The original direct `selected`
   variant stays intact; Read/Retain use one source Tool identity and their actual
   canonical binding definitions. URL/hash overrides are rejected. A registration
   checksum is never treated as an expected content hash; unknown baseline stays
   neutral, never green. Seven fresh Rust tests cover actual retained Monty/kernel
   HTTP/PG behavior, full large-response hashing, 404, wrong pin, overflow/live block,
   and immutable diagnostics without enabling availability. Remaining work is exact
   qualification, durable admission/settlement, used-source adapter integration,
   coalescing/cancel/reconnect/restart and trusted UI projection; candidate execution
   does not close those gates or collect reviewed knowledge for generation.
4. **Finish profile/shared-engine parity (B/C).** Port and qualify Defensive, HA
   and tomedo source/config/coverage/citation adapters against the shared worker;
   preserve their raw caches and domain semantics, no clinical tomedo content or
   HA leakage. Preserve the separate standalone adapters until replacements pass.
   Exit: deterministic host/path-independent packages, all six profile contracts,
   malformed/incomplete-source failures and domain-specific configuration checks.
5. **Close model-policy/capability preparation (C/C2; §5.3).** Use only the existing
   provider definition map/API/revision owner. [simplified §10.1](../../simplified_v3.md#101-umsetzung-und-entfernung)
   implements DB-only catalogue lifecycle, shared notifications/readback and CLI
   migration. This plan adds §5.3's prefix/model-specific fields and consumes that
   existing API/revision stream; it does not create another registry, reload owner
   or settings store. Preserve hard limits when discretionary budgeting is off.
   Capture exact answer/Sempai/design target identities, policy references,
   tokenizer/template/injection/output/deployed-context facts, with Unknown explicit.
   Exit: different providers/models, stale edits, toggle/restart persistence and
   no second mutable setting. Missing required target facts are Setup required.
   Include the K3 UI handoff in that exit: an original mutation receipt proves
   persistence, not that its returned catalogue is still current or that runtime
   uptake succeeded. Inference/Prefix editors must refresh the existing canonical
   provider query after confirmed persistence, including known-commit/reload-error
   paths; preserve the original receipt and visible read/reload errors separately.
   Do not publish an older mutation snapshot into that shared query. One provider's
   revision cannot order unrelated providers or role selections. The readonly
   Prefix Model Designs consumer must also fence superseded catalogue responses
   and errors so a late read cannot restore an older capture target. Signal known
   persistence through the existing notification owner even if runtime reconciliation
   remains pending; Unknown is not confirmed persistence. These are consumers of
   K3, not a second revision, reload or settings owner.
   That original-publication/UI handoff now has scoped shared K3 acceptance:
   [38 fresh/19 qualified reused Rust results and 26 current UI cases](../validation/2026-10-24-prefix-budget-selection.md#k3-prefix-ui-handoff--scoped-package-passed),
   including the real HTTP/PostgreSQL Prefix caller and strict affected/default
   lints. The subsequent [shared consumer package](../validation/2026-10-24-k3-provider-consumers.md)
   qualifies atomic backend/publication reads, async preparation, compatibility
   checks before cache use and raw retry attempts, and live cache/fill fencing.
   It has 123 actual passing Rust cases (120 main plus three explicitly executed
   live-model cases), 43 UI cases and strict affected/default lints. Reuse these
   existing owners; do not build another publication, provider or retry guard.
   Subsequent shared packages qualify role CAS, desired/effective publication,
   failed-target state, task-wide model selection and optional-role startup in
   their recorded scopes. Consult the current
   [implementation/ownership index](../implementation-status.md) for remaining
   K3 session/login/callback/refresh and other protocol custody; do not recreate
   already qualified provider or embedding owners. Prefix still owns the single final
   semantic envelope/token-size guard in step 6. The qualified compatibility gate
   is not that guard and does not complete model-policy preparation or serving.
   The earlier ordinary **MATCH** native-model prerequisite now has scoped
   [task-selection acceptance](../validation/2026-10-24-k3-task-model-selection.md):
   the existing owner captures DB-backed Kohai/Sempai selections before effects,
   retains them through admission and verified children, and the actual native
   model capability consumes that binding without a fabricated legacy route.
   Reuse it rather than adding a Prefix resolver or legacy `approved_routes`
   grants. Optional-role startup/recovery subsequently passes in the shared
   [Prefix successor acceptance](../validation/2026-10-24-prefix-budget-selection.md#profile-derived-design-inputs--shared-successor-acceptance),
   including provider-free ordinary Match after restart with both roles unavailable.
   Historical fixtures with manually recorded routes and global No-Match/client
   success alone were insufficient; those are not the accepted replacement.
   Task-selected model identity is not accepted named/composite Prefix selection
   or a final-envelope token-size guard. The shared
   [task-family embedding consumer](../validation/2026-10-25-k3-task-embedding.md)
   is now scoped accepted for both retained and generic Memory dispatch. Remaining
   K3 lifecycle/protocol boundaries stay with their existing owners.
6. **Implement unconditional technical request guards (C1/C2).** Kohai receives
   both active-target and Sempai-target bundles. Add the qualified semantic
   prefix/dynamic partition measurement and one shared last pre-dispatch guard over
   the actual transformed envelope. Check the complete Sempai request before review,
   after Kohai has formatted only the Orchestrator's dynamic prompt with the
   final target's pinned design. If connected/enabled, Sempai optimizes/corrects
   that already formatted prompt and returns the final dynamic prompt. Kohai
   joins its intended finished prefix unchanged exactly once, without formatting
   the return again, and rechecks the combined request before final dispatch.
   Use Sempai's returned final dynamic value for this join, not the original or
   pre-correction prompt. Generator and Kohai consume the same
   accepted design revision for that provider/model pair; the Sempai call uses
   its selected pair's pinned design and separately generated reviewer prefix;
   a shared pair uses the same design revision. Verify the
   [explicit handoff acceptance](../reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests).
   Include unchanged/disabled review, retry/tool continuations and
   tightened live ceilings; no token-budget bypass, estimated fit or silent cuts.
   Exit: exact/one-token-over tests for both caps/context/output with budgeting on
   and off, zero provider calls on overflow and explicit stale-policy handling.
   The [physical-span encoding prerequisite](../validation/2026-10-26-prefix-token-partition.md)
   now passes: the existing offline Tool and both canonical encoding Recipes
   support version-2 `partitioned_text` probes. One complete literal rendered
   text is tokenized once; actual UTF-8 byte offsets attribute token IDs to an
   explicitly supplied span or the remaining prompt. Boundary merges and
   unmappable nonempty-span offsets reject the measurement. Version-1 text/chat
   reports remain unchanged. Actual retained Ornith/Hugging Face, native
   PostgreSQL/bootstrap/journal and global-runtime checks pass with strict
   affected/default validation. This observation is explicitly
   `supplied-byte-span-only`: it does **not** establish that the span is the
   authenticated semantic prefix. Still implement the qualified adapter's
   exact transformed-envelope/range provenance, accepted prefix/composite and
   canonical policy binding, both ceilings and output/context arithmetic in
   the last pre-dispatch path. Do not treat the observation or the older
   complete-probe `fits` result as that guard. No new limit setting/store exists.
   The [leading-chat render successor](../validation/2026-10-26-prefix-leading-chat.md)
   now connects that measurement to exact chat rendering: `leading_prefix_chat`
   verifies complete supplied prefix bytes at offset zero in the full rendered
   chat, then partitions one actual encoding and binds the original ordered
   context digest. The real shared injection adapter has an independent six-context
   Ornith/Hugging Face comparison with stable prefix IDs across dynamic
   tools/roles/thinking/history; actual packaged native/bootstrap/journal and
   ordinary global checks pass. Its `verified-leading-render-bytes-only` label
   proves no accepted serving artifact, provider-wire mapping or policy compliance.
   Still bind this verified expected prefix/context to the actual host-owned
   transformed request and accepted artifact, then enforce current canonical
   prefix/prompt/context/output ceilings immediately before dispatch. Do not
   activate it as a guard or route version-2 reports through the old version-1
   capacity calculator. The observation is usable by those later Recipe steps;
   it neither adds a policy setting nor broadens legacy measurement admission.
   The [canonical dispatch-policy/count prerequisite](../validation/2026-10-26-prefix-dispatch-policy.md)
   now exposes exact provider/model input caps, applicable configured context and
   original requested output in the existing live guard's revision-bound result,
   from the same canonical read as cache policy. It adds no mutable settings or
   database owner. A reusable technical comparison checks whole-input count
   consistency, both caps and actual input plus output with checked integers;
   another model cannot borrow default-model context, and unknown limits/output
   remain errors. Actual PostgreSQL Kohai/Sempai/current-revision/original-target
   and live cache/request/global consumers pass. This is **configured-policy
   arithmetic**, not authenticated counts, deployed geometry or automatic final
   enforcement. The existing tightening failure remains until qualified fresh
   evaluation is actually integrated. Bind the actual effective output after SDK
   parameter transformation too; requested `max_tokens` may be stripped and must
   not become fictitious server headroom. Keep both this comparison and the old
   diagnostic fit receipt distinct from the final accepted guard.
   **Historical dispatch observation (26 October; superseded in part below):**
   `brassclaw_llm::ProviderRequestFacts` currently contains only `model` and
   `max_output_tokens`; the existing `GuardedProvider`/cache checks cannot derive
   actual input size from it. `brassclaw_reborn::model_gateway` coalesces system
   messages and sorts Tool definitions before the provider adapter translates
   them. The OpenAI `OptionalAuthHttp` path prepares the final request before
   calling `ProviderWireObserver.before_submission`; that response-custody callback
   receives no body. The 28 October successor below now adds a separate native
   body check at that same transport boundary. Complete/reuse these preparation,
   submission and custody
   boundaries with their shared owner; do not add another transport, observer
   registry, input journal, provider client or Prefix workflow loop.
   Preserve a single immutable prepared-request identity through exact mapping,
   measurement, cache handling and actual submission. Bind its actual transformed
   messages/tools/flags to the retained tokenizer/template and host-selected
   prefix artifact, rather than trusting a separately supplied diagnostic context
   or serializing an allegedly equivalent request a second time. Injection is
   request-owned only: use the target's default provider/model formatting and add
   the role/composite content exactly once through a qualified request slot.
   Prefix compilers already produce the finished provider/model-specific text.
   Kohai formats only the Orchestrator prompt part, then joins the pinned prefix
   unchanged. Previews reuse that same join contract and never reformat the
   prefix; final checks inspect the composed actual request. Provider file
   access, installation, service changes and a global role override are forbidden.
   Verify the required prefix-first order and actual content spans; a default
   mapping that moves tools/history ahead of the fixed prefix blocks this path
   rather than qualifying it through span-only accounting.
   Cover actual supported transports and multimodal expansion; text placeholder
   tokens cannot certify image/audio input size. Unsupported/unverified mappings
   remain explicit blocked states for the guarded target path.
   Apply technical checks before cache return as well as before a real send.
   At the last pre-submission fence, recheck that the selected prepared bytes,
   target and live policy revision still match the measurement; a tightened cap
   needs fresh qualified evaluation or an explicit configuration-changed failure,
   never an old fit receipt. Keep `PgProviderDispatchGuard`'s current tightening
   rejection until that replacement is actually connected and tested. No role
   change may silently reselect an existing task's model. Include zero-submission
   tests for late transformations, stale context/artifact/policy, tool/retry
   continuations, cached responses and both Kohai/Sempai targets. No optional
   discretionary-budget flag may bypass these checks.
   **Scoped SDK/transport successor — 28 October 2026:** the
   [final-request transport acceptance](../validation/2026-10-28-prefix-final-request-guard.md)
   now covers one immutable actual HTTP request, full-body/target identity,
   mandatory canonical checks after measurement and immediately before send,
   call-local guarding after SDK message/schema rewrites, shared registry-client
   reuse, cache bypass, unsupported-transport rejection and native no-send
   evidence. Eight final-guard/SDK cases plus the existing SDK/policy, native
   gateway/PostgreSQL and global Monty regressions pass: sixteen actual Rust
   tests, none ignored, with connected/default strict checks. Measurement
   fixtures supply declared counts; these checks do **not** qualify a real model
   tokenizer, semantic prefix partition, accepted design or ordinary attachment.
   `submission_rejected` records a native barrier; an error category alone
   cannot release unknown effects, and a previous submission-start marker wins.
   No additional provider pool, limit store or selectable hard-limit bypass exists.
   The initial supported hook covers immutable OpenAI Chat/ChatMaxTokens SDK
   calls; unknown/routed, guarded streaming/multipart and unqualified multimodal
   mappings remain blocked in this scoped path. Model KV caching is unchanged;
   the cache bypass concerns BrassClaw's complete-response DTO cache only.
   **Concrete contained counter successor — 29 October 2026:** the
   [native encoding prerequisite](../validation/2026-10-29-prefix-final-model-encoding.md)
   now supplies `ContainedFinalRequestMeasurer` over the existing bounded worker,
   explicit strict SDK-body mapping, original default-template/tokenizer assets,
   one complete encoding and exact leading-byte/token partition. Three independent
   offline Ornith/HF comparisons and eight native integration/regression cases
   pass freshly; 27 unchanged encoder/SDK results retain explicit applicability.
   Connected/default strict checks pass. The SDK producer now avoids redundant
   equal-model overrides that emitted duplicate JSON keys. The native acceptance
   verifier used in the integration case remains an explicit fixture, not trusted
   pair/profile/generation acceptance. No ordinary role attachment or provider
   rendering/cache qualification is claimed.
   **Native acceptance and caller successor — 29 October 2026:**
   [the connected acceptance record](../validation/2026-10-29-prefix-native-acceptance-and-callers.md)
   supersedes the missing native-binding/ordinary-attachment observations above
   for the scoped OpenAI text path. V182–V185 extend the existing proposal,
   acceptance, generation and admission owners. Pair/Profile/Generation approvals
   retain exact original sources, mapping, tokenizer/default-template assets,
   native worker/client contract and measured evidence. Approval and CAS target
   activation are separate; old tasks retain their selected generation. Ordinary
   Kohai/Sempai calls and the global post-turn reviewer now use that native binding
   and the actual final SDK body. Canonical live limits apply with discretionary
   budgets disabled. A selected final measurer replaces the older provisional
   tightening rejection with real current-cap evaluation; unmeasured paths retain
   the original fence. Twenty-three distinct relevant Rust cases and connected,
   default and Root-caller strict checks pass; this is local caller acceptance,
   not deployment or physical model-server parity.

   Apply these design/generation instructions through the existing producers:
   (a) retain source-only compiler packages unchanged and create a separate
   `brassclaw-prefix-target-generation/1` envelope with the exact original source
   package, finished `prefix.utf8` and `model_input_binding.json`; source-only
   imports cannot serve. Bind its Pair/Profile proposals and mapping without
   self-referential generation/approval hashes. (b) Use the actual default
   template **and actual SDK serialization** to choose the stable prefix boundary.
   Multiple system messages may be joined into one preamble. A closing system
   delimiter can therefore belong to the dynamic suffix: do not assume it is a
   stable prefix merely because a hand-authored example renders that way. Preserve
   the complete reference and fixed model-facing bytes without double framing,
   truncation or provider-server configuration changes. (c) Pair construction
   `/2` declares explicit wire mapping, supported plain/Tools modes, reference
   normalization and a positive request-output default. That default is not a
   duplicate hard cap. Qualify authored cases **and** the native no-submit SDK
   corpus (system joining, history, and declared Tool continuations) using the
   real bounded tokenizer. The corpus deliberately stops before HTTP submission;
   no fabricated success or model-provided counts qualify it. (d) Pin the measured
   leading token-ID fingerprint on each generation, alongside exact prefix bytes;
   check both in one complete encoding for every actual request. Shared Pair
   approval must not contain a generation-specific prefix fingerprint. Unknown
   context, mode, mapping, assets or binding blocks the guarded request.

   Remaining gates for this step: qualify the physical provider's default
   rendering/prompt-affecting defaults and applicable cache behavior against the
   retained assets; operator default-contract attestation and local SDK/tokenizer
   agreement do not prove remote-server parity. Connect ordinary Generate/
   Regenerate's source-only output to the checked target-envelope producer and
   existing acceptance/activation APIs, retaining the exact reviewed candidate.
   Complete explicit Recipe-selected and qualified PrefixFree workflow selection
   through the existing retained-policy owner, without bypassing the final guard
   or inventing another renderer/settings store. Qualify any additional transports
   or modalities before enabling them. Keep the full step open until those gates
   and its applicable product acceptance pass; do not repeat the now-passing
   native-binding/caller work merely because the broader step remains open.
7. **Implement accepted model/design selection (C/C3; §5.3).** Extend the existing
   model specification/proposal owners with validated runtime evidence, acceptance
   and profile/consumer bindings. Complete the concrete caller migration recorded
   in §5.3 and consume the same pinned accepted design in compiler and Kohai;
   metadata/proposal storage alone is not runtime design selection.
   Specify supported budget ranges, mandatory whole
   units/dependency groups, optional priorities/ties, rendering/teaching/citations
   and adapter fingerprints. Bootstrap reviewed supported designs without a model
   call. Exit: two different model designs, same-name/different-target isolation,
   missing/incompatible/unsupported-budget states and immutable accepted revisions.
   Implement the private draft-evaluation path **before writing acceptance**:
   V145 already requires an actual retained candidate generation for each design
   acceptance. Reuse the stage graph from step 8 with pinned unaccepted proposal,
   canonical target policy, current sources and exact artifacts; retain its
   quarantined candidate and validation evidence through the existing generation
   and admission owners. Qualify this draft operation explicitly; do not require
   its proposed design to have been accepted already, invent a placeholder
   generation, or create a second runner/store. Ordinary Generate must still
   require accepted design bindings. Acceptance references the exact reviewed
   draft candidate and publishes that artifact without a second unpinned compile,
   as the Redesign contract below already requires.
   The reusable `select-prefix-design-by-measurement` candidate Recipe now supplies
   deterministic whole-unit preparation and measured reference selection. Its
   `brassclaw-prefix-unit-design/1` contract validates all declared size bands,
   dependencies and stable priorities; exact full-candidate tokenization selects
   the fullest fitting candidate without assuming monotonic token counts.
   Connect this existing stage to accepted model/profile bindings and trusted
   canonical policy/source/artifact resolution; do not create another selector
   or treat its candidate definitions as approved/activated components. New source
   units and changed unit identities still require authoritative coverage checks.
   Treat its unit design as a resolved per-build plan. Reuse the implemented
   `brassclaw-prefix-generation-rules/1` resolver: reconcile current source/component
   coverage through exact/namespace/complete-unit selectors and required coverage,
   derive group membership and pin accepted-rule and resolved-plan hashes
   separately. Normal target Generate must bind these reusable rules, not mistake
   a fixed unit-index snapshot for a reusable template. A new component can change the latter without
   changing the accepted generation design. The existing proposal owner now
   parses historical v1 and the versioned v2 `whole-unit-reference/1` declaration,
   retaining its nested generation rules as an **unaccepted** proposal under the
   original model specification. Native source-independent shape/graph checks
   cover every band; actual source matching and coverage remain Recipe work.
   The legacy complete-reference measurement explicitly rejects v2 before
   admission, and its WebUI explains the exclusion. Four declaration/proposal
   tests, two actual Monty/tokenizer integrations, the real HTTP/PostgreSQL caller
   and ten affected UI tests pass, together with strict affected lints. Exact
   shared run-6 evidence and its raw-log limitation are recorded in the
   [successor validation](../validation/2026-10-24-prefix-budget-selection.md#versioned-proposal-successor--24-october-2026).
   Complete
   accepted model/profile/consumer and qualification bindings through this same
   owner; declaration retention is neither design acceptance nor ordinary Generate.
   V2 carries no copied hard limits or legacy reserve setting. Do not introduce
   another design registry or duplicate model-limit fields.
   The [original-policy successor](../validation/2026-10-27-prefix-model-policy.md)
   now verifies the model specification's recorded configuration facts against
   its exact retained provider revision through the existing owner. A matching
   snapshot hash alone cannot substitute another namespace, protocol, context or
   token-limit policy. Capture/read/catalogue and lost-response retry share this
   check; historical snapshots stay byte-identical after later provider edits.
   Earlier records with neither limit field remain readable and Unknown, never
   backfilled from the current definition; partial owner/value pairs fail.
   This closes recorded-configuration integrity, not canonical build admission.
   The [budgeted captured-selection successor](../validation/2026-10-28-prefix-budget-generation.md)
   now resolves verified immutable model-policy inputs with explicit build
   reservations through that same specification owner and reuses the pure budget
   planner as the first step of a third variant of the existing captured Recipe.
   Its complete captured receipt retains the planning inputs/result and selected
   or blocked source evidence. The native singleton runs all ten scenarios with
   discretionary token budgets disabled; unknown limits/context reject planning,
   and requested size cannot increase canonical caps. The existing dispatch
   compatibility fence also rejects removal of previously captured limits.
   This is private candidate execution, not ordinary Generate admission or a
   complete final-request guard; no extra limit store, endpoint or role setting
   is introduced. Old variants and their retained component bodies remain.
   Next, the draft/Generate owner must resolve and pin the applicable canonical
   target policy and pass its explicit limits/reservations to the existing pure
   budget planner, retaining the original revision and planning result. Unknown
   mandatory limits must block that operation; they are not zero or model-position
   defaults. Final dispatch still checks the applicable live technical limits.
   The Python dependency/closure implementation now scales through adjacency and
   sorted ready waves, preserving the previous rendering order. Its
   [scoped scaling acceptance](../validation/2026-10-25-prefix-graph-scaling.md)
   covers 15 Python regressions, all 4,096 four-group edge subsets, the actual
   Monty 1,001-group chain, both retained variants and strict affected/default
   checks. Complete candidate banks/texts/probes match the archived implementation
   for three differential fixtures. This is graph scalability, not completion of
   step 2's authoritative >1,000-record capture or a whole-model/cache benchmark.
   Compare proposed and resolved coverage with the independent authoritative
   profile/source floor; a provider's own `required_coverage` declaration is not
   evidence that its design preserves all mandatory knowledge. Source capture
   completeness and current-source eligibility remain prerequisite checks.
   The shared pure library now supplies a `coverage` phase comparing canonical
   `brassclaw-prefix-coverage-floor/1` input with every generation-rule size band's
   mandatory dependency closure. It retains exact comparison inputs and resolves
   namespace selectors against current whole units. This is supplied-data
   comparison, not authentication of the floor or source hashes. Wire the floor
   from the independent installation/profile owner into the private evaluation
   and accepted Generate graphs; do not let the design provider supply or weaken
   it. The existing six-step candidate Recipe still lacks that trusted resolver
   and is not eligible for ordinary activation. See the
   [comparison contract](../reborn/prefix-budget-design.md#execution-and-output).
   Its scoped successor passes 13 standalone cases, three actual utility-worker
   operations, both actual Monty/tokenizer/retained-assembly integration cases and
   strict affected Composition/Skills lints. The
   [raw acceptance and cleanup/reconciliation limits](../validation/2026-10-24-prefix-budget-selection.md#independent-coverage-floor-comparison-successor)
   do not close the trusted-floor resolver or accepted Generate/activation work.
   A follow-on pure `design-inputs` stage in the existing compiler library now
   derives that floor from `prefix_prepare`'s existing per-profile requirements
   and produces the included design-source inventory from its exact native
   digest batch. It retains the original complete snapshot, including excluded
   records, and accepts no model-provided mandatory list or edited compiler
   state. The typed constructor and actual native projection now have shared
   successor acceptance: all three Prefix integration cases, four compiler and
   13 selection Python cases, strict Root/Composition/Skills/default lints and
   the full HTTP/PostgreSQL Prefix caller pass. Its 319-source, 8,970,577-byte
   capture retains originals; the oversized 372,043-token Sempai diagnostic stays
   `fit=false`, without replay or activation. See the
   [exact successor scope and receipts](../validation/2026-10-24-prefix-budget-selection.md#profile-derived-design-inputs--shared-successor-acceptance).
   The [installed captured-source successor](../validation/2026-10-27-prefix-captured-design.md)
   now retains/inspects this exact graph and executes its complete captured variant
   with owner-captured core sources, native digest/encoding journals and the
   independent every-band floor on the already-running instance service. It
   preserves the original snapshot/comparison for selected and capacity-blocked
   outcomes and rejects weakened current or unused bands before candidate encoding.
   This closes the scoped private execution prerequisite, not ordinary admission.
   Installation-owned workflow qualification and target-aware draft/Generate
   admission remain pending: authenticate and pin canonical target policy,
   proposal/artifact/source eligibility and the complete native observations;
   retain the actual eligible candidate through the existing generation owner.
   The independent floor comes from the existing profile owner, never the model.
   Ordinary Generate must still consume accepted design/profile bindings.
8. **Build the ordinary target-aware Generate Recipe (C).** Reuse source-only
   admission/compiler/Retain, rather than replacing them. Pin trusted current
   sources, canonical policy and accepted design; use pure budget planning, accepted
   selection/rendering, exact full-envelope/partition encoding and required
   artifact/coverage/teaching/config checks. Retain originals and disposition maps.
   Publish the exact validated quarantined target artifact with a correlated durable
   receipt. Exit: zero model calls, deterministic repeats, capacity/coverage blocked
   failures with preserved evidence, cancellation, reconnect and no effect replay.
   Reuse the reference-selection stage from step 7. Its `reference-selected`
   result is not a final target artifact: perform real composite/chat partition
   checks before retention as an eligible target generation. Retain its original
   source inventory, full measurements and per-unit omission dispositions.
   Connect captured-source evaluation by extending the existing candidate Recipe,
   not by cloning its measurement/selection workflow: source preparation ->
   explicit original UTF-8 digest usage -> profile-derived floor/inventory ->
   every-band independent comparison -> reused candidate preparation/digest/
   encoding/selection. Bind candidate inputs to the successful comparison's
   exact output. Return the comparison and complete original snapshot alongside
   either `reference-selected` or `capacity-blocked`; failure to fit must not
   discard excluded originals or the coverage evidence. The original diagnostic
   variant is not a substitute for this captured-source gate. The existing
   `captured_design_selection_drafts` constructor preserves that diagnostic
   variant alongside three explicit native usages in the captured variant.
   The [earlier assembly acceptance](../validation/2026-10-25-prefix-captured-selection-and-sources.md)
   establishes both retained preloads and their truthful nonempty producer contracts.
   The [installed execution successor](../validation/2026-10-27-prefix-captured-design.md)
   now reuses exact source/compiler/native revisions, retaining only the candidate's
   complete dependency closure without advertising it in the active catalogue.
   Four private candidate scenarios run through the existing singleton and its
   acknowledged live resource policy; no second worker or fixed small-fixture
   limit is used for the full corpus. Original snapshot, coverage comparison,
   selection/dispositions and actual native journal handoffs are checked.
   Three actual Rust tests pass (one four-scenario candidate test and two existing
   Pair regressions), zero ignored, with affected-feature strict lint evidence.
   This is not an installation-qualified or activated target-aware Generate
   workflow. Next, qualify the exact graph and wire trusted canonical policy,
   accepted pair/profile design, eligible sources/artifacts, durable draft/Generate
   admission, candidate retention and final target-specific formatting/geometry.
   Reuse existing owners and the same Recipe; do not add a competing selector,
   queue, model-limit setting or per-task VM.
9. **Complete composite generation under that operation (C; §9).** Build/reuse
   eligible immutable parents and composites explicitly required by the selected
   consumer content contract, without compulsory base inheritance for all roles,
   preserving evidence IDs/origins and recomputing final geometry. Retain child
   run references and reconcile completed children. Use qualified iteration only
   if the graph actually requires it. Exit: one-click missing/stale/reused-parent,
   conflict/duplicate-source, partial failure, cancel/restart and aggregate-overflow
   cases; a parent-only success cannot claim target-composite completion.
10. **Finish dependency invalidation and publication freshness (C).** Track exact
    catalogue/source/design/policy/template/parent lineage; use existing coherent
    activation notifications and revision fencing. Publish desired/stale/blocked
    status and preserve last-good/history/retained tasks. Exit: source and provider
    races, output arriving after desired inputs change, upgrade/reseed and no
    active replacement from an outdated successful build.
11. **Implement named/composite consumer resolution and activation (C2).** Extend
    the existing facade/store/serving seams for independent Kohai/Sempai selections,
    revision-checked Activate/Rollback and retained continuations. Enforce the exact
    endpoint/template/injection contract and prefix once; unknown incompatibility
    fails closed. Implement target-scoped Warm/reuse through a retained, correlated
    provider operation with normal accounting/cancellation; it never activates or
    changes the inference deployment. Exit: ordinary provider calls, restart/activation races,
    deactivation/last-provider/reactivation and old-task/artifact retention, with no
    fallback to a minimal/stale bundle. Do not enable activation before step 12's
    target-specific minimum checks and step 6's hard guards pass.
12. **Qualify each real injection/cache path (C2/E; §9.1).** Verify actual deployed
    identities, effective tokenizer/template and plain/constrained requests. For
    observable native/external caching, test cold/native/external/restored-native
    expected content, hybrid attention/recurrent groups/checkpoints and stable
    namespace/salt behavior. An enabled known-corrupt external path blocks that
    target; missing optional telemetry stays Unverified. Exit: exact actual
    injection/semantic restore receipts for the configured path, without service,
    package, Compose or venv mutation from Generate/Warm.
13. **Finish the Sempai production contract (C1; shared v3.5 ownership).** Reuse
    the shared evidence/model/proposal/deployment/accounting components rather than
    specialized prefix Rust workflows. Version strict review claim/request schemas
    to carry both targets' trusted limits/identity; update producer/parser/persona/
    transport together, retaining old readers. Preserve full roles, Tool/effect
    evidence and authorized edits. Qualify the fitted role-specific Sempai reference and any explicitly required composite,
    artifact-first decoder, lossless sink and current component authoring/validation
    rules. Exit: held-out prompt repair and actual validation-candidate Recipes,
    malformed/adversarial/summary disagreement cases, no selfapproval or Tier-0
    hidden model calls, no original-turn replay and correct accounting settlement.
14. **Bootstrap runtime Prefix-Maker (C3).** Register/qualify its existing compiler
    and approved knowledge/model-specific design through the same generation and
    selection path. It uses a generation fitting its actual design-provider target,
    separate from the model it designs for. Exit: first install with no prefix or
    provider can build source artifacts; supported target bootstrap has no circular
    self-reference or ungrounded fallback and preserves old generations.
15. **Implement provider-assisted design Recipes (C3).** Compose Redesign, Generate
    model design and Create Prefix from shared source/evidence/provider/validation/
    proposal primitives. Confirm the exact request, pin current approved content
    and architecture, preserve raw responses, validate supported declarative rules
    and compile complete trial candidates. Use qualified multi-stage inspection or
    block when evidence cannot fit. Accept redesign records its exact design plus
    validated updated generation without a second Generate; production activation
    stays separate. Exit: cancel makes zero calls, dedup/unknown effect/stale
    acceptance/rejection, custom identity conflicts/reseed, two targets and safe
    Prefix-Maker self-redesign using the predecessor. No arbitrary compiler code.
16. **Integrate answer and configuration validation (C1/E; §10).** Carry the exact
    selected generation's evidence map through real answer/reviewer paths; adapt
    quotation/citation resolution without replacing tool/reviewer response schemas.
    Port HA checks and qualify independent defensive/tomedo parsers/semantic checks.
    Exit: forged/wrong-generation/unrelated-but-valid citations, misleading exact
    quotes, invalid-but-parsing configurations and malformed proposals are detected;
    no unapproved execution or live device changes merely to validate output.
17. **Complete the existing WebUI/API vertical (D).** Wire normal Generate/jobs/
    details/download/cancel/progress and composite readiness to the accepted paths;
    add Activate/Rollback/Warm gates, Sources check-on-open, confirmed Redesign/diff/
    acceptance, Create Prefix and model-design generation/history/binding. Reuse
    the shared limits editor/query/endpoint with always-visible hard-limit semantics.
    Preserve auth/CSRF, translations, keyboard/focus and accessible neutral/error
    states. Exit: real facade/HTTP/browser matrix, navigate/reconnect without job
    loss, duplicate clicks, races/failures and no success badge from metadata alone.
    Sources state must also belong to its exact profile and selected generation:
    no foreign generation request, stale source list or read error during a profile
    switch. A delayed registration result may clear only its original unchanged
    draft; preserve edits made while saving, and keep registration errors separate
    from inventory errors. Three regressions reproduce these failures in the
    baseline component. The correction is now applied and passes the five existing/
    new component tests and corresponding isolated real-browser comparisons with
    shipped React; settings transport is controlled in that fixture. Strict
    Root/Composition/Skills/default source-package checks also pass in the
    [same successor receipt](../validation/2026-10-25-prefix-captured-selection-and-sources.md).
    Backend availability checks and production deployment are not supplied by
    this UI correction.
18. **Qualify knowledge at supported model sizes (C1/C3/E).** Evaluate quarantined
    immutable candidates with pinned suites/decoding/adapter/runtime and coverage
    contracts. Test supported budget bands and transfer to unseen tasks across
    component authoring, defensive maintenance, HA YAML/MQTT/Modbus and tomedo API/
    billing-version cases. Keep first-pass, repair, semantic/citation/config errors
    and cache failures separate. Exit: recorded quality thresholds and limitations;
    neither spare tokens, cache hits nor benchmark memorization establish expertise.
19. **Finish installation/upgrade acceptance (B/C/D; §6/§12).** Package the complete
    source/compiler/validator/dependency manifests and supported model defaults;
    use existing installation qualification and idempotent seed owners. Preserve
    operator edits, accepted custom designs, old task/native artifacts and verified
    imports. Register actual supported compiler/validator artifacts, not developer
    paths or filename-only metadata. Any initial background source build uses the
    qualified model-free Recipe after global readiness, with no self-prefix dependency.
    Populate canonical active-target hard policies through supported APIs before
    serving cutover. Exit: fresh/offline/no-provider setup, upgrade/reseed,
    missing-artifact/unknown-policy and restart readback without boot network/GPU
    dependency or incompatible raw DB writes.
20. **Join the shared product cutover (A1/C2/§12).** [simplified Phase 6](../../simplified_v3.md#phase-6--datenbank--dateisystem--und-secret-migration) and [simplified Phase 8](../../simplified_v3.md#phase-8--cutover-und-entfernung-des-legacy-modells)
    implement the one upgrade/backup/restore coordinator; [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben)
    owns runtime draining, recovery and Ready-before-producers. Supply the prefix
    inventory/migrations/artifact-retention hooks and consumer smoke checks from
    §12; do not implement another cutover command, recovery service or startup gate.
    Enable only accepted prefix actions and retire prefix-specific legacy sweep/
    serving fallbacks after migration and reference release. Exit: shared cutover
    receipts plus prefix-specific restart/restore/rollback acceptance. External
    inference infrastructure remains operator-owned and unchanged.
21. **Close the full plan through the final matrix (§12).** Run every row below
    through actual facade/ingress/global Recipe/browser/consumer paths, map exact
    receipts to current applicable sources/configuration and account for every
    remaining gate. Keep optional unavailable cache capabilities honest, with no
    waiver of mandatory safety/source/size/quality checks. Update the delivery table
    to Complete only after this evidence and the shared applicable v3 gates pass;
    otherwise retain the precise unresolved step and owner.

**Critical order:** steps 1–7 establish target-facing prerequisites; 8–10 complete
ordinary generation; 11–13 establish safe consumption; 14–15 add assisted design;
16–18 complete product/quality acceptance; 19–21 finish installation/cutover/full
acceptance. Source-only work and bounded UI improvements can progress independently.
Steps 11/12 and the relevant parts of 16/18 are a joined activation gate, not
permission to activate first and test the configured cache later. Model-free Generate
must not wait for optional Redesign, while an unsupported budget is never silently
redesigned by a model call. See §5.3 for always-on hard limits and both-target checks.

#### Open definition, teaching and consumer successors from K4

This open Remaining-sequence successor governs new A1/C1/C2/Serving/upgrade
consumers (Exact D01–D03/D05–D07/D09–D11); implemented Phase-B/C journals, earlier
Phase-A/source/§5 requirements and their receipts remain untouched. Reuse current
retained exports, source capture, jobs and settings rather than rebuilding them.

New teaching/compilation must distinguish preload definitions, pinned invocation
and standalone probe bodies. The [single definition/reference successor](monty.composition.implementation.md#open-definition-and-example-successors-from-k4)
provides actual parser factory/bindings/closure and a separately unapproved pure
preview design. Its source-inspection report is JSON text, not a Dict or verdict.
Numbered previews retain selection/coverage metadata and cannot replace complete
raw originals. Source text is legitimate declared data; command grammar and
raw-value conversion still reject unsupported representations before effects.

Publicly matched Recipes retain recipe.md's public intent/negative coverage;
private source/compiler/reviewer work qualifies real typed admission, selected
variant/inputs and result custody instead. MCP publication remains optional and
separate: advertisement pins a listed command, actual chat capture pins the task.
Stale incompatible publication rejects before capture; captured tasks keep their
exact selection. No direct MCP execution path or new exposure registry.

Keep pre-call correction of the already formatted dynamic Kohai prompt separate
from completed-turn analysis of eligible No-Match evidence. The former returns
only the final dynamic prompt; Kohai adds its intended unchanged prefix once and
checks the full send. The latter returns analysis/unreviewed proposals and cannot
alter or replay the completed answer. Reuse existing transport/guards without
merging these response schemas or introducing reviewer recursion/bypass flags.

Changed embedded guides/teaching are new exact package inputs, not updates to old
receipts. Integrate with the existing writer's bounded validation window; preserve
originals/overrides and use existing immutable seed/revision/upgrade owners. Check
actual consumer uptake under K5/K6 before claiming installed use. The teaching
manifest/verification must identify the same source bytes; keep old receipts as
historical evidence, never relabel them. Unchanged program probes may be reused
only for their unchanged semantic scope. No new provider call solely for prose.

These additions are source/design guidance. General pure approval and future
compiler/Serving activation keep their existing gates; actual current bugs get
concrete owner work. The document update adds no global freeze or K8.

### Shared infrastructure ownership — reconciled 2026-10-22

**Concurrent delivery contract:** implement this plan alongside `simplified_v3.md`.
The owner in the following table implements each shared mechanism exactly once;
this plan contributes prefix requirements and integrates that implementation.
This assignment overrides older imperative shared-infrastructure descriptions
and dated implementation notes below. Existing accepted owners are extended,
not replaced. A missing prerequisite remains an explicit dependency, never a
reason to create a prefix-local substitute.

| Shared mechanism | Implementation owner | Prefix contribution and integration boundary |
| --- | --- | --- |
| Operator identity, scope/role removal, protected management | [simplified Phase 1](../../simplified_v3.md#phase-1--einzige-instanz--und-betreiberidentität), [simplified Phase 2](../../simplified_v3.md#phase-2--autorisierungsscopes-entfernen-daten-ids-entkoppeln) and [simplified Phase 5](../../simplified_v3.md#phase-5--webui-auf-eine-betreiberoberfläche-reduzieren) | Use the shared caller/context, authenticated facade and CSRF/origin enforcement for Prefix routes; supply prefix data-key migration cases. No prefix identity, grants or management stack. |
| Current Tool policy, alias identity and technical enforcement | [simplified Phase 3](../../simplified_v3.md#phase-3--eine-feste-runtime--und-sicherheitsrichtlinie) | Register compiler/source/publication primitives through the same host boundary; exercise live blocking through prefix Recipes. No stored bind-time permission or prefix policy engine. |
| Global Monty boot, admission, task/child scheduling, cancellation and transport | [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben) | Supply generation/design/Sources Recipes and typed payloads to existing admission. No second VM, durable queue, worker-claim authority, Inbox or Recipe executor. |
| Task selection, typed bindings, retained native artifacts and review consumption | [simplified Phase 0a](../../simplified_v3.md#phase-0a--ground-truth-komponentenverträge-implementieren) and [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben) integrate the shared component contracts | Use exact selected workflow/native references. Validator owns review writers/general pure approval; composition owns canonical library migration. Prefix adds only its own usages/graphs, not another loader, manifest store or approval route. |
| Invocation intent/count, effect custody, waits and crash reconciliation | [simplified Phase 0a](../../simplified_v3.md#phase-0a--ground-truth-komponentenverträge-implementieren) items 9–10 and [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben) | Provide operation-specific compiler/publication artifact/status adapters; retain links to original run/invocation records. Prefix job stages are projections, not a second recovery ledger or scheduler. |
| Runtime resources, discretionary budget switch and live settings publication | [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben) with [simplified Phase 3](../../simplified_v3.md#phase-3--eine-feste-runtime--und-sicherheitsrichtlinie) | Consume effective revisions and bounds for compiler jobs. Prefix §5.3 owns target-specific size semantics; it extends existing model/provider fields rather than duplicating runtime resource/settings ownership. |
| Provider catalogue, DB-only readers, reload, deactivate/reactivate and legacy-file retirement | [simplified §10.1](../../simplified_v3.md#101-umsetzung-und-entfernung) | Consume the existing provider map/revision/API; add only prefix model-capacity/design/injection integration. No provider registry, reader fallback, parallel lifecycle coordinator or second mutable policy source. |
| Shared seed/upgrade boot sequence, data migration and secret preservation | [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben), [simplified Phase 6](../../simplified_v3.md#phase-6--datenbank--dateisystem--und-secret-migration) and [simplified Phase 8](../../simplified_v3.md#phase-8--cutover-und-entfernung-des-legacy-modells) | Supply exact prefix source/compiler/profile/design seed payloads and additive migrations, preserve overrides/IDs, and plug them into the one initializer. No prefix bootstrap/migration runner. |
| Backup/restore, compatible binary/schema transition and runtime rollback | [simplified Phase 6](../../simplified_v3.md#phase-6--datenbank--dateisystem--und-secret-migration) and [simplified Phase 8](../../simplified_v3.md#phase-8--cutover-und-entfernung-des-legacy-modells) | Supply prefix generation/design/source/native references and restoration assertions; do not run a second stop/backup/restart procedure. Prefix selection rollback remains its own product operation. |
| Combined product and cross-platform acceptance | [simplified Phase 7](../../simplified_v3.md#phase-7--tests-architekturverträge-und-dokumentation-bereinigen) and [simplified Phase 8](../../simplified_v3.md#phase-8--cutover-und-entfernung-des-legacy-modells) | Supply prefix facade/global Recipe/browser/consumer and restore cases; reuse applicable shared receipts once. Passing either plan's local slice is not combined cutover acceptance. |

Prefix retains ownership of source capture/coverage and diagnostics, compiler
profiles/artifacts, model specifications and accepted designs, immutable generations,
composites, desired/effective prefix selections, Sources/Generate/Redesign/Warm UX,
and the exact rendering/partition measurement and final model-request size guard
in §5.3. [simplified remaining-work ownership](../../simplified_v3.md#remaining-implementation--consolidated-2026-10-22) explicitly consumes those results;
do not delegate them back and form a circular prerequisite. Prefix snapshots of
provider facts are immutable evidence, never a second mutable provider catalogue.

Coordinate shared boundary changes before writing competing implementations:
record the existing owner and required contract/consumer in the current status or
validation record, then implement the mechanism in its owning plan. Prefix may
wire/test a supported contract concurrently. If it is not ready, keep only the
affected action blocked and continue independent source/compiler/schema/UI work.
The runtime must still boot/manage the instance without a generated prefix or
configured model; optional provider-assisted work must not block model-free work.
Qualification follows the graph's actual requirements, not completion of every
unrelated phase. Never widen an existing narrow bootstrap exception to bypass a gap.

Read this plan with [Sempai v3.5](sempai_v3.5.md) and
[Simplify connections v3.5](simplify_connections_v3.5.md). Their existing owners
retain Sempai business sequencing, shared model/evidence/accounting semantics and
connection/facility consolidation. Use the instance-injected PgPool, network/egress
services and Monty transport actor. No per-profile pools, reconnect/pipe-reader
loops, review consumer or universal recovery journal is introduced here.

Completed infrastructure is reusable evidence for its exact scope, not a blanket
Gate A1 certificate. Named registry, immutable generation storage, installed source
capture, qualified source-only compiler jobs and durable measurements already exist.
Continue from those owners and their receipts. Generation/design publication stays
distinct from executable-component activation; prefix jobs link to actual executions
without claiming metadata registration or source observations are approval.

### Current development builds and historical evidence

Follow [the development policy](../development-policy.md#shared-source-and-build-ownership-2026-10-22)
for all new work. On this host, use the common repository-root checkout and
configured `/Users/ollama/brassclaw-target`, serializing Cargo under
`/private/tmp/brassclaw-target-cargo.lock`. Invoke Cargo directly with its inherited
configuration; no inline target override, shell wrapper, target2 or mutable build
mirror. Keep the mandatory disk-space rules and preserve needed evidence before
cleanup. Coordinate relevant source edits during validation; a build lock alone
does not freeze source. Other hosts use their configured canonical target.

Dated source/binary/receipt archives below remain valid only for their recorded
claims and inputs. Do not rebuild from them, restore them over current source or
rewrite an old receipt to describe the new build environment. Reuse applicable
evidence, complete coherent changes, and validate only the remaining affected
requirements; final integration/cutover gates still apply.

### Historical boundary summary

The former 2026-10-21 summary is superseded by the current delivery table and
remaining-work sequence above. Retain the dated Phase B/C entries below for exact
implementation/evidence history. Their whole-plan completion limits still apply.
Historical builds used `/Users/ollama/brassclaw-target2` while concurrent work
used target1; that split is retired and must not be recreated. No target-capacity
result authorizes inference-service changes, hidden source removal or declaring the complete plan implemented.

## 1. Recipe-first workflow and intended result

Enhance the existing Prefix settings page to manage six seeded profiles and operator-created profiles:
`base-prompt`, `defensive`, `homeassistant`, `tomedo`, `sempai` and `prefix-maker`. Each has a Generate /
Regenerate action, independent build status, immutable generations, source quality
details and an explicit deployment/cache status. Replace the current base-prompt
row concatenation with the exact-source compiler approach already used by the
domain compilers. Preserve the large-prefix objective and retain the complete
source library even when one serving model cannot hold it all.

The operator workflow is a Recipe, not a new Rust workflow Tool:

```text
Generate / Regenerate
  -> admit durable generation job for a registered profile
  -> pin approved component selection and release-document source snapshot
  -> export immutable compiler input
  -> run the profile compiler in an isolated worker
  -> validate evidence, coverage, template and actual token geometry
  -> publish immutable generation; finish the build job

Activate generation (separate operation)
  -> validate provider/profile compatibility
  -> atomically select generation for new model requests
  -> optionally warm the selected provider
  -> verify exact injected tokens and measured cache reuse

Redesign / Create prefix (separate Tier-1 Recipes)
  -> obtain explicit confirmation of the provider-assisted design request
  -> refresh/review applicable content and architecture evidence; pin current snapshot
  -> pin Prefix-Maker, model-specific design/source evidence and provider/target metadata
  -> provider proposes a model-specific design or new profile draft
  -> retain proposal; validate contracts and compile a complete updated candidate generation
  -> show design/content diff and quality/capacity evidence
  -> operator accepts validated design and publishes its exact updated generation
  -> later Generate uses that model-specific revision; Activate remains separate
```

### 1.1 What one click must deliver

Generate is a durable, model-free build workflow, not direct invocation of a
developer script. For a selected registered target it produces a reproducible,
source-backed, capacity-checked artifact suitable for the declared consumer. For
Kohai/Sempai that means the complete selected domain/reviewer artifact, including
a composite only where its explicit coverage contract requires one, not a parent
reference requiring an undocumented second manual build. Default selection uses
base; specific role/domain selection does not automatically append base. Orchestrate
missing/outdated parents and composite compilation as separate reusable Recipe
tasks under one visible operation; reuse eligible immutable parents. Pin a coherent
source/parent selection and retain child job references. Never secretly activate,
change a provider template, or replace an inference deployment on Generate.

Without a configured provider, Generate can still build a source generation against
registered local tokenizer/model artifacts. Label it Generated / target not selected;
it cannot promise serving readiness. Missing worker/artifacts, unreviewed source
changes and insufficient context have actionable blocked results. The UI must not
say “successful” when a subprocess exited zero but no validated artifact exists.

| Completion state | Evidence required | Meaning in the UI |
| --- | --- | --- |
| Source generation validated | Immutable originals, coverage, compiler and artifact checks | Downloadable reference; may still need a composite/target. |
| Target artifact validated | Above plus required parents, exact rendering and context checks | Generated for this consumer/target; eligible for activation only if resolver/target gates pass. |
| Model quality measured | Versioned task-suite results and intact drafts/responses | Separate first-pass and repair results; not universal expertise. |
| Effective / cache verified | Actual injection contract and qualified cache path | Serving observation tied to the current runtime, never inferred from compilation. |

“Perfectly designed” means all declared build contracts pass with no hidden loss or
unsupported claims. It cannot guarantee a small model answers every unseen problem.
Unused tokens are available capacity, not a requirement to add filler. Large,
high-quality reference content remains the objective.

### 1.2 Recipe and primitive boundaries

Use separate reusable Tool usages and pure-logic PythonCode components for each
operation. Each Rust-channel ToolSkill binding is immediately followed by its
associated orchestrator-channel PythonCode usage. Each component step references
one UUID. Skills describe one Tool usage and explicitly associate their executable
PythonCode; a Skill does not hide the whole generation workflow.

Reuse the current `host-assemble-prefix-bundle` Recipe, sweep/store primitives,
component stores, runtime admission/cancellation and existing shell primitive where
their contracts actually fit. Extend or version them rather than creating a second
prefix orchestration subsystem. Add a primitive only for an identified missing
operation, such as exporting a coherent catalogue snapshot or executing a bounded
compiler worker with artifact handles. Keep sequencing in Recipes.

The Python compiler makes no synthesis model calls. This does **not** make a Recipe
using `builtin.shell` Tier 0: shell Recipes are Tier 1 under the binding rules.
The current trusted internal-turn route is described as Tier-0-only. Verify and,
where necessary, implement support for an explicitly selected Tier-1 compiler
Recipe without permitting Tier-2 fallback. However, the first-build/rebuild path
must not depend on an LLM or an existing fresh prefix. Reuse an accepted deterministic
build/compiler adapter if its contracts fit; otherwise implement the missing bounded,
artifact-addressed compiler-worker primitive and its components before production
generation. This primitive runs the registered compiler with typed inputs and
returns artifacts; it does not sequence snapshot/validation/publication itself.
Accept the model-free Recipe path as Tier 0 only through its actual non-shell
execution contract. Do not relabel shell execution or bypass the orchestrator.

Record every proposed generation Recipe's exact LLM calls, prerequisites and tier.
The mandatory offline snapshot -> compile -> validate -> publish Recipe makes zero
LLM calls and admits work even with no generated prefix or configured provider,
using pinned local model/tokenizer artifacts. A missing artifact reports a specific
build prerequisite, not a provider request. Shell/Tier-1 variants remain separate
and cannot be the sole bootstrap path; any LLM steps require an already fresh usable
prefix and configured provider. No minimal/stale-prefix exception is introduced.

## 2. Precedence and reconciliation of the two older plans

Read this plan with [recipe.md](../../recipe.md), [skills.md](../../skills.md),
[tools.md](../../tools.md), [toolskills.md](../../toolskills.md),
[AGENTS.md](../../AGENTS.md), [simplified_v3.md](../../simplified_v3.md) and the
[development policy](../development-policy.md). Those binding contracts override
obsolete examples in the plans below.

| Earlier source | Preserve | Correct or replace |
| --- | --- | --- |
| [Archived Prefix Cache V3](../archive/prefix_V3.md) | Persist assembled content, stable fingerprints, cheap serving reads, separate Kohai/Sempai consumers and an operator Prefix tab. | Its completion marks are historical claims. Named entries are not supported by the existing scope-only unique key. Generation must not depend on an optional Sempai connection. |
| [Prefix Plan 2](../../prefix_plan2.md) | Recipe-owned sweep/store flow, validation gating independent of consumer tags, real class codes, fail-closed internal jobs, request-specific completion checks and concurrency fencing. | Its multi-UUID steps, source interpolation and mandatory fresh-step state are superseded. Do not adopt its old `host.*` capability IDs, arbitrary row caps, or architecture-heading slices. |
| Current domain compiler work | Literal evidence cards, raw snapshots, checksums, mandatory coverage, immutable bundles, actual tokenizer/template fingerprints and hybrid-cache measurements. | Domain compiler assertions and adapters must be separated from the reusable engine. Citation/configuration helpers are client checks, not automatic enforcement in every BrassClaw model call. |

The Python namespace remains `host.sweep_validated_components` /
`host.store_prefix_bundle`; their current capability IDs use `builtin.*`.
V088 repaired that distinction. Scope tickets and generation leases currently
fence a specific internal operation. They must not become extra Tool permission
grants. The v3 operator token and live instance-wide Tool policy control authority;
legacy tenant/user/agent/project columns are compatibility data until their
coordinated migration, not justification for new feature-role checks.

## 3. Current code: legacy path and successor seams

The legacy-path rows describe code still reached by ordinary base-only regeneration
and compatibility serving. They are not claims that the successor stores/jobs are
missing. The current delivery table above owns overall status; section 11 retains
its dated acceptance history.

| Area | Current implementation | Upgrade consequence |
| --- | --- | --- |
| UI | `prefix-tab.js` and `usePrefixes.js` already render entries and distinguish Generate from Regenerate. | Extend this surface; do not add a parallel settings page. |
| Browser API | `settings-api.js` calls the v2 prefix routes. | Retain the same facade-backed API family. |
| HTTP | Existing facade-backed routes cover list/legacy regenerate, immutable generation import/history/details, source inventory/registration, model metadata/artifacts/proposals and durable measurement inspection/admission. | Extend these route families for normal target-aware generation, source jobs and activation/design acceptance; existing detail/job routes are not missing infrastructure. |
| Facade | `RebornServicesApi` delegates to `InterceptorConfigService` in `reborn_services/interceptor_config.rs`. | Extend the existing service seam and DTOs; no direct DB/compiler access from HTTP handlers. |
| Service | `list_prefix_entries` reads all registered rows, including six seeds. Ordinary `regenerate_prefix` still executes only the legacy base path; registered unsupported names return Unavailable and unknown names return NotFound. | Wire accepted named/composite jobs and custom-profile lifecycle through this existing service; do not reseed a parallel registry. |
| Generation | `regenerate_prefix` creates an internal conversation, scope ticket, lease and request UUID, submits `host-assemble-prefix-bundle`, checks terminal reply and exact stored `generation_id`. | Preserve durable operation fencing and failure semantics; replace the long synchronous HTTP wait with job admission/status. |
| Sweep | `PgSweepValidatedComponentsBackend` selects validated rows from fixed tables without the old consumer-tag exclusion; sorts by class/prompt UID. | Reuse the read boundary, add one consistent snapshot and complete semantic records. |
| Lost information | Sweep expressions prefer `prior_knowledge_content`; Recipes contribute only prior knowledge or an empty string. Tool schemas, Recipe variants/bindings and associations are absent from the rendered body. | Export complete original content and structured execution metadata; summaries cannot replace them. |
| Truncation | Sweep SQL has `LIMIT 1000` per table and runs separate queries without an enclosing consistent-snapshot transaction. | Export the complete eligible catalogue with stable pagination under one snapshot; do not silently mix generations or omit rows. |
| Docs | Sweep appends compiled-in CLAUDE/AGENTS slices starting at architecture headings. | Include complete current authoring guides and required precedence sections, including material before those headings. |
| Role contamination | Sweep appends the Sempai response schema to the common bundle. | Keep role-specific response instructions outside the shared factual corpus. |
| Store | Legacy `PgBasicPromptStore` still uses a scope key and request UUID. Successor `prefix_generations.rs` retains named immutable content-addressed packages and evidence; job definitions link existing turn runs. | Migrate consumers to those existing stores and add selections/lineage. The legacy request UUID is not the immutable compiler generation ID. |
| Serving | `PgKohaiPort::complete_with_stores` resolves a stored bundle; `run_sempai_review` has `SystemBundleSource`. | Trace and test both independently. The archived claim that Kohai runs on every turn must not cause model calls on deterministic Tier-0 tasks. |
| Staleness | Q2 approval calls `mark_stale`; current `get_system_bundle` uses a minimal fallback for stale rows or DB errors and has a default-project probe on a miss. | Add dependency-aware invalidation and a deliberate serving policy; do not silently discard a large usable generation during a rebuild. |
| Startup | `component_boot::initialize_runtime_components` seeds host and first-party components and checks integrity; runtime initialization calls it. | Seed reference sources through this shared boot path, not only on WebUI mount. Global Monty lifecycle acceptance remains the separate Phase-3a prerequisite. |
| Citation helpers | All three domain compiler `--mode run` entry points use `run_compiler_task`; the standalone shared CLI verifies the server token prefix. | Carry evidence IDs/manifests into production provider/answer paths. Existing direct vLLM calls remain unchecked. |
| Standalone compiler execution | Sempai has legacy triage/distill/run modes as well as offline context compilation; the offline switch applies only to context mode. Runtime inspection uses local process flags. | A production worker must expose a pinned model-free build contract, not execute `--mode all`, inherit arbitrary environment settings or mistake its own host for the inference host. |
| Hybrid cache evidence | LMCache 0.5.5 / vLLM 0.31.0 on the inspected Ornith server produced corruption on external restores; separation alone failed, then an upstream adapter backport passed sentinel restores. | Make runtime/cache-layout correctness an explicit target prerequisite; package versions and native hit counts alone are insufficient. |

Relevant inspected files:

- [`interceptor_config_service.rs`](../../crates/brassclaw_reborn_composition/src/interceptor_config_service.rs), [`pg_prefix_bundle_backends.rs`](../../crates/brassclaw_reborn_composition/src/pg_prefix_bundle_backends.rs), [`pg_basic_prompt_store.rs`](../../crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs), [`pg_prefix_scope_ticket.rs`](../../crates/brassclaw_reborn_composition/src/pg_prefix_scope_ticket.rs).
- [`builtin_bootstrap.rs`](../../crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs), [`component_boot.rs`](../../crates/brassclaw_reborn_composition/src/component_boot.rs), [`pg_docus_store.rs`](../../crates/brassclaw_reborn_composition/src/pg_docus_store.rs), [`factory.rs`](../../crates/brassclaw_reborn_composition/src/factory.rs), [`runtime.rs`](../../crates/brassclaw_reborn_composition/src/runtime.rs), [`webui.rs`](../../crates/brassclaw_reborn_composition/src/webui.rs).
- [`pg_kohai_port.rs`](../../crates/brassclaw_reborn_composition/src/pg_kohai_port.rs), [`loop_driver_host.rs`](../../crates/brassclaw_reborn/src/loop_driver_host.rs), [`serve.rs`](../../crates/brassclaw_reborn_cli/src/commands/serve.rs).
- [`interceptor_config.rs`](../../crates/brassclaw_product_workflow/src/reborn_services/interceptor_config.rs), [`prefix-tab.js`](../../crates/brassclaw_webui_v2_static/static/js/pages/settings/components/prefix-tab.js), [`usePrefixes.js`](../../crates/brassclaw_webui_v2_static/static/js/pages/settings/hooks/usePrefixes.js).

## 4. Six seeded profiles, custom profiles and one reusable compiler engine

| Profile ID | UI label | Compiler entry point | Source content |
| --- | --- | --- | --- |
| `base-prompt` | BrassClaw Base Prompt | `base-promt-compiler.py` (offline development adapter) | Current release docs and complete approved component snapshot. |
| `defensive` | Defensive / Servers and Networks | `Defensive-compiler.py` | Existing defensive source families and security/maintenance evidence. |
| `homeassistant` | Home Assistant / MQTT / Modbus / YAML | `Homeassistant-compiler.py` | Existing official domain sources and coverage gates. |
| `tomedo` | tomedo / API / macOS / PostgreSQL / Billing and Coding | `tomedo-compiler.py` | Existing official docs, qualified forum observations and curated local findings; no clinical knowledge. |
| `sempai` | Sempai / Prompt Review and Component Authoring | `sempai-compiler.py` | Complete BrassClaw authoring/validation references and explicit prompt-diagnosis, Recipe and component-proposal procedures. |
| `prefix-maker` | Prefix-Maker / Prefix Design | `prefix-maker-compiler.py` (offline adapter; runtime qualification pending) | Versioned prefix engineering, evidence/formatting, model/tokenizer/template and cache architecture references, measured design lessons and the supported profile-design contract. |

Keep `base-promt-compiler.py` as the requested filename; the canonical profile name
is correctly spelled `base-prompt`. Paths are registry-owned, not browser-supplied.

The offline base adapter now reuses extracted HA evidence/rendering functions in
`scripts/prefix/prefix_evidence.py`, rather than retaining another monolithic copy.
Any remaining profile migration must replace **all** HA-specific source lists,
domain/platform tags, coverage gates, routing patterns, headers, runtime guidance,
default cache paths and usage instructions. Add release-document and component-
snapshot input adapters. Do not leave a Home Assistant topic gate or system prompt
in the new base compiler. Do not rename or replace the existing domain compilers.

After obtaining behavior parity, extract the common evidence/assembly machinery
into a shared module. Keep small profile entry points and profile-specific source
adapters. Preserve independent caches, raw sources and historical generations.
Sharing a rendering engine must not mix domain source databases or approval rules.

Current parity is limited: the Sempai compiler has the v25 scoped artifact audit,
reviewed strict-JSON display projection and evidence-view manifest; the independent
Home Assistant, Defensive and tomedo compilers do not yet share those features.
The base and Prefix-Maker entry points exist as offline development adapters.
Installed source-only Recipe support is separate from qualification of every
standalone mode or target-facing design. Port and qualify
these evidence/rendering facilities in the shared engine before claiming parity.
Keep Sempai's reviewer/planner schemas in its consumer adapter, not in domain answer
contracts. Inventory each profile's collection modes, source configuration, coverage
and validation adapters; generic domain-source environment variables are not a
supported way to configure the current Sempai compiler.

Builds expose explicit input/output directories, pinned model/tokenizer artifacts
and a machine-readable result. An offline build from a fixed source snapshot is
deterministic and uses no model synthesis. Source refresh is a separate observable
stage, with its own network permissions, recorded revisions and failure outcomes.

### 4.1 Production compiler contract and artifact layout

**Required migration under the corrected prefix/prompt ownership:**

The compiler changes below must connect to the existing callers identified in
§5.3's source-only caller audit. An accepted generated artifact alone does not
replace the installed catalogue's current prefix source or qualify the existing
interceptor's review/recomposition order. Implement those consumer changes through
their owning runtime/transport/design stores, without a parallel reader or loop.

1. Inventory `prefix_template.py.inject_reference`,
   `prefix_reference_components.rs::template_adapter_draft`, measurement capture/
   Recipes, exporters/package validators and UI/CLI guidance that generate or
   require `vllm_prefix_template.jinja` or `--chat-template`. Preserve legacy
   originals/readers/receipts, but remove provider-file installation and global
   role-template selection from the production path. **Do not remove local
   provider/model-specific prefix generation.**
2. Extend the existing versioned package/manifest with the finished target-specific
   prefix payload, exact text/UTF-8 hash, role/task/composite, explicit selected
   provider/model (including inactive role targets), accepted prefix-format/design,
   tokenizer/default-format evidence, canonical policy and the joining contract.
   Migrate producer, validators, stores, seeded Recipe handoffs and consumers
   together through existing owners. Required role knowledge belongs in that
   payload, not an assumed provider-side Sempai template. No parallel registry.
3. Use shared reusable PythonCode/Recipe compilation stages with accepted
   provider/model-specific prefix rendering. Produce the fixed exact payload at
   generation time. Independently, Kohai's existing adapter formats only the
   Orchestrator's dynamic prompt at runtime before optional Sempai review. If
   connected/enabled, Sempai receives that formatted prompt, uses its own finished
   reviewer prefix/design for its call and returns the final dynamic prompt.
   Kohai's assembler then reads the intended pinned prefix and adds it unchanged
   once before that returned final dynamic prompt, without invoking the formatter
   again. Sempai's reviewer prefix stays in its own request and is not part of
   that returned dynamic prompt. No runtime prefix rendering,
   template reinterpretation, normalization, newline rewriting or source rebuild.
   Define framing/separators at the correct target layer before qualifying the
   join; neither side may silently add a second set of model control markers.
4. Use the explicit selected target for generation/redesign/budget planning even
   when another model is active. Qualification previews reuse the **join** and
   dynamic-prompt formatting contracts; they consume the already formatted
   prefix rather than treating it as semantic input to a whole-request formatter.
   Keep prefix payload hash, dynamic-input/format identity, assembled-request
   identity, current policy and real token-attribution receipts separate. Unknown
   capacity/format is blocked, never replaced with the active model's defaults.
   Mandatory overflow stays Capacity blocked with originals intact.
5. Expose no production provider-config/template-install/restart action. Keep
   historical exports readable as labelled evidence. Production cannot require
   access to provider files, Compose, systemd, host environments or SSH.
6. Test byte/character equality of the generated prefix before and after preview,
   dynamic prompt formatting, joining, lossless transport encoding, final mapping
   and resumption. Cover Unicode, CRLF/newlines, quotes/backslashes and model
   delimiters, separate fixed artifacts for two models, same-model role switching
   without leakage, inactive WebUI generation targets, no double injection, and
   real prefix/prompt/total/output boundaries with zero sends on violations.
   Qualify the exact prefix-first join before using `leading_prefix_chat` as
   evidence for that target. Generic physical-span measurements remain useful
   diagnostics, but they cannot authorize a reordered dynamic-tools-before-prefix
   serving path. Unsupported default/transport mapping blocks the target until
   the adapter preserves the required join. Cache reuse needs its own evidence;
   existing custom-template receipts cannot qualify this production path.

Extract a library/worker interface consuming immutable registered inputs:
profile/policy revision, coherent source export, exact component dependencies,
verified teaching package where applicable, selected provider/model specification,
accepted **prefix** format/design, canonical ceilings and pinned model/tokenizer/
default-format facts. Its result is the complete target-specific prefix text and
provenance, not a runtime-neutral document that Kohai subsequently reformats.
The dynamic-prompt formatter and prefix assembler have their own versioned
contracts; measurement connects their final combined output without merging the
ownership of the two formatting stages. Obtain model facts through supported
metadata or authorized versioned official/operator imports, never provider-host
configuration access. Paths are BrassClaw artifact handles. Build-time refresh,
inference calls and live host/process inspection are excluded. Synthesis/doctor/
run/reset modes are not production build entry points. Test fixed-snapshot builds
without provider/network access; imports must not start clients or fetch sources.

Specify supported Python/library versions, installed dependencies, CPU/RAM/disk/
time limits, bounded child-process/output handling, cancellation, stage checkpoints
and atomic staging/publication. Compilation is primarily parsing, evidence
selection and tokenization; an NVIDIA GPU is not a prerequisite. Worker placement
is an execution choice, not a browser host/path field or new provider catalogue.
Resume verified stages; never publish a killed compiler's incomplete export.

| Logical artifact | Required contents |
| --- | --- |
| Input manifest | Canonical release/catalogue/source/policy/compiler/tokenizer/template identities and all pinned dependencies. |
| Raw source package | Complete original bytes, source disposition, selected/omitted whole-unit map and licensing metadata. |
| Evidence index | Stable IDs, original ranges/fields/revisions, original, display and rendered hashes, the pinned transformation/mapping manifest, applicability and navigation. |
| Finished target-specific prefix | Immutable provider/model-specific formatted text for the selected role/task/composite, containing required knowledge/instructions/contracts and exact character/UTF-8 identity. No live history, job IDs or per-turn timestamps; no server configuration file. |
| Prompt/join/measurement handoff | Kohai first formats only the Orchestrator's dynamic prompt for its target pair. A connected/enabled Sempai corrects that already formatted prompt with its own reviewer prefix/target design and returns the final dynamic prompt. Kohai then inserts the intended pinned prefix unchanged exactly once, with no second formatting of the return or prefix. Retain separate prefix, initially formatted dynamic prompt, review request/result, target join, assembled request and whole-request token identities. |
| Token manifest | Actual reference/shared/full-envelope token counts and leading-token hashes across supported variants; runtime cache assumptions separate. |
| Build validation receipt | Coverage, artifact integrity, source/teacher checks, capacity, deterministic reproduction and specific blocked/error reasons. |
| Quality/cache observations | Append-only receipts bound to generation and actual validator/provider/runtime revisions; never substituted for originals. |

Use relative artifact names or content-addressed references. Host checkout/output
paths, download locations and timestamps belong to operational records, not the
content ID or stable model text. Avoid hash cycles: establish generation identity
from canonical immutable inputs/rendered payloads, then attach receipts to it.
Different output directories/worker hosts must not change evidence IDs, compiled
text, templates or leading token IDs.

### 4.2 Current Sempai build configuration and creation instructions

This is the audited standalone v25 procedure, not an implemented WebUI job or a
portable interface for every model. Production Generate must resolve equivalent
immutable typed inputs from the accepted design and source export; it must not
inherit shell environment or discover processes. Record the compiler file digest
and pipeline revision `2026-10-09-sempai-artifact-scopes-v25` with every build.

| Current setting | Actual behavior and production requirement |
| --- | --- |
| Context target/reserve | Defaults are 100,000 requested prefix tokens, 131,072 context tokens and 31,072 reserved tokens. Reserve must be at least 8,192. Effective capacity is the lesser of the request and model context minus reserve, checked against rendered envelope variants. The earlier 114,688-prefix / 16,384-reserve preview is superseded for the current Ornith target: maximum prefix 100,000; maximum non-prefix prompt 14,688; combined input 114,688. Resolve production ceilings from the canonical provider/model policy; 31,072 offline reserve includes that non-prefix prompt plus 16,384 output/technical reserve. No fit is implied. |
| Model/tokenizer | `ORNITH_MODEL_CONFIG` identifies local config; the tokenizer loads offline from that resolved file's parent directory. Supply a coherent pinned config/tokenizer directory. `TOKENIZER_MODEL` does not replace this selector. The current adapter requires the inspected 24-linear/8-full architecture and single-GPU FP8 attention geometry; a new model requires a supported adapter, not just a new DB design. |
| Source inputs | `SEMPAI_SOURCE_ROOT` supplies the declared complete source package and teaching dependencies. Optional `SEMPAI_SOURCE_MANIFEST` pins the captured manifest. `CODING_REPOS` and `CODING_WEB_SOURCES` are rejected. Collection retains all declared originals; selection changes only the model-facing view. |
| Reviewed selection | `SEMPAI_REFERENCE_POLICY` must match the current document/excerpt hashes and whole-unit coordinates, inventory every unit and satisfy mandatory/topic coverage. Without it, the compiler renders literal originals; strict-JSON display projection is enabled only with the reviewed policy. Stale v22/v24 policies cannot qualify edited v25 sources. |
| Offline mode | Collect, distill and reduce are deterministic source stages with no synthesis calls; context uses `--offline`. Offline geometry is an assumption recorded as unverified serving configuration, not a cache qualification receipt. |
| Online discovery | The current context preflight combines target `/models` with process inspection on the compiler host and requires explicit hybrid flags and exact chunk/block equality. This is stricter than some supported runtime configurations and unsuitable for proving a remote target. Replace it with a versioned target-evidence adapter; do not modify operator services to satisfy the heuristic. |
| Identity/publication | Current generation identity includes compiler bytes and profile fields containing local artifact paths; source records also carry checkout provenance. Cross-host/path independence in §4.1 is a migration requirement, not demonstrated current behavior. Generation publication and the local `current` pointer do not activate a provider prefix. |

The current reviewed-policy gate requires complete model-visible `recipe.md`,
`skills.md`, `tools.md`, `toolskills.md`, `sempai-authoring-reference.md`,
`sempai-monty-compatibility.md`, `sempai-worked-examples.md` and
`crates/brassclaw_interceptor/src/packet.rs`; source/topic coverage gates additionally
apply to the selected corpus. Other whole units may be omitted only under reviewed
selection while their full originals remain retained. The schema-1 policy status is
`reviewed-source-selection`; each unit records path, line bounds, document/excerpt
hashes and an exact boolean selection. Its status label alone is not review evidence.
The present renderer orders ordinary sources before verified worked examples and
the compatibility overlay, followed by dispatch navigation, the source-heading index,
factual checks, scoped artifact audit and request boundary. This ordering is currently
compiler code, not an arbitrary provider-generated layout option. Port supported
layout changes explicitly and qualify them before exposing them as accepted designs.

Before running the example, supply an existing dedicated compiler Python environment
with the declared dependencies, a coherent source package including official Monty
artifacts and verified teaching receipts, local model/tokenizer files and a newly
reviewed selection policy for that exact package. The placeholder paths must be
replaced; use a fresh build directory. Do not install into or change the operator's
vLLM environment. Reuse unchanged teaching evidence only when its complete pinned
checker/dependency combination still matches; revalidate affected examples after
source drift. Never repair a stale policy by automatically rewriting its hashes.

```sh
export PREFIX_CHECKOUT=/path/to/brassclaw
export PREFIX_PYTHON=/path/to/compiler-environment/bin/python
export SEMPAI_SOURCE_ROOT=/path/to/coherent-source-package
export ORNITH_MODEL_CONFIG=/path/to/pinned-model-and-tokenizer/config.json
export CODING_DISTILL_CACHE=/path/to/new-sempai-build-directory
export SEMPAI_REFERENCE_POLICY=/path/to/reviewed-current-source-policy.json
export MODEL_CONTEXT_LIMIT=131072
export PREFIX_WORKSPACE_RESERVE_TOKENS=31072
unset CODING_REPOS CODING_WEB_SOURCES
# Remove an inherited source-manifest setting unless it pins this exact package.
unset SEMPAI_SOURCE_MANIFEST

"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode plan
"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode collect
"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode distill
"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode reduce
"$PREFIX_PYTHON" "$PREFIX_CHECKOUT/scripts/prefix/sempai-compiler.py" --mode context --offline --context-tokens 100000
```

For a source-manifest-pinned build, explicitly set `SEMPAI_SOURCE_MANIFEST` to the
matching immutable manifest instead of unsetting it. Do not add `--offline` to the
other stages: that flag is accepted only by context. Avoid `--mode all` for this
procedure: its context call currently uses defaults rather than the CLI context
request. Legacy triage/run modes are not required to create this exact-source prefix.
If mandatory selected units exceed capacity, retain the failed build and resolve
coverage/model capacity; the example does not authorize automatic truncation.

Publish and inspect the immutable `generations/<generation-id>/` artifacts, including
`context_100k.md` (a legacy filename, not its measured size), template, source/evidence
manifests, policy, teaching receipts and token/build manifest. Pin those paths rather
than the mutable top-level aliases or `current` pointer. Complete original documents
remain under the build's `raw/documents/` and teaching material under `raw/teaching/`;
the generation directory alone is not the complete raw source package. Production
publication must retain that package separately with integrity and retention links.
A build's `external_cache_reuse_verified=false` cannot become a warmed/qualified badge.

Consumer schema/grammar generation is a separate protocol artifact step. Pin exact
ordered schemas, grammar compiler/runtime identity and decoder options; regenerate
incompatible planner/reviewer grammars. The retained v25 protocol checks do not
constitute a newly generated prefix or model inference result. Optional provider
planning and quality evaluations remain explicit Tier-1 workflows, never hidden
inside model-free Generate.

## 5. Base-prompt knowledge: complete originals and applicability

### Mandatory current documentation

Include complete `skills.md`, `recipe.md`, `tools.md`, `toolskills.md`, `CLAUDE.md`,
`AGENTS.md` and `simplified_v3.md`. The actual filenames are case-sensitive on Linux;
map user-facing lowercase names to their real paths. Do not rely on macOS case
folding or architecture-heading slicing.

Create a versioned inventory of all other documentation for the current BrassClaw
release: current `docs/agents-v3`, subsystem contracts and crate `AGENTS.md` /
`CLAUDE.md`, current supported installation/configuration/API/operations references,
and relevant runtime schema/capability definitions. Inventory every candidate and
record whether it is included, superseded, historical, development-only or private.
Curated inclusion rules must have reviewable reasons, not silent omissions.

Treat `docs/reborn/model-token-limits.md`'s shared selection/design/assembly and
generation-dialog contracts as current architecture reference candidates in that
inventory. Reconcile inclusion with the existing release-document/embed owner and
simplified K5 before claiming installed compiler uptake. Merely linking this guide
from an embedded document does not prove its content was seeded. These documentation
changes do not modify old generations or close source-package/runtime acceptance.

Archive material and implementation plans are not current runtime truth. Preserve
their provenance where useful, but label historical examples and unimplemented
targets. `prefix_plan2.md`, `docs/archive/prefix_V3.md` and this upgrade plan must not
become evidence that a target feature is already implemented. Keep development-only
rules separate from runtime operator guidance. Exclude `LOCAL_TEST_ENV.md`, secrets,
attachments, private deployment details, build products and patient data from the
shipped reference package.

### Skill knowledge and optional MCP publication

A complete Skill needs no dedicated Recipe or command. Distinguish its reusable
function contract, actual consuming/test workflows and optional MCP publication.
The current `catalogue_source_export.rs` projects selected workflow closures (plus
its explicit private-prefix source scope), not all independently approved unused
library rows. Include every selected Skill regardless of missing MCP declaration;
do not filter it as incomplete. A future broader library inventory needs explicit
owner-derived selection/approval semantics, not a raw-row scan or a second registry.

Extend source/render/knowledge acceptance with: a complete Skill without MCP,
the same usage in two consumers, a reference Recipe without public exposure and a
known Skill that is not an available model-call command. Optional `mcp_call` belongs
to the immutable Recipe revision; malformed declared contracts still fail closed.
Keep existing Retain/Read/Encoding/Digest/private compilation workflows and their
exact evidence. "Canonical" in their names identifies that concrete workflow,
not an existence requirement or ownership edge for every reused Skill.

Changed packaged guides or generated Skill prose change source/package checksums.
Keep historical receipts and captured tasks; qualify new source/seed revisions via
the existing integrity/bootstrap/upgrade path before claiming serving uptake. Do
not rewrite frozen teaching/probe archives. Regenerated prefixes and future quality
probes must teach the corrected distinction without implying new executable APIs.

### Components themselves

Export all eligible activated/approved components and the structured data needed
to understand them, not just `prior_knowledge_content`:

- Tools: stable UUID, real class, callable/capability identity, complete description,
  parameter/result schemas, effects, technical prerequisites and implementation
  identity; runtime availability is a separate fact.
- ToolSkills: complete binding descriptors and Tool references.
- Skills: full prose, explicit associated PythonCode and exact combination approval
  records where supported; no inferred association from names.
- PythonCode: exact source, declared inputs/results, internal includes and pinned
  dependency graph. Code excerpts remain reference data and are never executed by
  the compiler.
- Recipes: full `step_descriptions`, ordered one-UUID steps, variants, `step_link`,
  input layouts, dependencies, intent examples and failure/completion contracts.
- Extension/catalogue, Orchestrator/Scaffold and other supported component records:
  real classes and full relevant bodies/metadata. Do not invent separate tables for
  classes 10/50 or describe classes 1–3 as a leaf/domain hierarchy.

Target: include the complete eligible library once its broader owner-derived
inventory is supported; current selected-closure export is not that full inventory.
Retired/pending/rejected records are retained
in their proper source/history stores but not presented as current callable usages.
Consumer tags remain routing metadata, never validation evidence. Reject incomplete
required associations/references and surface unsupported schema/version features.
Do not claim today's `validation_status='validated'` proves the final immutable
version/association target is already enforced.

A reproducible build pins one catalogue snapshot plus one release-document manifest.
Capture the catalogue generation and immutable UUID/revision/checksum selection,
including transitive dependencies. If complete version storage is still missing,
an immutable export can preserve a build input, but cannot be advertised as final
IBS version enforcement. Implement and accept that prerequisite before making the
stronger claim. Resume/retry never reads a new “latest” selection midway.

### Model-facing representation

Keep source text, YAML spacing and Python source literal. Render structured records
in stable canonical order with explicit type, source UUID/revision, contract,
dependencies and navigation links. Keep authoritative source JSON bytes separately
when canonical rendering changes their presentation. Evidence cards bind rendered
sections back to original document ranges or component fields and source hashes.

Put binding definitions and precedence early, then a stable component index and
complete source units grouped by usage/dependencies. Preserve qualifications and
failure rules with their commands/examples. Do not replace executable code or
Recipe bindings with synthetic summaries. Architecture targets, current support
and observed test evidence are distinct fields/sections. Source documents do not
gain instruction authority, grant Tools or override current kernel policy.

### Dedicated Sempai reference and review inputs

Seed a fifth `sempai` knowledge profile and its compiler/source registration at
installation through the same shared boot, generation history and integrity path.
The requested copy to `scripts/prefix/sempai-compiler.py` already exists. Audit its
replacement sources, topic gates, cache paths and guidance; preserve its verified
tutorial/evidence rather than recopying the HA file over this work. The copy and
`sempai-authoring-reference.md` procedure are local compiler artifacts; DB seeding,
activation and production reviewer wiring remain implementation work.

Collect and preserve complete `skills.md`, `recipe.md` (the actual filename, not
`recipes.md`), `tools.md`, `toolskills.md`, `AGENTS.md`, `CLAUDE.md`,
`simplified_v3.md`, the validation-queue guide, observed Sempai proposal transport
and dedicated authoring procedure/persona. Model-visible coverage may use an
explicit reviewed source-selection policy: retain the complete four binding
authoring guides, procedure and proposal transport, and approved complete source
units from the other documents. Bind document/excerpt hashes, all selected/omitted
unit dispositions and policy identity into the immutable generation. Source drift
requires review; no greedy truncation or keyword-only coverage substitute. Record
source applicability and distinguish targets from observed runtime/older examples.
Preserve every mandatory contract and negative requirement. Do not include live conversations,
credentials or patient data in the seeded reusable reference. Add current installed
component catalogue context through Sempai's own declared content/coverage profile;
compose shared sources explicitly where needed and deduplicate
identical original evidence while retaining provenance and complete required content.

The procedure teaches two related tasks:

- Diagnose the Kohai prompt/current conversation for the actual target provider:
  intended outcome, instruction conflicts, irrelevant duplication, missing evidence,
  real context capacity, thinking/tool/structured-output capabilities, role ordering
  and exact tool-call/result relationships. Preserve user intent, observations,
  incomplete effects and uncertainty. Adjust only the permitted volatile prompt;
  never rewrite the task's pinned base or infer unknown provider capabilities.
- Propose reusable components, especially Recipes: discover existing usages before
  authoring, define variants and typed input/result flow, one UUID per component
  step, immediate ToolSkill/PythonCode pairing, internal dependencies and supported
  constructor fields, ten-plus intent examples, exact Skill associations/approval,
  failure/retry/effect contracts, semantic/behavioral review, Q1 and human Q2,
  coherent activation, immutable retention and dependency-aware prefix rebuilding.

Extend the trusted review packet through the existing interceptor/gateway seams to
carry the Kohai request, current conversation, packet/run/turn identity, pinned
workflow/prefix references and DB-resolved target provider/model revision and
capabilities. Distinguish this target from the provider serving Sempai itself.
Preserve structured roles, tool envelopes and call/result IDs instead of reducing
them to text pairs. Redact secrets through host handling. The current role/content
pair input does not supply all these facts; missing metadata is an explicit gap,
not permission for Sempai to invent it. Distinguish pre-call prompt review from
post-response learning: pre-call review cannot observe a response not yet produced.

Sempai produces drafts through the current proposal sink; it cannot approve itself,
mutate live component versions, grant Tools or activate settings. Use actual
class-specific proposal/store schemas and trusted draft identities; missing APIs
block that proposal. Preserve `SempaiReviewOutcome`/persona response contracts as
role-specific instructions outside the factual prefix. Update their versioned
schema/persona together where needed; the older persona example must not override
the current `proposed_components` transport or introduce unsupported insert fields.
The standalone compiler's citation `--mode run` is a factual benchmark only and
cannot replace the production reviewer/proposal response schema.

Bind the Sempai generation (or its explicitly required composite) independently of Kohai's
selected domain. Keep both provider selections DB-only and pin review generation
and target request metadata through continuations. Add no Sempai model invocation
solely because a deterministic Tier-0 task completed. Reference readiness, review
enablement, proposal validation and provider/cache observations are separate states.

Initial standalone evidence (2026-10-06): eleven full source documents, 247 validated
source units and 542602 source bytes. The pinned local Ornith tokenizer's sum of
mandatory rendered cards is approximately 184990 tokens before the final envelope,
already above its 131072-token context and the explicit 100000-token preview target.
The compiler correctly reports Capacity blocked; no generation was published or
deployed. See [the capacity receipt](../../scripts/prefix/sempai-capacity-verification.json).
This is not a shared-token/cache measurement or production acceptance. A larger
compatible target or an explicitly reviewed source/partition policy is required;
complete mandatory guides cannot be silently omitted to fit this deployment.

Reviewed standalone deployment (2026-10-06): the explicit selection policy now
compiles 119/247 preserved units into a 67432-token reference (67700 maximum
rendered probe tokens). It was deployed to the existing `brassclaw` vLLM template;
exact server token IDs and 66528 native cache-hit tokens were verified. Fourteen
direct-model reviewer probes from `brassclaw2` demonstrated a port-validator draft
passing 13/13 independent Linux CPython cases after feedback and an offline v3
Recipe design passing structure/intent checks. Initial JSON/proposal failures,
duplicate/incomplete drafts and a malformed blocked Recipe remained important
negative evidence. This is supervised draft generation, not production Q1/Q2,
Monty compatibility, DB seeding, automatic proposal insertion or consumer wiring.
The audited sink supported classes 21/22 but discarded v3 Recipe fields. The
subsequent [draft-persistence repair](../reborn/contracts/sempai-draft-persistence.md)
retains supported constructor fields, rejects unknown fields and commits the
legacy draft/queue together. This does not establish immutable review admission,
complete class-specific validation, association support or production approval;
those prerequisites remain required before activating v3 proposals. See
[the live test report](../../scripts/prefix/sempai-evaluation-20261006/README.md).

### Evidence learned since that initial deployment

The v14 standalone reference has 97,822 text tokens, 97,828 shared leading tokens,
92 complete 1,056-token blocks (97,152 cacheable tokens) and a 676-token shared
tail. Its 29 complete teaching examples pass 132 Linux behavioral probes with
deliberate mutations rejected. These are teaching artifacts, not activated
components or Monty/production Q1 proof. See the
[retained experiments](../../scripts/prefix/sempai-accuracy-20261007/README.md).

Unchanged-client v13 measured 54/60. V14's different two-call planning, strict-schema
and input-presentation protocol measured 55/60 first pass and 60/60 after up to two
feedback repairs. Additional development transfer cases measured 10/12 first pass
and 11/12 after two repairs. Neither protocol changes nor repair acceptance may be
advertised as prefix-only gains or flawless first-pass ability. Healthy native-only
semantic failures remain distinct from cache serving failures. V14 is standalone
Sempai: its capacity does not prove a full installed base-plus-Sempai composite fits.

### 2026-10-09 Monty/compiler evidence and promotion limits

The complete pinned official Monty source package and compatibility overlay now
compile with all four current authoring guides. After concurrent source changes,
the latest isolated v22 generation has 113,903 text tokens, 113,909 shared leading
tokens and 107 complete 1,056-token blocks (112,992 cacheable tokens); its maximum
build-time rendered probe is 114,171 tokens, with 16,384 tokens reserved. These
measurements belong to Ornith's inspected tokenizer/template and standalone Sempai,
not a full installed base-plus-Sempai composite or a universal model design.
The current v22 priming check verified exact automatic server injection and
112,992 native cache-hit tokens. Plain external restoration was exercised after
GPU displacement: zero native hits, 112,992 extra LMCache hits and the correct
sentinel answer. This evidence covers the inspected LMCache MP in-memory path,
not persistence across service restarts or unrelated cache layouts. The structured
external-restoration check also passed with the same cache counts and correct READY
JSON. Keep quality checks and production qualification separate.

The preceding fixed v19 design/content trial scored 58/60 first pass versus 55/60
with the prior two-call protocol, with five improvements and two Recipe regressions.
All 24 pure-logic candidates passed 318/318 probes in actual upstream Monty 1.0.0.
Transfer scored 10/12 first pass and 11/12 after two bounded repairs. Monty reference
answers had 22/22 correct booleans but only 17/22 source-topic-supported citations
after manual audit, including one corrected evaluator spelling false negative.
Retain the original receipts; valid citation IDs alone did not prevent selection
of an unrelated passage. See [the retained experiment report](../../scripts/prefix/sempai-accuracy-20261009/README.md).

These observations establish source/format and limited development behavior,
not flawless performance, a no-regression promotion or production Q1/Q2. Current
source refreshes need their own qualification; do not relabel v19 scores as v22
scores. Prefer complete general teaching and actual artifact-level checks over
adding benchmark-specific answers. Require helper returns, missing-only defaults,
and review/sink prerequisites in the emitted artifact itself. Repair evidence stays
separate and cannot conceal first-pass regressions. Actual custom Monty loader,
Tool binding, full approved component associations, proposal persistence and the
installed consumer path remain independent production acceptance gates.

The refreshed v22 knowledge trial completed 27/28 JSON answers, with all 27 boolean
answers correct; one response was truncated and stays failed. Its original citation
topic check passed 18/28. Manual review identified three narrow wording/newline
matcher false negatives, yielding 21/28 supported answers, with six unrelated
passage selections still rejected. Preserve both original and audited receipts.
Recompilation of the identical pinned v22 inputs reproduced its exact generation
and manifest. This does not convert the old 58/60 trial into a current-content
score or remove the no-regression/production prerequisites.

The same-source v23 display/navigation experiment preserves v22's exact original
evidence and selected 220 units. Lossless strict-JSON display compaction saves
5,633 tokens; a source-heading index and general artifact audit use that space.
All 28 reference responses are valid JSON, 27 booleans are correct and the
unchanged topic-support checker accepts 23 complete answers; correct explanations
with reversed booleans and unrelated citation IDs remain failures. Frozen first
pass is 57/60, transfer 11/12. Both earlier Recipe-prerequisite failures are fixed,
but two validators never enter a success state and one Recipe repeats an intent.
This is development evidence, not accepted no-regression promotion. Retain the
failed drafts and comparison instead of claiming a perfect design. The following
v24 candidate groups navigation by document and strengthens contract-versus-plan,
valid-path, cardinality and boolean-proposition audits. Its fixed first-pass suite
passes 59/60, transfer 10/12 and strict reference checks 25/28. The missing-only
default remains incorrect; one transfer request emits no requested Recipe, and
the MCP server-lifetime answer still contradicts its own explanation. Preserve
these failures. Actual upstream Monty checks pass 302/302 probes for the 23 frozen
programs admitted by the pure-Python gate, not all 24 requested programs; transfer
passes 33/34. This is not flawless or complete production qualification.

Qualification must also check decoder field order against the accepted design.
The historical v24 evaluation schema emitted `composition_summary` before
`proposed_components`, contrary to the prefix's instruction to summarize after
checking the final artifact. Reference output emits the boolean before its reason.
The v25 evaluator now emits proposed_components before composition_summary, with
the summary last; other field constraints remain unchanged. Grammar manifest v2
fingerprints the ordered schema and enforces fixed property order. The loader
rejects old reviewer grammars with stale ordering, while retaining compatible v1
artifacts only when their exact schema/layout still matches. New planning
schemas necessarily require regenerated grammars. Local tests and installed
XGrammar 0.2.7 matching verify accepted artifact-first and rejected summary-first
output. This fixes a protocol contradiction, not a demonstrated accuracy score.
Evidence/reason-before-verdict for reference questions remains a separate future
change. Preserve baseline artifacts and distinguish client-format changes from
prefix-only improvements.

The v25 authoring procedure and compiler classify host embedding examples,
preload definitions, invocation bodies, Recipe constructors and prompt review
separately. Module-level result/invocation requirements apply to invocation bodies;
preload definitions expose typed returns without running at load. Retain upstream
Monty originals and verified teaching programs. Optional planning now returns
exact current-contract quotes with aspect labels and supplied Recipe references,
not invented executable branches. The host verifies quote provenance and reference
identity; artifact classification, coverage and semantic agreement still require
review of the complete original contract. Neither valid quote selection nor JSON
shape proves code behavior or authorizes activation. Empty compatibility arrays
never cancel requested component drafts; offline constructor support is distinct
from live execution/sink limitations. Fresh source capture and normal policy/integrity
qualification precede compilation; these edits do not rewrite published generations
or change the deployed prefix automatically.

The v24 paired MCP-lifetime probe verifies a cold miss, 112,992 native reused
tokens and an external restore with zero native hits and 112,992 extra LMCache
tokens. The same incorrect boolean and correct explanation appear in all paths;
this particular polarity failure is reproducible without cached execution.
The retained `v24-lifetime-semantic-cache.json` and its separate audit establish
only this fixed question, not general cache correctness. Keep task/grammar/content
identical across cache paths and score semantics independently. A short READY
sentinel alone cannot qualify Sempai behavior.

The paired v24 missing-default writer probe also verifies cold/native/external
paths, with 114,048 tokens externally restored and zero native hits. All four
writer stages emit exactly the same code, bound by SHA-256 to the retained Linux
and upstream Monty checks (6/7 behavior probes pass; absence fails). Reuse that
exact-code evidence explicitly instead of implying new executions. This holds
the already-generated untrusted plan fixed; it does not test planner regeneration
or prove correctness of all restored tasks. Both paired cases reproduce their
failures cold, so these particular errors cannot be attributed solely to cache
restoration. The active startup log also confirms automatic `align` mode.

### 5.1 Knowledge quality and model-facing structure

Include a pinned official Monty documentation package and a separate BrassClaw
compatibility overlay. The current standalone package is upstream v1.0.0 at
`85c5d1f6bef038405cfc40a4eed94806e303567e`, with 61 original artifacts and retained
MIT licensing. Preserve all originals; rendered API-template directives alone do
not establish callable signatures. Bind upstream version, selected custom Monty
revision, host ABI and effective constraints independently. Cover language/module
limitations, eager iterable behavior, host-value conversions, private transport
limits, cumulative task accounting, trusted snapshots and durable effect recovery.
Upstream support for eval/exec or filesystem callbacks is not BrassClaw permission.

Distinguish effect-free Skill definition preloading from pinned export invocation
and standalone step-body artifacts. Validators must select the actual artifact
kind: preload definitions must not dispatch Tools, while an invocation captures
the function's complete returned result. Do not require module-level invocation
inside a preload-only artifact or accept an uncalled helper as a completed step.
Pin export names/signatures, dependencies and isolated mutable state through the
real loader/runner contract. MCP sends a completed listed command through an
ordinary chat; it is not a direct Monty/IBS/Tool execution endpoint. Historical
tutorials retain their dated applicability and do not override current guides.

Version each profile's source and teaching policy. Require a coverage matrix mapping
each promised capability to authoritative source units, applicability and checks;
heading/keyword matches alone are insufficient. Sources record release, product/
protocol version, licensing/distribution disposition, original hash and review
status. Official specifications and project contracts take precedence; forums,
cookbooks and local findings are qualified observations with prerequisites and
limitations. Resolve conflicts explicitly rather than giving every source equal
instruction authority. Refresh failures cannot silently substitute unreviewed
content or manufacture current-version claims.

For base/Sempai, include complete required authoring/schema/association/retry guides
and approved catalogue records. Defensive coverage includes evidence preservation,
incident response/backdoor/rootkit investigation and reversible maintenance with
actual platform/version context. HA covers YAML indentation/typing, automation/
script semantics, MQTT session/QoS/retention and Modbus addressing/data interpretation.
Tomedo preserves versioned API, macOS/Postgres and dated German billing/coding
sources, distinguishes local discoveries and excludes clinical knowledge. None of
these references grants operational authority.

Use one predictable envelope: trusted role/authority and precedence, navigation,
applicability/contracts, literal evidence units, verified general teaching units,
then one short task-independent dispatch/checklist boundary before volatile input.
Keep the shared base spine stable; consumer instructions belong to the registered
trusted envelope, not duplicated personas inside factual source units.
Test stability across knowledge updates as well as request variants. The current
renderer places the complete source inventory and version-dependent document-card
IDs before their evidence text; a new component or a late document edit can
therefore invalidate an otherwise unchanged leading core. A successor accepted
model design should place stable core material before changing catalogue/control
metadata and move versioned citation/provenance indexing after the relevant stable
text where the supported renderer permits it. Preserve version-bound citations
and exact origin reconstruction; do not substitute path-only identities to inflate
hit counts. Trial an appended component and a late-source edit with unchanged
mandatory knowledge, record full framed token-prefix changes, and qualify hybrid
reuse separately. Implement a new versioned renderer/design contract; do not alter
frozen literal/v1 or v2 semantics silently. Repeated transfer envelopes may use a
lossless reference index only when every original value/definition is retained,
references resolve exactly and the complete raw envelope remains archived. The explicit v3 trailing-inventory and v4 stable-sharing options below address
parts of this requirement; accepted per-model selection, stable placement of all
citation metadata and full serving/cache qualification remain separate work. Only the
trusted envelope establishes authority; quoted source instructions remain data.
Clearly delimit reference data. Preserve complete programs, YAML blocks,
qualifications and related prerequisites together; nested source examples must not
become chat roles or new instructions. Do not summarize away executable artifacts
or repeatedly duplicate instructions. Stable source/component identifiers support
navigation and citation checks.

Separate original evidence from its model-facing display. A reviewed model design
may compact standalone strict JSON examples only when decoded values and embedded
program strings remain identical. Keep Python, YAML and prose literal; preserve
ambiguous JSON (duplicate keys, nonfinite values and precision-sensitive numeric
lexemes) unchanged. Retain original excerpts, source locations and quote resolution
against original bytes. Pin a display manifest to the generation with original,
display and rendered-card hashes; reject changed mappings rather than rewriting
evidence fingerprints. Verify transformations with real teaching artifacts and
negative examples. Display hashes establish integrity, not semantic approval.

Generate a compact task-boundary source/section index from actual selected headings
and exact citation IDs. Navigation labels never replace evidence identities. Do not
embed evaluation questions, expected answers or candidate feedback in that index.
Include a short general artifact audit scoped to five distinct artifact kinds:
host embedding, preload definition, invocation body, Recipe constructor and prompt
review. Host setup examples are embedding references, not class-22 entry points.
Preload definitions export typed functions without running them; invocation bodies
call the approved entry point and capture its result. Historical standalone teaching
programs do not override preload authoring rules. Trace absent/null/type/boundary
branches and typed returns against the actual artifact contract; verify
Recipe prerequisites in the emitted artifact rather than only its summary. Keep
this model-specific layout within the rendered context plus workspace reserve;
reject overflow without dropping mandatory evidence. Measure citation support,
first-pass emitted artifacts and transfer behavior before accepting a redesign.

Audit optional model-derived plans as source-anchored navigation against the entire
original trusted contract, not as executable branch specifications. Exact selected
quotes prove provenance only; they may omit requirements or misclassify an artifact.
Derive and check every branch from the original contract, including missing-only
defaults; an intermediate plan cannot cancel or invent them. Explicitly trace a valid path
through reject-on-failure loops and verify their success-state assignment. Count
distinct final values for cardinality/uniqueness requirements. For binary source
questions, the boolean must answer the actual proposition, including negation,
and agree with the explanation. Locate support in the full original evidence
before copying an ID; an index heading is navigation, not supporting evidence.

Teaching must transfer beyond a fixture: presence before value access, missing
versus null, exact booleans versus integers, recursive container/item constraints,
numeric boundaries, defaults only on missing inputs, complete invalid results,
helper invocation and module-level result assignment for invocation bodies,
effect-free preload exports and typed returns, typed step handoff, effects
and supported proposal transport. Pair positive complete examples with scoped
negative contrasts. Validate whole artifacts and behavior against independently
specified contracts; AST/schema success alone does not establish meaning.

Pin teacher manifests, validator code and transitive checker/schema dependencies,
execution environments and verification receipts. Reject drift until affected
examples are revalidated. Developer-edited examples require review; compiler output
must not turn model-generated explanations into official sources. Frozen evaluation
fixtures/oracles and failed-draft feedback never enter the reusable reference.
Feedback belongs only to that candidate's volatile review task. Keep first-pass,
bounded-repair and fresh/held-out results separate.

Concurrent component or documentation edits never justify changing expected hashes
inside compiler code, weakening integrity checks, reverting another author's work
or rewriting a published bundle. Capture a new coherent source snapshot, review
changed whole-unit selections, rerun affected teaching checks and compile a new
generation. Keep the previous sources, receipts, failed attempts and experiments.
Content digests belong to versioned source/policy/evidence manifests; the compiler
checks those records. Compare the build's pinned catalogue/document generation
with current desired inputs before activation and show changed sources as stale.
An in-flight experiment remains tied to its original snapshot; do not redeploy a
different prefix midway and combine its results into the same accuracy score.

Select content by required coverage and measured transfer, not “fill remaining
tokens.” Preserve the large prefix and full source store; admit optional whole
units only under a reviewed deterministic policy after mandatory units fit. If
mandatory knowledge cannot fit, fail with exact coverage/capacity evidence rather
than choosing lossy slices. A fully seeded base library may require a larger model.

### 5.2 Prefix-Maker reference and versioned design workflow

Introduce `prefix-maker` as the sixth seeded knowledge profile, independently
generated through the same model-free compiler and evidence gates. Register its
source package, procedure and `prefix-maker-compiler.py` entry point;
reuse the shared engine rather than another rendering implementation. Bootstrap
uses a reviewed shipped design and pinned local artifacts. Generating Prefix-Maker
must not require Prefix-Maker, a provider call or a pre-existing fresh prefix.

Its knowledge package includes authoritative, versioned model/provider/tokenizer/
chat-template and cache specifications; the four BrassClaw component guides and
supported profile/compiler interfaces; evidence-preserving source selection,
canonical formatting, navigation, code/YAML/JSON integrity, authority boundaries,
context/output capacity and stable leading-token construction. Include measured
lessons from Ornith and section 9.1: INT4 weights versus attention/recurrent dtypes,
full/linear attention groups, physical/logical pages, recurrent checkpoints, native/
external cache restoration, compatible namespaces, injection ownership and cache
qualification. Distinguish measured configurations from general principles; do
not apply Ornith geometry to an unknown model or claim a cache trains the model.

Teach complete general design examples and their validation receipts, including
coverage/capacity failures, contradictory sources and source-versus-instruction
boundaries. Apply the same freshness, licensing, complete-unit and held-out quality
rules as other profiles. Benchmark answers, secrets and conversations are not
reusable reference sources. Public/provider documentation refresh is a separately
recorded source job, never uncontrolled browsing during compilation.

Define two Tier-1 Recipes using reusable discovery/export/provider/proposal-store/
validation usages: Redesign an existing profile and Create a new profile. Record
their real component UUIDs, typed contracts, ten-plus intent examples and positive/
negative routing cases during implementation; explanatory steps here are not
persisted Recipe JSON. Each independent Tool/provider call is a separate usage.
Use the accepted DB-only provider gateway and actual sink contracts; introduce no
special Rust workflow executor or guessed API. The design provider may differ from
the target model: pin both identities and capabilities explicitly.

A profile Redesign updates its content rules and knowledge under the selected pair
design. Changes to shared format/limits compatibility require an explicit proposed
pair-design successor and a visible impact review for all affected profiles; do not
create a private Sempai/domain format design or change manual limits through model
output. Collect the current eligible
approved component catalogue, release/architecture documents and applicable model/
cache evidence under the configured refresh/review policy. Preserve newly collected
unreviewed sources in quarantine and report the prerequisite rather than treating
them as current authoritative knowledge. Pin the coherent current snapshot after
this stage; do not merely reformat an old generation or wait for a later Generate
click to incorporate the new information. Preserve the old generation as the
comparison baseline and return a complete updated candidate as the task result.

A design task pins an approved fresh Prefix-Maker generation/composite, current
profile/design/generation, source/evidence manifests, requested goals, hard coverage
requirements and target metadata. For new profiles the existing-profile reference
is absent explicitly. Compose one trusted envelope with Prefix-Maker exactly once;
the prefix being reviewed is delimited untrusted input, not a second system persona.
Check the full request capacity first. If it cannot fit, use only an explicitly
reviewed multi-stage inspection contract retaining complete evidence and coverage,
or return Capacity blocked; never silently slice the reviewed prefix. Unknown
architecture or unsupported adapter behavior requires evidence/setup, not invention.
Use the existing host secret/data handling before export to the design provider;
credentials and unrelated conversations never enter the review packet. User goals
and imported content remain typed task data, not Python source or trusted authority.

Provider output distinguishes shared pair-format/design changes from profile/content
changes, with separate references/diffs. The profile draft contains purpose/consumer, proposed source
registrations, coverage requirements, whole-unit selection and canonical ordering,
section/rendering rules, teaching/validation requirements, target compatibility,
expected tradeoffs and supported shared-engine options. Preserve the raw output,
rationale, exact request/response identities and diff against the existing design.
Provider suggestions about cache/service flags are advisory; this workflow never
patches packages, changes services or deploys inference infrastructure.

The model cannot install its output as arbitrary compiler Python, browser-supplied
paths or shell commands. Validate a versioned declarative design schema against
registered adapters and supported options. Missing engine/schema support is explicit
implementation work, not executable text or a silently ignored setting. If a design
requires new executable components, route them through separate supported authoring,
Q1/behavioral review/human Q2 and registration; do not treat the popup as approval.

Compile a complete quarantined candidate generation with the current pinned sources
and proposed pair-format/profile-content revisions, including composites only where
the consumer explicitly requires them; require
evidence preservation, coverage, formatting, capacity and reproducibility checks.
Optional provider evaluations are explicit child jobs with first-pass/repair/transfer
results and cache correctness separated. Show Unsupported, Capacity blocked or
Validation failed rather than claiming an upgrade merely because a model approved it.
Present old/new design and content diffs, omitted-unit reasons and receipts for human
review. Accept redesign records the new immutable model-specific design and publishes
its exact validated updated generation through a fenced, reconciled operation. Reuse
the candidate's verified artifacts, not a second unpinned compile. If content/design/
target changed meanwhile, report stale and require explicit rebuild/review; do not
accept yesterday's snapshot as current. Authored component changes still require Q1
and human Q2. Select the accepted design for future builds independently of production
prefix activation; the new generation is Generated, not automatically Effective.

Generate snapshots the applicable approved knowledge using the selected immutable
design. Identical design/target/source inputs reproduce identical output; new approved
components change information, not design. Only explicit accepted Redesign or a
reviewed design upgrade changes formatting/selection rules. Concurrent edits make
a proposal stale for acceptance; retain it for rebase/review, never overwrite a newer
design. Existing jobs/tasks retain their pinned revisions.

Prefix-Maker may itself have a Redesign button: use its previously approved fresh
generation to propose its successor. It cannot approve itself or use its unvalidated
successor as the review authority. If no usable predecessor exists, restore/build the
reviewed bootstrap reference first; do not fall back to an ungrounded model call.

### 5.3 Database-owned model specifications and model-specific designs

#### Source-only caller audit — 26 October 2026; migration still required

- V145 defines model specifications, design proposals and acceptance storage.
  `prefix_model_designs.rs::PgPrefixModelDesigns::catalogue` still reports
  `design_generation_available = false`. These records do not establish accepted
  design generation, serving selection or Kohai's use of that design.
- Ordinary skills-db `runtime.rs` wires `InstalledMontyCatalogue` as
  `SystemBundleSource`; its `get_system_bundle_for_run` returns the retained
  `task.view.prefix`. That is a task-pinned catalogue source, not evidence of a
  stored provider/model design lookup or a finished target-specific artifact.
- The wired `InterceptingModelGateway` currently captures an already assembled
  request before optional review and then recomposes its messages. Preserving
  an existing system basis during review does not prove a separate Sempai target
  design/prefix or final-target prefix insertion only after correction.
  Migrate this existing boundary instead of wrapping it in another workflow loop.
- `PgKohaiPort` explicitly describes its routing path as having no Sempai wired;
  do not mistake a direct test of that path for ordinary-runtime review/design
  qualification or use it as a second implementation of the workflow.

Steps 6/7, §4.1 and serving must therefore wire and qualify the existing owners:
original Orchestrator prompt → Kohai formats only the dynamic prompt using the
**same saved, accepted, pinned target design as the generator** → optional
connected/enabled Sempai optimizes/corrects that formatted prompt and returns the
final dynamic prompt → Kohai joins the intended final-target prefix unchanged
exactly once, without formatting that return again → complete-request guard →
submission. The review request resolves the design for Sempai's selected
pair (the same design when the pair is shared) and its separately generated
finished reviewer prefix; never copy the final provider prefix into editable review
data. Reuse the existing design store, catalogue/task retention, Recipe orchestration
and provider preparation/custody boundaries. Do not add a second design registry,
prefix reader, provider client or loop. These observations are source-only and
do not certify the required migration or runtime behavior.

#### Budget-aware design and generation contract (2026-10-22)

**Always-on hard limits, including when budgeting is disabled (operator
clarification):** `max_prefix_tokens` and `max_prompt_tokens` are technical
per-target input ceilings, not optional `ProviderTokenBudget` allowances. Their
persistence, resolution, visibility and enforcement must never depend on
`token_budgets_enabled`. Turning budgeting off removes discretionary slice/task
quotas; it neither clears these fields nor bypasses a size guard. The same holds
for deployed context and output constraints. Unknown required limits/tokenization
block a model-facing dispatch with a setup/measurement reason; do not guess a
character-to-token ratio or depend on the provider rejecting the request.

The planning term "prompt reserve" means space reserved for the canonical
`max_prompt_tokens` ceiling; it is not a separately editable or optional setting.
Generate always consumes both canonical ceilings, even with discretionary token
budgets disabled. Removing an already captured model-limit entry must invalidate
old text-client work rather than turn that work into an unbounded request.

Kohai must receive two separately identified, host-resolved limit/geometry bundles:
the active answer provider/model and the configured Sempai provider/model. A bundle
pins canonical policy revision, prefix generation/design, injection adapter,
tokenizer/template, actual deployed context and output allowance. Expose both
numeric ceilings to the selected orchestration components as typed data and to
Sempai's trusted review instructions; model output cannot change these values.
Do not send Sempai only its own limit while withholding the target answer limit.

The pre-call review/prompt-optimization Recipe must implement this sequence:

1. Kohai formats only the dynamic Orchestrator prompt for the final answer
   provider/model using the same saved accepted pair-design revision as its prefix
   generator, with the actual retained history/tools. Preserve that formatted
   dynamic prompt separately from the finished prefix. If Sempai is disabled,
   this is already the final dynamic prompt; proceed directly to the prefix join.
2. If Sempai is connected/enabled, build its exact request around that already
   formatted dynamic prompt as review data, including its own target-specific
   Sempai prefix (or explicitly required composite), trusted instructions,
   original-target limits, reviewable prompt/evidence/history,
   tools and provider framing. Check Sempai's prefix and non-prefix prompt ceilings
   independently and complete sequence plus output against its deployed context
   **before** dispatch. Knowledge already present in a verified identical prefix
   need not be duplicated, but a hash/reference to absent evidence does not mean
   Sempai inspected it. Use only qualified evidence views or multi-stage inspection
   with explicit coverage; otherwise report Capacity blocked and preserve originals.
3. Retain the exact Sempai request/result and coverage. Sempai returns the final
   dynamic prompt to Kohai. Treat that return as untrusted candidate data while
   keeping the immutable prefix, target format/design, Tool protocol and user
   constraints intact. Validate the selected result schema, permitted edits and
   compatibility with the retained final-target format; validation does not
   reformat or normalize the returned dynamic prompt.
4. Kohai joins the intended already generated fixed target prefix unchanged exactly
   once before the final dynamic prompt. Do not format Sempai's return again.
   Re-tokenize/check the effective provider layout against that target's
   prefix and non-prefix prompt ceilings and full context/output. A smaller draft
   or Sempai's approval does not establish the final request fits. This check also
   runs when review is disabled, returns unchanged text or produces no edit.
5. Perform the same unconditional check at the shared last pre-dispatch boundary,
   after provider transforms and before any real request/retry/tool continuation.
   Detect changed target/policy/envelope identities; no stale check, hidden replay,
   silent history truncation or fallback to the original oversized prompt. A bounded
   explicit repair Recipe can propose another candidate; it cannot skip the guard.
   Retaining an old task policy preserves evidence and selection, not permission
   to exceed a newly tightened live hard ceiling. Block/reconcile that dispatch
   explicitly without swapping the task's selected prefix or replaying effects.

Post-turn review uses the same Sempai-target guard without replaying original
effects. Prefix-Maker/design calls use their own target guard as well. Python/
Recipes own evidence selection, permitted edits and repair decisions; shared host
preparation/transport verifies actual identities and technical fit. Reuse one
technical guard, rather than independent interceptor, CLI and provider algorithms.
The current settings map and planning draft do **not** establish this runtime
wiring; it remains a required generation/serving and Sempai cutover gate.

Acceptance must run with budgeting both on and off: values survive toggle/restart;
Kohai obtains both providers' limits; distinct-provider limits are not interchanged;
oversized Sempai input causes zero Sempai calls; a Sempai edit that fits alone but
overflows with history/tools/prefix causes zero final provider calls; unchanged/
disabled review cannot bypass checks; exact boundary and one-token-over cases are
checked after real adapter rendering, including continuations and policy races.

Cutover prerequisite: enumerate the actual active answer/Sempai/design targets and
populate/verify their canonical policies through the revisioned provider owner
before enabling the successor serving path. Preserve existing explicit values;
never backfill a prefix ceiling from `max_position_embeddings`, an optional budget
or an unrelated provider's setting. Missing values remain Setup required, with
read-only configuration/source inspection available and model dispatch blocked.
An old application that cannot read these fields must be upgraded before a write;
do not bypass its API with raw incompatible JSON. No optional "disable hard
limits" switch is introduced. Test the staged upgrade and restart readback as
well as fresh installation; retained task policy references remain readable.

Model capacity is not prefix capacity. The retained Ornith configuration's
262,144 position limit describes a model artifact; the observed server limit is
131,072 for the whole sequence. Neither sets `max_prefix_tokens`. The requested
operator policy remains 100,000 prefix and 14,688 non-prefix prompt tokens, keyed
by the exact provider/model serving target. Do not replace it with 262,144 or
copy it into a second editable prefix-database record. Prefix records reference
the canonical provider revision and retain its historical values as evidence.

Apply the following contract to **every** profile, including base, Sempai and
Prefix-Maker, and to Generate, Regenerate, Redesign and Generate model design:

1. Resolve the consumer's actual provider/model, canonical limits and revision,
   selected accepted design, tokenizer/template/injection adapter and verified
   deployed context/output constraints before selecting model-visible content.
   Kohai, Sempai and the design provider may be three different targets. Missing
   policy or geometry is an explicit prerequisite for a target-facing build;
   source-only compilation and diagnostic measurements may still proceed.
2. Derive the rendering budget from the smaller of the configured prefix ceiling,
   the profile generation request's requested size and deployed context minus the permitted dynamic
   prompt, required output and any separately accounted technical framing.
   Account for framing exactly once. Configuration position capacity is an
   additional compatibility bound, never evidence of deployed capacity. Reject
   impossible reservations; do not silently reduce the promised prompt/output.
   A design request is immutable build input, not another model-limit setting.
3. The accepted profile/content rules, bound to the shared pair-format design, must
   specify budget-dependent presentation:
   mandatory coverage, whole-unit/dependency groups, optional-unit priorities and
   stable tie-breaking, navigation/citation overhead, deduplication rules and
   qualified render/selection strategy. Every supported budget band must preserve
   required contracts, negative rules and usable complete examples. More space
   permits additional useful examples/reference units; less space cannot discard
   required safety or authoring knowledge. Preserve complete original sources,
   selected/omitted dispositions and provenance outside the model-visible view.
4. Generate/Regenerate executes those accepted rules without a provider call.
   Equal source/design/target/**policy revision** inputs yield identical bytes.
   A changed limit can select another already accepted budget band; it cannot
   authorize new summarization, changed priorities or lossy compression. If no
   accepted design covers the new budget, report Needs redesign. If mandatory
   content cannot fit, report Capacity blocked with the exact failing coverage.
5. Redesign and Generate model design receive these limits as typed trusted
   inputs and review actual current content, architecture and coverage at that
   budget. Validate the proposed rules and their candidate at the intended size,
   including smaller/larger supported bands. No permission to increase settings,
   alter vLLM, omit mandatory knowledge or accept the provider's fit assertion.
   Redesign returns the complete updated candidate and reviewed design together.
6. Exact measurement must check the complete selected prefix/generation or explicit
   composite for that consumer, with prefix injection once, actual tools/history/role framing and
   output reservation. Use a qualified adapter-defined prefix/dynamic boundary;
   token counts of separate text snippets are not generally additive. An LCP
   across arbitrary probes proves shared tokens, not a semantic prefix size.
   Validate prefix <= configured maximum, remaining prompt <= its maximum and
   whole request plus output <= deployed context. Diagnostic probes do not prove
   all future requests fit; final preparation repeats the check for the real input.
   Cache alignment, stable headers, indexes, citations and padding consume this
   budget too. A verified attention/recurrent checkpoint layout may guide complete
   unit packing and boundary placement; never round the artifact above its cap,
   split code/contracts to meet a chunk boundary or equate INT4 weights with KV
   precision/capacity. A fit result proves no hybrid-state restoration/cache hit.
7. A setting edit invalidates *new-work compatibility* and updates all settings
   surfaces through the existing owner. Never mutate accepted historical designs,
   artifacts, receipts or admitted task selections. Revalidate a candidate against
   current desired policy before acceptance/activation; stale revision conflicts
   cannot silently activate an older policy. Retained tasks keep their exact
   request/design evidence, subject to current technical dispatch constraints.
   Keep provider/policy revision IDs and timestamps in manifests/receipts, outside
   the stable teaching text. A new settings revision need not change rendered
   bytes if effective limits/content/design did not change. Reuse an identical
   artifact only after compatibility checks; neither gratuitous metadata churn
   nor ignoring a genuinely changed budget is a stable-prefix strategy. Cache
   namespaces still require qualified model/template/token/state compatibility.

**Implementation audit:** the packaged `measure-prefix-template-and-capacity`
Recipe and `assess_prefix_capacity` currently establish only complete-probe plus
explicit-reserve arithmetic. Keep their version-1 request/result contracts and
historical receipts readable. They do not resolve the canonical provider policy,
identify a prefix/dynamic partition, select budget-aware knowledge or qualify
serving. Implement successor composition from shared policy-read, pure budget
planning, accepted selection, render, exact encoding, partition/capacity checking
and immutable receipt usages; preserve independent Tool calls as separate steps.
Do not relabel an old `fits: true` result as per-model policy compliance.

The new reusable class-22 draft `plan-prefix-model-budget` in
`prefix_compiler_components.rs`, backed by `prefix_model_budget.py`, separates
planning from that measurement-v1 contract. Its typed inputs are a retained
`policy_ref`, the two canonical ceilings, deployed context, output reservation,
additional framing and a requested design size. It returns the effective budget
and explicit reservation/request shortfalls, without selecting content or claiming
fit/approval. Additional framing must be outside the measured prefix and remaining
prompt; normally zero when the prompt measurement already includes all framing.
The provider owner must resolve and authenticate `policy_ref`; this pure export
does not turn caller-supplied values into trusted policy. Constructor/Monty checks
passed; no production Recipe is activated by packaging the draft.
Wire it into the successor generation/design Recipes through normal exact pure
qualification and retained selection, followed by real rendering/partition checks.

Bounded local evidence (2026-10-22):
`target/validation/prefix-registry-artifacts-u4w40vr0/accepted.json` records strict
affected-library lint, worker/test builds and seven focused cases, including actual
Monty budget arithmetic and the real PostgreSQL/provider-service limit owner.
The latter checks Ornith 100,000/14,688, revision conflicts, preservation, clearing
and readback. Exact/one-token context changes, separate prefix caps, requested
design sizes, framing, impossible reservations and disabled prefix capacity are
planner cases; they do not establish final rendered-partition enforcement.
`target/validation/prefix-model-token-limits-ui-ld6sv9d2/accepted.json` records
37 actual-source helper/state tests and syntax checks, including selection retention
across a provider revision update. No browser, external-CLI push, installed-instance
write, generation/activation or Kohai/Sempai dispatch guard acceptance follows.
The frozen native scope overlays the corrected planner declaration onto
`prefix-registry-source-05qej4ht`; the newer UI revision-selection correction has
its separate receipt. Failed handler-registration/schema snapshots and logs remain
retained rather than being rewritten as passing. Current remote application/API
upgrade and verified canonical policy write remain prerequisites for live use.

Acceptance must cover two providers serving the same named model with different
limits, two model/tokenizer designs, exact-boundary fits and one-token overruns of
each ceiling, impossible output reservations, missing policy, base/domain aggregate
overflow, mandatory coverage overflow, accepted budget-band transitions, unsupported
bands, deterministic rebuild, policy-edit races, retained old tasks, and held-out
authoring/diagnostic quality at each accepted size. Show source-only, measured,
policy-compatible, quality-qualified and effective states separately. Persistence
and editor checks alone do not satisfy these generation/serving gates.

**Single settings owner (2026-10-22 implementation):** mutable per-model input
ceilings belong exclusively to `ProviderDefinition.model_token_limits[model]` in
the existing `brassclaw_llm_providers.definition` JSON and provider revision/CAS
contract. Both Inference and Prefix settings reuse `ModelTokenLimits`, the same
`["llm-providers"]` query, and existing GET/POST
`/api/webchat/v2/llm/providers`. No second settings table, config key, model-info
writer or prefix-specific settings endpoint is permitted. The fields are
`max_prefix_tokens` and `max_prompt_tokens` (complete remaining rendered input,
including history/tools/framing). Combined input is their checked sum, never a
third editable value. Output allowance and physical/deployed context remain
distinct. The requested current Ornith policy is 100,000 + 14,688 = 114,688;
it is instance configuration, not a global hardcoded model default.

Omission preserves the map; `{}` explicitly clears it; null/duplicate model names,
invalid numbers, overflowing totals and totals above a known default-model context
are rejected. All edits use the provider revision and preserve unrelated model
entries/credentials/selection. A stale editor reports a revision conflict rather
than overwriting another page's change. Successful server snapshots update the
shared query immediately; mounted prefix catalogue views refresh after provider
edits. Add the same refresh to future CLI/server-push consumers using the existing
settings notification owner; do not add a second mutable cache or private polling
loop. Live enforcement in compilation/final prompt preparation must consume this
canonical revision, with no copied per-profile/model-info setting.

Model specifications, accepted design records, queued measurements and actual
receipts retain immutable historical pins/observations. They are not additional
editable owners and must be labelled with their provider revision. New captures
include the selected model's policy from that same provider revision. Do not rewrite
historical evidence after a setting changes. A design's requested prefix size may
be lower than the current model ceiling; it cannot increase or replace that ceiling.
Known model-context limits and diagnostic measurement assumptions must remain
visibly separate from these settings. See [model token limits](../reborn/model-token-limits.md).

Prefix length, formatting and cache strategy are model-dependent. Persist model
specifications and design revisions in PostgreSQL through the final provider catalogue
and prefix stores, not browser state, environment variables or a second provider
registry. Reuse/extend the DB provider/model identity contract from simplified-v3 §10;
define migrations/DTOs explicitly where current storage lacks these records.

| Proposed record | Required contract |
| --- | --- |
| Model specification revision | Stable registered model identity plus exact weights/config/tokenizer/template identities, technical context/output limits, supported roles/tools/thinking/grammar, architecture and attention/recurrent-state facts with versioned evidence and Unknown where unavailable. Display name alone is not identity. |
| Target runtime/cache revision | DB provider/model binding, injection adapter, actual deployment/backend/KV and recurrent dtypes, group/page/chunk/checkpoint layout, effective limits and capability/qualification receipts. Model architecture and deployment-specific cache settings remain separate. |
| Model design revision | Immutable provider/model-pair design primarily defining supported prefix and dynamic-prompt formatting standards, framing/join rules, tokenizer/cache compatibility and references to canonical prefix/prompt limits and context/output facts. Record supported whole-unit compilation/teaching options and their compiler revisions without embedding role knowledge. Generator, Kohai and Sempai use the same accepted pair-design revision when targeting the same pair; Kohai applies its dynamic-prompt format before optional review and its exact-prefix join after Sempai returns the final dynamic prompt, without reformatting that return. No role-specific duplicate design, mutable formatter store or editable limit copy. Prefix representation is resolved at generation. Link exact model specification, provenance, compatible runtime evidence and Prefix-Maker proposal/review/schema/hash. |
| Profile/model design selection | Revision-checked mapping from a role/content profile to its provider/model-pair design and coverage requirements. Different role prefixes for one pair share the pair design; content selection does not create another copy of limits or formatting. |

A provider/model pair can have multiple historical design revisions. Profile-specific
coverage/content choices and smaller generation targets do not override the pair's
formatting contract or hard ceilings. Do not make one global token target apply
to every pair or copy a pair design for each role. Shared defaults are immutable
reusable references, not copied mutable blobs.
Record measured counts and full-envelope/output allowance; token budgets disabled
does not remove model context limits. A supported chunk-alignment strategy may be
part of a target-specific design, but changing runtime cache settings invalidates its
compatibility evidence without inventing different underlying model architecture.

**2026-10-26 source-reader prerequisite completed:** the existing artifact GET and
inspector now expose exact template alternatives, original-file and decoded-template
hashes, tokenizer/special-token declarations and declared memory/window fields.
See the [scoped acceptance](../validation/2026-10-26-prefix-format-sources.md).
This does not derive a default, fill/accept a design or prove current serving/cache
facts. The [shipped Prefix-Maker teaching successor](../validation/2026-10-26-prefix-maker-construction-teaching.md)
now reconciles the old server-injection/adjusted-template instructions and has exact
offline export plus actual native packaged-reference/import/sourcejob acceptance.
The then-qualified teaching described the earlier review-before-format order;
the operator's later correction supersedes that directive: Kohai formats the
dynamic prompt first, optional Sempai returns its corrected final dynamic prompt,
then Kohai joins the unchanged prefix without reformatting the return. Historical
teaching receipts retain their exact old bytes; revised teaching requires its own
applicability check. Neither receipt implements those runtime consumers. The [V180 draft successor](../validation/2026-10-26-prefix-pair-construction.md)
now extends that same proposal owner with a role-independent, retained-artifact-bound
construction format, actual operator API/UI import and strict literal worked cases.
It preserves historical profile v1/v2 records and the old acceptance FK; it does not
supply an accepted formatter or machine-ready runtime design. The
[complete-case successor](../validation/2026-10-26-prefix-pair-cases.md) adds the
source-owned operator GET `/api/webchat/v2/prefix-model-designs/proposals/{specification}/{proposal}/cases`
and canonical `brassclaw-prefix-pair-case-packet/1`: retain all cases, original
ordered context JSON and literal expected prefix/full-render strings. Whole-packet
capacity failures block export; never omit or shorten cases. The candidate Recipe
`compare-prefix-pair-default-template-cases` prepares that packet, dispatches the
existing UTF-8 digest usage and existing original-template encoding usage in
separate binding/executable steps, then compares complete rendered hashes/token
IDs and fixed-prefix token partitions in reusable pure PythonCode. It supplies no
replacement template. The four-context independent HF/native/Monty evidence checks
these stages and the inspected Recipe graph. The subsequent
[whole-Recipe/journal prerequisite](../validation/2026-10-26-prefix-pair-journal.md)
now executes that complete graph through the actual contained global Python root,
registered native usages, kernel and PostgreSQL journal. Its source-owned capture
and receipt verifier compare exact native selection/arguments/answers/attempts,
all pure/native input-result handoffs using each step's captured limits, and the
acknowledged worker/preparation identity. A valid text mismatch completes with a
negative result; a missing leading prefix fails at encoding before comparison.
Forged receipts, unrelated runs, foreign source namespaces and wrong preparation
revisions are rejected. These checks are test-only development prerequisites,
not installed/bootstrap workflow qualification or an ordinary admitted public job.

**Bootstrap prerequisite completed with scoped R4 acceptance:** the strict
missing-leading-prefix oracle now follows the actual `InputEncode` →
`InvalidInput` mapping. The exact packaged six-case qualification, retained
journal/source/step-bound checks, forged-record and incomplete-intent rejection,
no-replay readback and existing full native Prefix integration pass. See the
[acceptance and preserved R1–R3 failures](../validation/2026-10-26-prefix-pair-bootstrap.md).
This removes the attester startup blocker. It enables no public Pair job or
provider/model design by itself; the ordinary private caller remains the next
implementation step at that baseline. The later [private admission](../validation/2026-10-27-prefix-pair-private-admission.md)
and [operator diagnostic](../validation/2026-10-27-prefix-pair-operator.md) receipts
now close that private/public diagnostic wiring. They retain the separate
model-design/serving gates.

**Grouped model declarations — scoped inspector acceptance:** the
[validated successor](../validation/2026-10-26-prefix-grouped-quantization.md) closes
the earlier projection gap: direct AWQ-style quantization fields alone
omitted `compressed-tensors` group parameters. The frozen Ornith configuration
declares that format, four-bit weights in named groups, explicit excluded weights
and 24 linear/8 full-attention layers. The existing read-only inspector now
retains source-resolvable group pointers,
targets, weight/activation parameters and exclusions. Preserve missing versus
null and exact original hashes; never infer effective attention-KV/recurrent
precision or serving capacity from these declarations. Validate grouped and
nested configurations, escaped group identifiers and exclusion of unrelated
fields through the existing projection/caller checks. The guarded single-file
successor now passes all nine focused tests, including group/nested/pointer cases
and actual frozen-original PostgreSQL readback, using the combined emitted image
and applicable strict lint. It is separately scoped from Pair bootstrap R4 and
from the other chat's overall Auth/Secret acceptance. Accepted designs, compiler/
Kohai consumers and effective runtime/cache evidence remain open.

Before exposing a case-check action or consuming its result for acceptance:

1. **Completed for the exact packaged graph:** installation-owned noncircular
   bootstrap now qualifies its Recipe, pure-export/dependency closure and existing
   usage/native combinations. Reuse that qualified program and its exact proof;
   changed/authored graphs need their own required qualification. Preserve installed
   overrides and retained task selections; this is not general pure approval.
2. Continue from the [V181 pair-target metadata prerequisite](../validation/2026-10-26-prefix-pair-job-targets.md),
   which is implemented in the existing immutable job owner. Rust represents
   Profile and Pair explicitly; only `check_cases` with target mode permits both
   profile columns to be absent. Existing profile/FK/positive-revision and
   immutable-row contracts remain enforced. Pair request metadata references exact
   specification/proposal IDs; initial capture must equal the complete source-owned
   packet. Same-ID/request retries retain the first program/input, changed explicit
   inputs conflict, and concurrent distinct targets yield one original/one conflict.
   No profile sentinel or settings copy is needed. Metadata is not admission:
   the previously accepted metadata path did not execute Pair. The new seven-file
   successor now connects private Pair admission/selection/receipt handling in
   source, but awaits compilation and actual native acceptance. Profile inspection
   still rejects Pair and no public pair-job route is enabled. Qualify the same
   private ResultOnly admission/selector/settlement path, then expose pair-aware status through
   the existing facade/read projections, validating original pair identity/namespace.
   Retain the original captured selection/input/result on reconnect. Do not replace
   these completed storage contracts, accept partial caller packets, copy limits,
   or introduce a public matching/queue/VM alternative.
3. Promote/reuse the exact tested source-owned capture and journal verifier in
   that qualified caller, preserving its integrity scope rather than moving
   preparation/comparison decisions into Rust. Verify actual retained digest and
   original-template encoding dispatch journals, exact inputs/results with each
   step's captured bounds, worker and acknowledged preparation revision before
   trusting the comparison. The pure comparison JSON alone proves no native
   provenance. Preserve resource/unknown/cancelled outcomes; no truncation, fallback,
   limit increase or replay to manufacture success. A mismatched literal remains a
   failed case; a template/preparation failure remains a failed measurement.
4. Retain complete per-case results with their original job; reconnect/readback
   never redispatches completed work. Expose the scoped `retained-template-cases-only`
   qualification. This does not prove final provider wire, full-prefix fit, live
   memory/cache behavior or answer quality; those retain their own gates.

Broader provider/documentation archive bindings, deterministic registered
construction options, final provider-envelope checks and trusted acceptance also
remain prerequisites. Implement them in this same owner before enabling
model-design generation, then
connect the accepted pair revision to compiler and Kohai. Keep role knowledge and
role-prefix candidates separate, preserve historical records, and do not copy
mutable limits or pair formats into each role. Do not replace these remaining
consumer steps with another research-note store or treat draft import/inspection as
model-design generation or serving acceptance.

**Primary design-generation requirement:** fill the formatting parts with the
[concrete construction specification](../reborn/model-token-limits.md#formatting-design-must-contain-construction-instructions).
A format label/standards name plus limits is not an accepted design. Acquire the
selected provider's exposed defaults/facts and research the exact model/tokenizer/
default-template and backend/cache documentation through existing preparation
usages. Preserve immutable source bytes/revisions/hashes and map each saved rule
to that evidence. Base-model or quantized-derivative claims require exact target
matching; public defaults do not prove the currently effective endpoint mapping.

Implement the planned Generate model design Recipe through the existing admission
and component owners, ordering discovery/research/archive →
evidence/target reconciliation → Prefix-Maker derivation of supported declarative
prefix and dynamic-prompt construction rules → exact case/token/envelope checks →
retained proposal/acceptance/binding. Include literal delimiters/framing ownership,
template branches for tools/history/reasoning/multimodal inputs, exact join rules,
capacity references and evidenced memory/cache packing constraints with units.
Record actual architecture/cache differences and Unknowns, not assumed generic
ChatML or INT4 KV geometry. Model-generated rules cannot install arbitrary code,
change server settings or enable unsupported adapters. Explicitly qualify any
missing schema/store/formatter support in the existing owners before accepting a
filled design. Profile knowledge-selection rules remain separate from pair format.

Acceptance requires complete usable construction cases and traceability, not
nonempty text or an LLM assertion. Show template matching, input fit, cache behavior
and held-out answer quality separately. The compiler and Kohai must actually consume
the accepted saved construction instructions; storing research notes without those
consumer paths does not close model design or serving. Normal Generate uses the
saved design deterministically and performs no hidden research/provider call.

Populate the pair design from provider-supplied metadata and supported API discovery
where available, plus versioned official model/provider documentation. Record each
fact's origin, observed revision and whether it is reported, operator-supplied or
independently verified. Do not assume a model-list endpoint supplies tokenizer,
formatting standards, cache geometry or both input ceilings. Missing facts can be
filled manually through the existing model/design settings; unknown facts remain
visible and block only operations requiring them. Refresh produces a reviewed
successor and never silently overwrites manual settings or changes an active design.

Expose manual edits of `max_prefix_tokens` and `max_prompt_tokens` for **every
registered provider/model pair**, including inactive pairs, through the existing
provider revision/CAS endpoint and shared settings query described above. Both
Inference and Prefix/model-design views edit that same canonical map entry; design
views show linked current policy plus their historical pinned policy, not another
editable limit record. Validate edits against known physical constraints; raising
a configured ceiling does not enlarge the provider's actual context. A policy edit
must visibly mark affected artifacts/design compatibility for revalidation and
generation/regeneration as needed; never mutate fixed prefix characters in place.
Running tasks retain design/artifact pins but still respect live hard-limit checks.

Add a Tier-1 Generate model design Recipe: select an explicitly registered model/target
(including an inactive role target),
collect verified architecture/runtime evidence, use a fresh compatible Prefix-Maker
on the design provider, retain the proposed declarative design, validate supported
rendering/capacity options and trial examples, then expose it for acceptance. This
creates design records, not a prefix for every profile. Generate on a profile resolves
the accepted profile/model selection and compiles current approved content model-free.
Redesign additionally reviews that profile's knowledge and produces its complete
updated generation. Bootstrap model designs are reviewed shipped records and need
no provider call; unsupported new targets are explicit setup/design prerequisites.

Provider changes, Kohai model changes and Sempai provider/model changes resolve the
appropriate consumer's design and prefix generation independently. Prefix-Maker also
needs a generation compatible with its actual design-provider model, which may differ
from the model it is designing for. Never reuse an Ornith prefix/design solely because
another provider has the same friendly model name. Reuse exact compatible records
where evidence permits; rebuild rendering/capacity when tokenizer/template/effective
limits differ, and requalify caches when only runtime/cache identity differs.

On a provider/model edit, show affected consumers/profiles and accepted-compatible,
Needs generation, Needs model design or Incompatible states. An explicit confirmed
switch workflow may queue model-free regeneration for accepted designs; do not hide
provider-assisted redesign calls inside Generate or provider settings save. If no
compatible prefix is ready, keep the previous working selection until a staged switch
commits, or expose the newly selected target as blocked under its explicit lifecycle.
Never silently serve the old incompatible prefix, downgrade to a tiny fallback or
disable deterministic Tier-0/management paths. Unsupported architecture, rendering options or decoder contracts require adapter
implementation and qualification before a design can be accepted. A provider-produced
design cannot add compiler features or silently ignore unknown options. The current
Ornith-specific adapter is not evidence of general multi-model compilation.
New tasks use revision-checked selected
targets/designs; retained tasks keep pinned references subject to the provider's live
availability/policy contract. Preserve old model designs and generation history.

## 6. Installation-time DB seeding: preferred combined approach

Use the database as the installed source catalogue, with a shipped reproducible
reference package as its seed input. The compiler consumes an immutable export;
installed systems do not require the developer checkout or public network access
to reconstruct the base prefix.

1. Release packaging produces a content-addressed source package containing the
   current document inventory, full text, release/commit identity, checksums,
   compiler/profile revisions, source classifications and licenses where needed.
   Existing first-party executable component seeding remains authoritative.
2. Supply reference-source registration as a seed participant to the one
   `initialize_runtime_components` sequence owned by [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben)
   (boot prerequisites). That owner performs ordering after migrations/component
   seeding and before integrity/readiness; do not add a second initializer or a
   WebUI-only seeding/startup path.
   Reuse `reborn_docus` and its supported interfaces for compatible document records;
   add reviewed schema support for release/provenance/immutable revisions rather
   than inventing fields that current constructors do not accept.
3. Keep system-seed integrity and human Q2 for authored changes distinct. Shipped
   system references are checked against their release manifest; user imports/
   edits enter the supported validation path. Merely transferring a document into
   a DB or executing an upsert does not make it approved evidence.
4. Seeding is idempotent and preserves operator edits/older approved revisions.
   Upgrades create new revisions and atomically select eligible source revisions
   under the applicable release-integrity/approval contract. They invalidate affected
   prefix dependencies; they do not activate new compiled prefixes or overwrite a
   body used by a running task. Never seed generated
   summaries back as authoritative originals.
5. Register the six seeded profile definitions and compiler artifacts at installation.
   Seed reviewed model specifications/design defaults only for supported pinned
   targets with provenance; do not guess architecture for arbitrary provider names.
   Preserve operator model-design revisions, accepted bindings and history on upgrade.
   Optionally import an existing verified domain generation. External domain source
   fetches and expensive generation are operator jobs, not boot-time network/GPU
   prerequisites. A missing domain bundle is visible as Not generated.
   Seed Sempai source/procedure metadata with its own checksum manifest. An optional
   imported reviewer generation must satisfy the same source/freshness checks;
   presence of reference text does not activate authored components or Q2 evidence.
6. Base-prefix generation follows successful boot prerequisites through the accepted
   model-free generation Recipe. An initial background job or packaged compatible generation
   may be used, with explicit readiness state. Do not manufacture a Monty global
   lifecycle in a WebUI constructor; honor simplified-v3 Phase 3a's real handshake.

Separate source catalogue, compiler artifacts and compiled prefix storage. Storing
complete originals does not eliminate the need to persist assembled prefix text.
Persisting a prefix does not eliminate raw evidence or installation seeds.

## 7. Persistence and generation lifecycle

Design forward migrations after the actual migration head. Do not rewrite V063,
V085, V087 or V088 on an installed system. The following are proposed contracts,
not existing tables/API fields:

| Record | Required content |
| --- | --- |
| Prefix profile | Stable profile ID, display metadata, registered compiler/version/checksum, source/coverage policy, model targets and supported validation adapters. |
| Design revision/proposal | Shared pair-format revision and separate profile-content rule references/checksums, explicit predecessor/bindings, pinned Prefix-Maker/design-provider/model-specification/source/target references, raw proposal, scoped impact/diff, exact candidate generation and acceptance evidence. Existing profile-bound proposal storage requires a compatible owning-layer migration, not duplicated mutable pair settings. A draft/status label is not acceptance. |
| Model specification/design selection | DB-owned versioned records and mappings from section 5.3, with explicit compatibility/Unknown states and retained revisions; no browser-only model-formatting configuration. |
| Source registration/observation | Versioned profile-source identities and applicability/review state plus separate append-only availability/content-check receipts from section 8.1. Generation manifests pin used revisions; transient observations do not alter source or generation identity. |
| Prefix generation | Content-addressed ID, profile ID, compiler/input snapshot identities, full bundle text, evidence cards, immutable source/dependency manifest, model/tokenizer/template hashes, exact token counts and quality results. |
| Generation job | Prefix request/profile, pinned source/design/target references, stage/artifact progress and original shared admission/run/invocation references. Execution attempt, cancellation/fencing, durable counts and effect certainty come from the runtime owner; any displayed copies are non-authoritative projections. Prefix compiler artifacts are not VM continuation checkpoints. |
| Active selection | Provider/model/consumer binding to one immutable generation or explicitly compiled composite; desired/effective state and revision for compare-and-swap. |
| Cache observation | Provider process/model identity, generation/token-prefix hash, runtime/cache-layout fingerprint and salt policy, expected-content checks, native/extra-external hits, warm/reload measurements and time; never confused with generation success. |

Migrate existing `reborn_basic_prompt_store` bundles as legacy base-prompt generations.
Keep compatibility reads until both Kohai and Sempai use the new resolver. A legacy
bundle without evidence/tokenizer provenance is labelled Legacy / Unverified;
import does not synthesize missing verification. The old scope-only uniqueness
constraint cannot distinguish named profile identities: add the actual name/selection
dimension or introduce generation storage with a clearly defined compatibility
pointer. Do not assert additive rows already solve this.

Job states: queued, snapshotting, collecting, compiling, validating, completed,
failed or cancelled; represent prerequisite/capacity blocking explicitly with a
reason and resumable dependencies rather than a misleading queued state. Generate
operations may contain parent and composite child jobs. Activation and cache
states are separate. Persist real stage
progress and counts; report a percentage only with a known denominator. A browser
disconnect does not cancel admitted work. Deduplicate repeated submissions and
serialize conflicting profile/target mutations, while allowing independent builds
within configured resources.

Use [simplified Phase 0a](../../simplified_v3.md#phase-0a--ground-truth-komponentenverträge-implementieren) items 9–10 for the shared failure/invocation
contract and [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben) for its admission, cancellation, durable
intent/count/effect records and recovery implementation. Prefix must not add its
own turn queue, permanent claim/retry ledger, persisted BuildInstruction or
supervisor. A generation job stores product stages/artifact references linked to
those original executions. Browser reconnect reads that status; it never admits
another run or resets an invocation. A request UUID deduplicates operator job
submission, not uncertain Tool effects.

Prefix-specific work is to provide operation status/artifact lookup and publication
receipts under the existing runtime contract, defining how an already-built artifact
or committed generation is reconciled without repeating the effect. Persist prefix
records and their original execution links through supported transactions; no
DB-plus-provider/filesystem atomicity is assumed. The shared owner retains counts,
keys, pinned selections and confirmed/unresolved effect certainty across recovery.
Unknown outcomes stay unresolved until that supported reconciliation establishes
what happened; output/reply/warm failure never permits publication/build replay.

Until the shared durable retry adapter is implemented and qualified, ordinary
retained prefix usages remain `stop`/`max_attempts: 1`. Safe read-only/deduplication
evidence alone cannot enable retries. Consume the shared live policy/settings and
attempt-freshness checks before dispatch, including after a wait; do not implement
retry loops in compiler adapters, prefix Recipes or UI reconnect handlers.

Publish only after all required evidence/coverage/token checks pass. Publication is
atomic; a separate selection change is independently atomic and revision-checked.
Publication does not change active selections. Failed regeneration leaves the previous immutable
generation intact. A completed publish is reused after downstream reply/warm failure;
never rerun its effects because an HTTP response was lost. Cancellation, policy,
attempt fencing and durable retry counts apply throughout. Artifact retention follows
active selections, jobs and resumable task references, not “keep newest only”.

Staleness is dependency-aware: relevant approved component activation, release-doc,
source, compiler, tokenizer or template change marks only affected generations.
Replace best-effort staleness updates with durable invalidation/reconciliation.
Expose separate Source outdated, Incompatible and Revoked states. Retain last-good
generations for rollback and tasks that already pinned them; retention does not make
an outdated generation eligible for a new task. New tasks require a generation
matching the current applicable source/selection revision. Rebuild and queue affected
model work, or return an explicit Prefix rebuilding / blocked result. Recheck
freshness and pin the generation atomically against that revision so an activation
race cannot admit new work with stale context. Existing continuations retain their
pinned generation after ordinary replacement; explicit revocation/incompatibility
blocks affected dispatch with a classified error. Make this policy explicit and
never silently substitute the current tiny fallback. This follows the new-task
freshness requirement in simplified-v3's acceptance matrix.

## 8. WebUI and API changes

Extend the existing Prefix tab rather than its frontend-only list. All six seeded profiles and registered custom profiles
appear even before their first build. Display a useful domain description, generated
and active generation, source freshness, build status, last build duration,
rendered tokens, coverage gaps, validation status and per-provider cache status.
Keep ordinary UI copy about knowledge and readiness; expose hashes/geometry and
raw manifests in an expandable details area.

Before Generate, show the resolved model/design revision, compiler adapter and source
snapshot, requested/effective prefix capacity, workspace/output reserve, coverage
policy and evidence-display mode. Distinguish stored defaults from experimental
settings. Unsupported options, stale selection policies and incompatible ordered
consumer grammars block readiness with their actual prerequisite; do not silently
ignore them or ask the operator to repair runtime services. The UI resolves DB-owned
inputs, rather than copying the standalone shell environment from §4.2.

Actions: Generate / Regenerate for each profile; View job / Cancel while running;
View sources and validation; Activate a chosen compatible generation; Warm /
Verify cache when the provider supports it; Roll back to a retained generation.
Generating a prefix must not automatically replace an unrelated provider's active
domain. Do not mark “fresh” optimistically from a job admission response.

Place a button labelled **Redesign** immediately beside Generate / Regenerate on
every seeded/custom profile row. Before admission, open an accessible confirmation
popup naming the profile/current design, design provider/model and target, permitted
source data sent to that provider, requested goal and the expected outcome: refreshed
knowledge plus a model-specific design and complete updated candidate generation for
review, with no active prefix changed. Offer Cancel and Confirm redesign;
Escape/cancel makes no job or provider call. Revalidate prerequisites and exact
revisions server-side on confirmation. If they changed, require confirmation of the
updated request. Persist the prefix-specific confirmed request identity and
link it to shared admission's idempotent request/run before dispatch; consume
[simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben) for claims, execution fencing and recovery. Double clicks or
reconnects read the original request/run; prefix must not introduce a second
execution claim or retry mechanism to prevent duplicate effects.
This is the requested UX confirmation, not a per-Tool authorization lease; current
global policy, external authentication and technical limits still apply independently.

Redesign shows content/architecture refresh, durable progress, raw design/content diff,
complete candidate validation and a separate Accept redesign action. Failed/cancelled/
rejected proposals preserve the selected design, generated artifacts and active
selection. Acceptance records the new model-specific design and publishes the exact
updated candidate generation; no extra Generate click is required to obtain it.
Activation remains separate. Source-only refresh does not change design; Redesign
includes current knowledge refresh, while Regenerate never asks a model to rewrite
the design. Missing provider/fresh Prefix-Maker/target evidence disables Redesign with
an actionable explanation while offline Generate remains available.

Add a **Create Prefix** tab within this same settings section, alongside the existing
profile listing. Collect display name, purpose, consumer, target and registered source
references/authorized import requests, mandatory coverage and optional design goals;
allow selection of the design provider. Allocate stable profile identity server-side;
names are not paths or compiler entry points. Show a confirmation summary before the
provider-assisted Create design job. Validate duplicate/conflicting names, unsupported
consumers/adapters and source access before dispatch. Draft profiles are visibly Draft,
not eligible serving selections. On validated human acceptance, register the custom
profile/design, then expose it in the same list with Generate and Redesign. Registration,
generation and activation remain distinct. Seed upgrades cannot overwrite custom
profiles/designs; removed drafts cannot break retained job/artifact references.

Add a **Model Designs** area/tab in the same Prefix settings section. Its selector
lists eligible DB-registered provider/model targets, including models not currently
selected for any active role. Explicitly identify the design target, whether it
is active, its retained/unknown format/limits and the separate design provider.
Selection of an inactive role target is sufficient for offline generation when
its required pinned inputs exist; it never activates a model/provider. Disabled
or deleted providers remain unavailable for inference, while retained artifacts
can remain inspectable/offline inputs under their existing eligibility contract. **Generate model
design** opens a confirmation summary, then admits the section 5.3 Tier-1 Recipe.
List already generated model designs, including retained designs for inactive models,
with model/provider identity, specification/design revision, formatting summary,
prefix-size/reserve rules, evidence provenance, validation/acceptance status, compatible
targets, profile bindings and affected/stale generations. Show unknown architecture
and missing prerequisites rather than guessing cache features. Details expose hashes
and geometry; ordinary copy explains what will fit and which profiles use the design.

Offer View/Review, Accept model design and explicit profile-binding controls; proposals
do not overwrite accepted records. Review the impact of selecting another design on
Kohai, Sempai and Prefix-Maker separately, and offer explicit Generate/Redesign actions
for affected profiles. Provider/model changes surface compatibility and missing-build
states here. Merely selecting a model in the form makes no provider call, prefix build,
deployment change or active selection mutation. The list comes from persisted records
and survives reload/restart, including historical designs and their receipts.

The primary Generate action uses a registered saved build specification: profile,
consumer/role/task contract, explicit connected or WebUI-selected target/model
revision, default-format request adapter, canonical token-policy revision,
accepted source-refresh policy and reviewed coverage/teaching policy. A general
source-only build may explicitly have no target; a target-aware build may not
silently borrow the active model when the selected target is inactive. Operators do not compose shell flags or choose packing
algorithms. Missing choices are explicit setup prerequisites. Snapshot-only offline
building remains available without a provider. Advanced parent/composite actions
are not mandatory hidden steps for normal target-aware generation.

Show one operation timeline across parent/dependency/compile/validation stages with
exact blocked reasons and counts. Distinguish source refresh/review from compilation:
use an eligible pinned package or an explicitly authorized refresh policy; changed
unreviewed sources are quarantined, not auto-approved. Unchanged inputs deduplicate
to the same content-addressed generation while retaining the requested operation.
Explicit refresh/rebuild creates new immutable inputs, never overwrites evidence.

Details expose coverage and omitted units/reasons, source versions, actual capacity,
teacher checks, first-pass/repair scores, injection owner and cache qualification.
Show Cache unavailable, Unverified and Cache correctness failed distinctly. Failed
enabled caching is a serving prerequisite failure, not missing telemetry. Citation/
configuration/review support is per adapter; no universal “expert” badge follows
from compilation or a warm timestamp.

Retain the current API family. Proposed extensions, subject to final DTO review:

| Operation | Proposed route/behavior |
| --- | --- |
| List | Existing `GET /api/webchat/v2/prefixes`, now returning seeded/custom profiles plus current job/build/selection state. |
| Generate/regenerate | Existing `POST /api/webchat/v2/prefixes/{name}/regenerate`, admit work and return a durable job reference (`202`); one operation handles first and later builds. |
| Redesign | Proposed `POST /api/webchat/v2/prefixes/{name}/redesign-jobs`: confirmed typed request with exact existing design/generation, registered design-provider/target references and goals; returns durable job reference (`202`). |
| Create profile draft/design | Proposed `POST /api/webchat/v2/prefix-profile-drafts`: validated form and confirmed provider-assisted design request; allocates draft identity and returns its durable job reference. |
| Review/accept design | Proposed design-proposal detail and explicit acceptance mutations under the profile/draft family; require exact proposal/checksum, predecessor revision and validation/review evidence. No acceptance by arbitrary JSON overwrite. |
| Model design catalogue/jobs | Proposed `GET /api/webchat/v2/prefix-model-designs` and `POST /api/webchat/v2/prefix-model-design-jobs`; read persisted model/design records and admit confirmed design jobs using registered model/target/design-provider references. |
| Model design acceptance/binding | Proposed detail/acceptance and profile-model binding mutations under that API family; exact model/design/schema revisions, compatibility evidence and compare-and-swap, never browser-authored runtime facts. |
| Source inventory/registration | Proposed `GET /api/webchat/v2/prefixes/{name}/sources` and `POST /api/webchat/v2/prefixes/{name}/sources`; return generation provenance plus current registered sources, or save a validated new source registration. |
| Source availability check | Proposed `POST /api/webchat/v2/prefixes/{name}/source-check-jobs`; admit a bounded check of pinned registered source identities and return job/progress/result references, rather than performing network I/O in a GET handler. |
| Observe/cancel | Implemented `GET /api/webchat/v2/prefixes/{name}/jobs/{job}` projects the existing run/admission/effect records. Use its opaque `thread_id` and `job_id` with the existing run/event/cancel APIs; do not add another prefix-job lifecycle or cancellation store. |
| Offline model/design measurement | Implemented `POST /api/webchat/v2/prefixes/{name}/jobs/{job}` admits `operation=measure` through the same run owner and returns the current job projection. Explicit retained generation/proposal/artifact IDs and probe/limit assumptions; no acceptance or activation. |
| Generation details | `GET /api/webchat/v2/prefixes/{name}/generations/{id}` with manifest, source and quality projection. |
| Activation/warm | Explicit mutation operations under the same profile family with registered target identity and exact generation, not arbitrary host paths or shell commands. |

Changing synchronous regeneration to job admission requires updating facade DTOs,
all callers and contract tests together. Provide a documented compatibility path if
other clients depend on the old completion response. Extend the existing
`InterceptorConfigService`/`RebornServicesApi` seams, or extract a prefix port behind
that facade if needed; do not build a competing WebUI-only compiler service.

Thread private worker/store/provider handles through `RebornRuntimeInput`, runtime
factories and `build_webui_services`. Handlers remain thin facade calls. Update route
descriptors, router mounts, sanitized errors, API functions, hooks, tab components,
translations and contract tests as one vertical change. The server owns profile IDs,
compiler paths and source packs; the browser selects registered IDs and validated
settings. Keep authentication, origin/body limits and technical resource enforcement
at their existing boundaries without adding legacy feature-role authorization.

### 8.1 Sources button, source inventory and availability checks

Add a button labelled **Sources** beside Generate / Regenerate in each seeded/custom
prefix row, alongside Redesign. Clicking opens a profile-specific **Sources** tab in
the same settings section; preserve the selected profile/generation and provide a
clear return to the prefix list. This is a source-management view, not another prefix
compiler. Implement it through the existing facade, composition and ingress seams.

Show every source actually used by the displayed generation, including inherited
base/composite sources, official web documents, qualified forum/local findings,
DB component records, release documents and teaching packages. Derive this inventory
from the immutable source/evidence manifest, not a reconstructed list of URLs. Also
show current source registrations, newly saved sources and omissions separately, so
“used in this generation” cannot be confused with “available for the next build.”
Before the first generation, show the registered source catalogue with Not yet used.

Each row shows title/type, URL or safe registered reference, source/version identity,
availability indicator, last check time/result, archived snapshot availability,
review/disposition and usage in the selected generation. Use **green Available**
when the declared source/content can actually be retrieved/read through its adapter,
and **red Unavailable** for a completed check that cannot retrieve/read it. Add
neutral Checking / Not checked / Check error states with text and accessible labels;
no color-only meaning and no false red during an unfinished check. Explain reasons
such as Missing, Authentication required, Timeout or Rate limited. Availability,
content changed and knowledge approval are separate fields: green never means
authoritative, unchanged or approved. A dead upstream URL does not delete an intact
archived source or change a historical generation's provenance.

On tab opening, render the inventory immediately and admit a bounded, model-free
source-check Recipe against the pinned source registrations. Stream per-source
results; offer Check again. Coalesce repeated opens/concurrent tabs into an existing
compatible check job, enforce concurrency/time/byte/request limits and show the age
of previous observations while rechecking. Closing the tab stops UI subscriptions,
not a separately admitted durable job. No continuous background crawl is introduced.

For web sources, use a short content-aware fetch through the existing registered
HTTP/fetch primitive: bounded redirects and response size, expected media/extraction
adapter, and sufficient content checks to detect empty pages, login/error pages or
soft 404s. HEAD may precede GET where useful; ping/DNS/HTTP 200 alone cannot establish
that the referenced document still exists. A range/snippet can establish a limited
availability observation but cannot certify full-content identity. Compare complete
content hashes only when the supported bounded fetch actually retrieves the full
artifact; otherwise report Content identity not checked. Authentication uses existing
host secret resolution, never URL credentials or secrets stored in manifests.

For DB/release/local artifact sources, resolve the registered identity/revision and
verify read/access and retained artifact integrity without web requests or arbitrary
browser-supplied filesystem access. All actual checks obey current global Tool policy,
network/secret constraints and technical limits, including redirect destinations and
private-network targets under the existing supported policy. URLs remain typed data;
the check does not execute source content or scripts, crawl unrelated links or create
a provider call. Reuse a deterministic non-shell fetch/check adapter; shell variants
remain Tier 1 and cannot be the sole model-free path.

Provide a text field labelled **Add source** and a **Save source** button in the tab.
Accept an HTTP(S) URL or a supported registered document/component reference; offer
source type/title/applicability details where needed. Validate and canonicalize through
the supported adapter without stripping meaningful version/query identifiers. Reject
unsupported schemes, embedded credentials and duplicate conflicting registrations;
show duplicate existing sources instead of silently adding another row. Saving persists
an immutable/versioned profile-source registration in PostgreSQL, with stable identity,
origin, applicability, intended required/optional coverage and review status. It is
not an automatic fetch approval, redesign, build or activation. A subsequent bounded
availability check may run through the same admitted check path.

A newly saved source is **Pending collection/review**, visibly not yet used. The next
confirmed Redesign collects it under the registered refresh policy, preserves original
bytes/provenance and checks relevance, quality, licensing and compatibility before
incorporation into the candidate. Generate/Regenerate may include it once an eligible
reviewed snapshot exists and the accepted design's registered source/coverage rules
permit it; adding a URL must not invoke an LLM or silently change those rules. If new
coverage needs a design change, show Needs redesign. If required content cannot fit,
report Capacity blocked; optional exclusions need explicit recorded reasons. Respect
trusted bundled source provenance versus post-installation edits; a source label or
green check bypasses neither authored validation nor component Q1/human Q2.

Keep availability observations separate from source revisions and generation identity:
timestamps and transient network failures never enter stable prefix text. Checks are
read-only diagnostics and cannot rewrite approved snapshots, delete sources, mark an
unchanged generation outdated solely because of a timeout, or invalidate active tasks.
Reviewed source-content/selection changes follow dependency invalidation normally.
Preserve historical registrations/snapshots and pin new jobs to a coherent selection;
sources saved mid-build apply only to a subsequent admitted build.

Implement source inventory, registration, collection and checking as separate reusable
usages in Recipes with typed results and retained identities; independent Tool calls
remain separate steps. Define real store/adapter contracts before authoring component
rows. Record positive/negative intent cases and exact errors/retries; the UI opening
calls the admitted workflow through the facade, not a second browser/server executor.

### Required generation dialogs — operator clarification, 2026-10-26

These dialogs are the interactive projection of existing shared readiness/admission
contracts, not a WebUI-only model-call exception. Headless callers get structured
readiness/remedy and remain guarded. A held task's generation/selection must remain
compatible with its original retained catalogue/design; new global sources cannot
silently replace its pins. Qualify pre-dispatch wait/repair and already-pinned
incompatibility cases in the existing runtime owner before enabling resume.

Implement the [shared OK/Cancel dialog contract](../reborn/model-token-limits.md#generation-prompts-missing-selection-and-model-use)
through existing Prefix jobs and provider selection surfaces:

1. An explicitly selected prefix that is missing/incompatible triggers a dialog
   naming profile and provider/model pair. OK starts the supported Generate/
   Regenerate operation; Cancel starts no job/model call. Keep the original
   request unsent at the supported preparation/wait boundary, with a visible
   not-sent/cancelled result on Cancel, not base fallback or Tier-2 replay.
2. Every successful provider/model **Use** click shows "Generate the base prompt
   for <provider / model>?" with OK/Cancel. Use the committed selection and
   refreshed current revision, also for Sempai selection. Existing compatible base
   means an explicit regenerate offer, not an omitted dialog or forced rebuild.
   Trigger from the actual Use action/receipt, not render, catalogue refetch or
   an unrelated limit edit. Lost Use acknowledgements use existing original-ID
   readback, not another selection mutation or duplicate popup/job.
   This operation generates base; it does not replace a role-specific selection.
3. OK uses existing canonical target policy, accepted pair design and profile
   content. Missing design/adapter support or mandatory-content overflow produces
   an actionable prerequisite, not an endless generation loop or hidden Redesign.
4. Cancel after Use declines generation, not the committed provider selection.
   Desired/effective readiness remains visible; dependent model calls remain
   blocked until their selected prefix is compatible. Management/model-free work
   remains available, with a later Generate action.
5. Reuse generation operation IDs, progress/readback and shared refresh/cancellation
   owners. Deduplicate repeated confirmations/reconnects. Stale pair/design/policy
   is rechecked before admission; do not generate for an obsolete Use response.
   Generated, validated, published and Effective are distinct. Offer existing
   qualified Activate/readiness where required; no unreviewed automatic activation.
6. Resume only through the existing retained pre-dispatch selection/wait contract,
   binding the exact ready artifact before the first provider attempt. Preserve
   old pinned selections and original input/effect accounting. No parallel modal
   runner, resubmitted chat, fresh matching or replay of already attempted calls.
   Qualify any missing wait/selection adapter in its existing runtime owner.

## 9. Serving, large-prefix capacity and hybrid cache

Provider definitions and active configuration come exclusively from PostgreSQL
(`brassclaw_llm_providers`) through the existing `PgProviderRepo` or a neutral port,
as required by simplified-v3 §10. Resolve provider identity, protocol, model, URL,
context capacity and active selection from that DB contract; prefix profile/model
metadata describes compatibility, not a second provider registry. No embedded
provider list, recurring Rust provider seeder, file catalogue or Env override/fallback
may supply production targets. Offline pinned tokenizer/model artifacts remain
compiler inputs, not an alternative provider configuration source. DB bootstrap and
Secret-Broker resolution remain separate; secrets never enter source manifests.

Use the existing revision-aware reload contract and show desired/effective state.
Provider deactivation/removal transactionally resolves affected active selections
to an explicitly chosen alternative or Unconfigured. Future dispatch, including a
continuation, must not use an inactive/hidden provider merely because its prefix is
retained. Treat already-started calls under the provider's defined completion/cancel
contract without replay. Reactivation is explicit, validates adapter/model/auth
compatibility and never silently restores previous selections. A DB error is a
technical error; absent configuration is Unconfigured. Neither triggers fallback.
Operator management and model-independent Tier-0 tasks remain available.

Use **request-owned prefix injection for every production target**. Resolve
the role/task's immutable base/domain/reviewer composite and add its complete
required instructions/knowledge exactly once through that protocol/model's
supported request slot. Sempai, Kohai/domain and other roles may share one model
endpoint without sharing a role prefix. The provider uses its default rendering;
BrassClaw neither reads/writes its host configuration files nor installs a
role-specific server template. There is no alternate server-owned mode, dedicated
template-install route, service edit or template override setting in the product.

Pin the finished provider/model-specific prefix payload, Kohai dynamic-prompt
formatter, target joining contract, default-format/model evidence and canonical
policy through the existing task/continuation selection. The concrete current
catalogue/interceptor callers in §5.3 must be migrated and behaviorally qualified;
their present pinned system basis is not this serving contract. Resolve and consume the
same accepted provider/model design revision used to generate the selected prefix;
the Sempai review call resolves its pair's design and separately generated reviewer
prefix. A shared provider/model pair uses the same design revision for both roles.
For the final provider request, Kohai first formats only the Orchestrator's
dynamic prompt with the final target's design. A connected/enabled Sempai then
optimizes/corrects that formatted prompt and returns the final dynamic prompt.
Kohai adds the intended final-target fixed prefix unchanged exactly once before
it; there is no second formatter call after Sempai's return. Never include that prefix in the editable review payload.
For the Sempai review request, format its dynamic review packet using its own
pinned design, then add its fixed reviewer prefix unchanged. Preview
and runtime reuse that exact join, not whole-prefix reformatting. Freeze and
measure the actual combined request and effective output reservation before
cache return/submission; lossless transport/default mapping must preserve the
prefix's exact characters and expose its real measured boundaries. A separate `/tokenize` probe
does not alone authenticate the later request. Qualify supported API/format/model
bindings and default rendering without depending on provider filesystem access.
If exact tokenization or output capacity cannot be established, retain an explicit
unsupported/unknown state for that guarded target; never invent server settings.

The required assembly is **finished fixed prefix + already formatted Orchestrator
prompt**, with dynamic tools/history in that formatted prompt part. A provider
default or SDK path that would subsequently move those dynamic parts before the
prefix, reformat it, or inject it twice is incompatible until its adapter supports
the correct joining contract. Do not accept span-only accounting as permission to
change that ordering or introduce a second whole-request render. Actual complete
measurement must verify exact prefix text/token identity, both input ceilings and
total context/output fit. The large-prefix allocation stays intact; an LCP is not
its size limit, and the 2,179-token diagnostic fixture is not a 100,000-token
capacity or cache test. Changing tools within the trailing formatted prompt does
not by itself prove loss of the fixed prefix's reuse. Measure backend token/cache
behavior separately, without unverified promises or provider configuration edits.

**Historical serving observation, 2026-10-24 (superseded configuration):** that
Ornith endpoint reported a 131,072-token context and rendered a minimal
single-user request as 113,600 input tokens; an explicit short system message
rendered as 113,613. The corresponding completion usage agrees. Evidence is in
`target/accepted-evidence/k3-role-selection-20261024/run-2/operator-model-*-facts.json`.
These are complete rendered-request counts, **not** authenticated semantic prefix
lengths, a new model limit, cache-hit proof or identification of the injected
generation. Capture/qualify the actual effective template and managed-prefix
boundary before attributing tokens to either hard ceiling. These counts do not describe the current test setup after the operator-reported
26 October removal of the Sempai override. Preserve their diagnostic evidence but
requalify default-template request-owned injection and actual capacity through
supported APIs before serving. Do not layer client content over an independently
detected unresolved automatic reference, or assume downloaded assets alone prove
the deployed renderer. Unknown/unexpected injection is an incompatible target
state to report, never permission to modify the provider or trim knowledge.

Capacity and quality checks must reserve output for reasoning as well as visible
answer tokens. In the same observed minimal request, the explicit 256-token test
limit was fully consumed by reasoning (`finish_reason=length`, no visible content).
Without that artificial fixture limit, the request completed with 770 output
tokens, including 614 reasoning tokens, and nonempty content. Preserve both
outcomes and the SDK's empty-response error; do not call the capped case a Prefix
quality failure or suppress it as a production fix. Production reservations still
come from the canonical target policy and actual remaining context, not these
diagnostic numbers or an unbounded-output assumption.

Rollback uses the same freshness, compatibility and generation-enforcement checks
as activation. A retained older generation is not automatically eligible for new
tasks; rebuild it against current applicable sources when those checks fail.

Pin the generation/injection mode with each admitted model task and continuation.
Do not reread the active pointer during that task. Provider/persona/reviewer schemas
and volatile state remain distinct from the common evidence corpus. Route-specific
Kohai and Sempai prefixes/warm states are independent. A deterministic Tier-0 task
still needs no model call merely because a prefix exists.

Select the prefix specified by the retained Recipe, Orchestrator or role; if none
is specified, select the target-specific `base-prompt`. A domain/reviewer prefix
uses its own required knowledge and does not automatically receive the whole base
prefix. Shared BrassClaw knowledge is included through explicit content coverage.
Seeded/custom profiles remain independently buildable. Where a consumer explicitly
requires a composite, its records pin the exact parents, pair design, content rules
and token manifest and the active selection points to that complete artifact.
A missing/incompatible specific prefix or required composite is blocked; never
substitute base or a domain-only artifact merely to make it fit.

### Composite generation workflow

Add a reusable composite-building Recipe only for explicit consumer coverage
requirements, with typed inputs: exact declared parent
generation IDs, registered target ID and registered rendering/coverage policy ID.
Resolve immutable parent source exports/evidence through existing store primitives,
pin the DB target revision and local tokenizer/template artifacts, run the shared
compiler's composite adapter, validate, then publish a distinct immutable composite.
Use the same durable job, invocation, cancellation and artifact contracts as other
builds. Composite building is model-free and uses the accepted non-shell worker
path; activation remains a separate operation after successful publication.

The adapter renders source units from pinned parent exports under one envelope;
it never concatenates two rendered chat templates. Preserve original evidence IDs
and source/revision/range identities. Deduplicate identical cards only when their
identity and payload agree; conflicting payloads for one ID fail validation.
Record parent generations, ordered units, coverage policy, required units and any
explicitly permitted partition in the composite manifest. Preserve all required
base content and declared domain coverage; do not silently drop units to fit.
Recompute the composite template, full context/output capacity and leading token
fingerprints rather than reusing either parent's cache geometry. Capacity failure
publishes no activatable composite and leaves parents/active selections intact.

Propose `POST /api/webchat/v2/prefixes/{name}/composite-jobs` for a domain or Sempai reviewer profile,
returning `202` and the existing durable job reference. The server validates exact
parent IDs, target/policy IDs and profile relationships; it accepts no paths/code.
Use the existing job status/cancel routes and generation-detail projections, extended
to show composite parents. Keep top-level rows for seeded and registered custom profiles; show target-specific
composites beneath their domain/reviewer row with Build composite / Rebuild, job progress,
Capacity blocked and Activate state. Parent generation alone is labelled Generated,
not Ready for BrassClaw activation. Normal target-aware Generate orchestrates this
Recipe with required parent tasks through the same job service; the explicit
composite endpoint supports advanced exact-parent builds. Never implicitly rebuild
within Activate.

Parent source/selection changes invalidate dependent composites for new tasks.
Rebuild explicitly pins the newest eligible parents under one coherent selection;
if a parent is missing/outdated, report that dependency and build it first through
the normal Recipe. Already running composite jobs retain their inputs; a completed
outdated result remains historical and cannot activate. Dedupe on exact parents,
target/tokenizer/template revision and policy/compiler identity. Preserve existing
task references and independently usable domain-only generations.

Keep a stable base spine before a selected domain branch where possible. Do not
concatenate complete 100k bundles for every profile: the verified example Ornith endpoint has a
131072-token technical context limit. Register compiled composite profiles or
model-compatible selections with explicit manifests and measured capacity. Store
the entire source library regardless; if required complete units cannot fit,
report Capacity blocked and offer a different model/context or an explicitly
reviewed partition. Never silently truncate code/bindings, drop required guides,
hide a component cap or pad the prefix with low-quality repetition.

Production policy follows `token_budgets_enabled=false` by default: no hidden
retrieval/history/knowledge caps. Actual model context/output capacity remains a
technical limit. Distinguish the current compiler's example 100000-token packing
target and explicit operator settings from disabled global token budgets. Reserve
workspace based on the real serving contract and selected model; do not install a
universal hardcoded 100k/31k policy in every provider.
Count the actual complete serving envelope, including tools, role/template overhead,
current history/user input and requested output allowance, before each dispatch.
Build-time reserve is an explicit target contract, not a guarantee that every future
conversation fits. Oversized requests return a specific capacity outcome; no hidden
history truncation or discarded mandatory knowledge compensates for overflow.

Keep original tokenizer and chat-template artifacts immutable. Measure leading token
IDs across thinking, tool and system-message variants. Stable text hashes alone do
not prove shared token identity. Dynamic timestamps, job IDs, source retrieval times
and metrics stay in manifests rather than the leading reference text. Stable source
identity/content stays in the evidence rendering.

The current Ornith compiler profile distinguishes 24 linear-attention layers and
8 full-attention layers, 4-bit weight configuration, attention KV dtype and
recurrent-state dtype. Read quantization configuration rather than inferring its
method from the model name (the retained artifact reports compressed-tensors). Preserve that distinction: INT4 weights do not mean INT4 KV or recurrent
state. Parameterize provider/model adapters rather than assuming every model has
this geometry. Derive hybrid block/chunk compatibility from the actual pinned
configuration and runtime. Treat previous 1056-token-block/99264-token reuse
receipts as observations for that deployment, not constants for all targets.

### 9.1 Runtime/cache qualification and stable namespaces

Compilation creates reference/context artifacts, not GPU attention pages or
recurrent state. Inference creates those caches; eviction/restart may remove them.
Caching is context reuse, not training or guaranteed knowledge persistence. A
compiled generation and qualified serving target have different lifecycles.

Qualify actual runtime/model/tokenizer/template and cache layout: vLLM/connector/
LMCache source or build hashes (including backports), attention backend, parallelism,
weights/config and relevant scale identity, KV/state dtypes, logical/physical block
mapping, group/chunk geometry, checkpoint mode and prefill scheduling. Associate
verified fingerprints with DB target revisions. Package versions are insufficient.
Relevant runtime changes invalidate target qualification; rendering changes
invalidate effective-token evidence. Renewing an observation does not require
rebuilding identical reference content.
Separate runtime compatibility identity from process lifetime: compatible restarts
retain the declared namespace, while invalidating process-bound warm observations.
PID/random engine IDs must not create a new namespace on every restart. Claim
restart/disk reuse only after a real supported restore passes content checks.

For enabled hybrid external caching, verify independent recurrent/full-attention
groups and compatible transfer code, not just chunk divisibility. Derive flags from
supported interfaces/effective metadata; absence of an explicit hybrid-manager flag
does not prove the manager is disabled. Sparse align-mode checkpoints exist at
scheduler boundaries, not every attention block. Larger prefill batches require a
separate measured experiment, never automatic adjustment by Generate.

For GDN/linear-attention hybrids, qualify behavior as well as cache-hit counts.
The inspected Ornith release has 24 linear-attention and eight full-attention
layers; the historically inspected deployment used a 1,056-token unified block.
Architecture and deployment block layout are separate evidence, not universal
defaults. Refresh runtime layout before applying those historical measurements. `--no-disable-hybrid-kv-cache-manager` keeps the
manager enabled but cannot prove recurrent-state restoration. Record effective
Mamba cache mode, attention/recurrent dtypes, page layout, group separation and
checkpoint granularity separately from INT4 weight quantization. Do not infer
an INT4 KV cache from AWQ weights. GDN cached/fresh generation is not necessarily
bit-identical; compare behavioral scores with fixed requests and decoding rather
than treating any token difference as corruption. Preserve cold, native, external
and restored-native outcomes independently. A sentinel restore is not a semantic
accuracy comparison. See the [LMCache hybrid guide](https://docs.lmcache.ai/mp/hybrid_models.html).

For an inspected block size N, an align-mode batch in [N, 2N) offers finer boundary
checkpoints; larger batches trade checkpoint density for prefill throughput when
supported group separation is active. Benchmark those modes under a fixed design
before selecting a model's accepted settings. Never apply another model's N,
force unsupported all-mode or enable speculative/sub-block features merely from
generic documentation. Preserve operator-owned deployment settings during prefix
generation; a configuration experiment needs separate retained runtime evidence.

For this hybrid, the linear layers retain recurrent summaries rather than an
independent attention key/value pair for every preceding token. Full-attention
layers still retain their KV history; cache capacity alone does not establish
reliable retrieval of every rule. Trial semantically grouped references, explicit
source/contract precedence and a compact terminal audit near the volatile task.
Keep these structures stable within an accepted design and preserve all selected
original evidence. Measure generalization and citation support, not only the
frozen score. Do not insert arbitrary padding or claim that placing a paragraph
on a 1,056-token boundary improves its meaning: physical checkpoint alignment
and semantic source organization are separate properties to qualify.

`cache_salt` only namespaces entries. Use a stable namespace under the target's
cache-isolation/runtime-layout contract; never create a salt per request or change
it between prime/planner/writer/repair calls within an experiment. Introduce no
tenant/feature-role registry for the single-instance operator. Keep incompatible
cache layouts separate; reconcile old objects through supported provider management.

Enforce generation selection independently through immutable injection and exact
tokens. Where safe shared-base reuse is intended, a compatible namespace may span
domain generations: token hashes distinguish divergent inputs. Salting every full
composite isolates experiments but prevents cross-domain reuse of the common base
spine. Record the tradeoff; never discard isolation to inflate hits. Salts/runtime
epochs/statistics do not belong inside model-visible reference text.

For observable local cache adapters, compare identical requests serially across
cold compute, native reuse, native eviction with verified external restore, then
native reuse of restored state. Keep prefix/template/messages/decoding fixed; retain
raw responses/errors, expected-content checks and request-local native, external
and **extra external** counts. LMCache total hits overlap native hits; extra hits
establish additional restoration. HTTP 200/native hits do not prove correct state.
Include unconstrained and constrained decoding and representative factual/behavioral
probes; a fixed JSON grammar alone can hide nonsensical logits.

The measured Ornith short warm probe ended within the final shared block, establishing
a static checkpoint before long histories. Apply this only where the resolved
checkpoint contract supports it; verify actual tokens and expected content. Priming
cannot certify corrupted native state solely by hits. Qualify while the shared
target is idle or through supported isolation; do not evict unrelated live requests
or claim load safety from a serial probe.

Measured 2026-10-07: LMCache 0.5.5/vLLM 0.31.0 omitted hybrid object separation and
its attention adapter matched only rank-five tensors. Ornith's rank-four buffers
used 32-token physical pages for 1,056-token logical blocks. Separation alone still
corrupted outputs. The relevant class from upstream
[PR 4253](https://github.com/LMCache/LMCache/pull/4253), alongside separation, passed
plain/JSON sentinels with three verified 97,152-token external restores. See the
[verification receipt](../../scripts/prefix/sempai-accuracy-20261007/cache-fix-verification.json)
and [source/layout review](../../scripts/prefix/sempai-accuracy-20261007/lmcache-4253/source-and-layout-review.json).
This qualifies that runtime; it proves neither arbitrary load, disk/restart
persistence nor model accuracy.

These layout/correctness checks precede production activation when the configured
path can serve external hits. Known-corrupt paths are ineligible, not Unverified;
fresh salts are not remediation. Requalify a repaired path or explicitly select an
independently accepted alternative cache mode. External caching remains optional:
providers not using/exposing it are not blocked by absent LMCache telemetry.
Deployments remain operator-owned; Generate never patches packages, changes Compose,
reloads systemd or restarts vLLM.
Qualification is a separate observable operation against an immutable candidate
using an accepted injection/isolation adapter; it must not require activating that
candidate first. A completed build stays Generated if qualification later fails,
with the target explicitly ineligible. Generate has no hidden model calls. Existing
runtime qualification may be reused only within its recorded compatibility scope;
candidate-specific token/injection evidence still names the exact generation.

Warm through the provider that will actually serve the selected prefix, using its
real template/tool envelope and a minimal normal completion when supported. Provider
adapters declare warm-request support, token inspection, native reuse telemetry and
external reload verification separately. Correct injection, capacity, freshness and
generation enforcement gate activation, alongside section 9.1's correctness gate
for enabled external cache paths. Missing telemetry alone does not block a valid
provider. Verify a second
request's reuse only when observable; otherwise report Cache reuse unverified and
disable unsupported verification actions without blocking valid inference. Token
inspection is also capability-specific: request-owned injection verifies exact
outbound content/manifest and uses qualified default-format adapter/tokenizer
capacity evidence. No server-owned role template or provider-file installation
is offered as an alternative. Do not
claim exact server token/cache observations when the provider does not expose them.
Record full-attention KV and recurrent-state support honestly where applicable.
LMCache external persistence is a separate optional adapter capability. Test reload
after native-cache eviction when claiming external reuse; a warm timestamp, HTTP
200 or native hit does not prove external recurrent-state storage. Do not replace
Compose, venvs or systemd units as part of this upgrade's default flow.

## 10. Citation and configuration validation in production

Reuse `prefix_response_validation.py` and its exact source identities, indexed
quotation copying, quotation validation and bounded repair behavior. Base-prompt
evidence additionally resolves document release/range and component UUID/revision/
field. Add adapters without breaking the existing domain generation format.

Carry the selected evidence manifest into actual Kohai/Sempai/gateway answer paths
and verify it matches the injected generation. Do not merely ship the helper script
and claim all answers are checked. Integrate with the existing message/tool/reviewer
contracts: the standalone `claims/configuration/missing_information` schema cannot
replace every tool-call or Sempai response schema indiscriminately.

Track provenance success separately from semantic support and runtime truth. Exact
quotes and basic word overlap can still support an incorrect interpretation or
reject a legitimate translation. Show that limitation and require appropriate
review. Production evaluation includes wrong-but-verbatim citations and misleading
source/version combinations, not just invented IDs.

Allow a model-specific design to select short readable citation handles if it
performs better than opaque hashes. Implement this through a registered rendering/
resolver contract and accepted response schema, not invented response fields.
Each immutable generation must map every unique handle to its exact source card,
revision/range/checksum; ambiguous, missing or wrong-generation handles fail closed.
The readable name is navigation, never source identity or semantic proof. Keep full
integrity fingerprints and original evidence. Compare valid-but-unrelated handles
and quoted passages as well as invented ones before accepting that design.

Only Home Assistant currently has static configuration adapters. Defensive server/
firewall configs, SQL and tomedo API payloads require their own versioned parsers,
schemas or non-mutating application checks. Never treat the YAML check as validation
of all other domains or run generated operational configurations against live
systems merely to validate them. Keep syntax, static
schema, semantic review and observed runtime/device checks separate.

Sempai additionally needs lossless proposal/whole-artifact validation through its
actual class-specific sink. Retain the draft before review; check supported fields,
immutable input identity, authorized prompt-edit boundaries, capabilities and the
entire component/association/workflow. Isolated bounded Linux pure-logic execution
checks are appropriate; unapproved candidates never dispatch Tools or mutate
devices. CPython success is not Monty compatibility or production Q1/Q2. Validate
summaries against payloads; prerequisites/warnings must survive in the actual
artifact, not exist only in a prose summary.

An optional contract-planning model call is untrusted intermediate data, not an
executable plan or approval. Each independent call/validation usage has its own
Recipe step. Version output schemas/grammars and pin their hashes and supported
parser behavior. The current optional planner schema contains `artifact_kind`,
`requirements` (aspect plus exact contiguous `contract_quote`) and `workflow`
(ordered knowledge/UUID references supplied by the original instruction). Mechanical
validation rejects empty/non-source quotes, non-Recipe workflows, unsupplied UUIDs
and extra executable branch fields. This does not establish classification, complete
coverage, correct order/bindings or behavior; the writer still reads the entire
original contract. Keep this planning schema distinct from persisted Recipe JSON.
Emit requested artifacts before `composition_summary`, and derive the summary from
the actual payload. Fixed-order grammars must fingerprint ordered serialization;
canonical key sorting or dictionary equality cannot establish decoder compatibility.
Protected-message/single-proposal/single-variant constraints apply
only when the trusted authoring contract requires them; offline benchmark settings
must not restrict general v3 Recipes. Unsupported fields/APIs are failures.

Bounded repairs consume only the retained failed draft and real validator evidence.
Record positive attempt limits and parent/response/feedback hashes with an unchanged
task contract. Never weaken checks to reach a score, replay effects, merge repairs
into first-pass accuracy or supply benchmark answers to first-pass requests. Keep
syntax, semantic behavior, citation support and serving failures separate. Quality
observations identify suite/revision, client protocol, decoding controls, runtime
fingerprint and first-pass/repaired results; absent measurements say Unmeasured.

## 11. Phased implementation and acceptance gates

### Phase A — Contracts and source inventory

Current status: baseline profile/source/compiler contracts are implemented for the
accepted source-only slices. Remaining steps 1–7 complete coverage, source jobs,
model policy/geometry, qualified design and hard-limit contracts; this phase is
not wholly unimplemented and its complete acceptance is not yet closed.

Freeze profile IDs and source classifications; inventory current source packs,
existing generation receipts and actual provider adapters. Specify typed input/
result schemas, Recipe variants, one-UUID steps, Skill associations and failure
contracts. Resolve the trusted Tier-1 compiler admission and typed-input/state/
version prerequisites rather than copying obsolete Plan-2 seed examples. Add at
least ten positive intent examples per authored Recipe plus negative routing cases.

Acceptance: reviewed end-to-end workflow and exact missing primitives; no invented
UUIDs, unsupported store fields or false Tier-0/Q2/runtime claims.
Include the saved build specification, six seeded profile coverage matrices and custom-profile coverage contracts, artifact/
identity schemas, worker dependency/resource contract and a target capability matrix
for injection, token inspection, grammar, native/external cache and validation support.
Inventory per-profile compiler capabilities and differences, including supported
architectures, source adapters, projection/policy behavior, protocol schemas/order
and actual entry-point defaults. Bind that capability revision to accepted designs;
unknown options fail validation rather than disappearing during compilation.
Specify section 5.3's model/specification/runtime/design identities, immutable schemas,
profile/consumer selection rules, provider-switch impact and the confirmed design-job
contracts before implementing database or UI fields. Unknown capabilities stay explicit.
Specify section 8.1's used-versus-registered source inventory, registration/collection
contracts, per-type bounded checks, availability states and source-review eligibility.

### Gate A1 — Runtime prerequisites for production Recipe jobs

The closed source-only compiler has real bootstrap/global-admission evidence;
reuse it for that exact graph. The general production successor must still satisfy
the applicable gates below. Do not mark every prerequisite missing merely because
normal Generate is incomplete, or infer general approval from the closed slice.

Track the [shared ownership table](#shared-infrastructure-ownership--reconciled-2026-10-22)
as explicit dependencies with exact implementation references and applicable
passing production-caller receipts. This gate consumes those owners; it does not
commission their mechanisms again:

- [simplified Phase 0a](../../simplified_v3.md#phase-0a--ground-truth-komponentenverträge-implementieren) supplies coherent selected contracts, typed layouts,
  association approval consumption and actual Tool artifact/ABI retention. Review
  evidence comes from its validator owner; canonical usage migration comes from
  composition. The prefix graph must retain the exact supported closure.
- [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben) supplies the one global service, Ready handshake, bounded
  task/child admission, cancellation, live resources and supported continuation/
  invocation recovery. Its owner implements missing shared gaps; a prefix job
  must not bypass them with another VM, queue, shell runner or Rust loop.
- [simplified Phase 1](../../simplified_v3.md#phase-1--einzige-instanz--und-betreiberidentität), [simplified Phase 2](../../simplified_v3.md#phase-2--autorisierungsscopes-entfernen-daten-ids-entkoppeln) and [simplified Phase 3](../../simplified_v3.md#phase-3--eine-feste-runtime--und-sicherheitsrichtlinie) supply operator identity,
  instance-wide Tool policy and technical enforcement. Prefix exercises those
  checks through its actual compiler/source/publication calls, including live block.
- [simplified §10.1](../../simplified_v3.md#101-umsetzung-und-entfernung) supplies the DB-only provider lifecycle before model-facing
  production actions. Missing provider configuration does not block an otherwise
  qualified provider-free source build or instance management.
- **Prefix-owned graph acceptance:** use an accepted non-shell, zero-model-call
  compiler adapter for Generate/composites; qualify source/coverage/artifact/output
  checks and the exact publication receipt through the global Recipe caller. The
  first build cannot require its own already-generated prefix. Shell/assisted
  variants additionally need supported Tier-1 admission; they do not block the
  qualified deterministic baseline.

No prerequisite is passed by a Markdown contract, metadata path or helper-only
receipt. Scope evidence to the selected graph and supported lifecycle: missing
shared safety/recovery support blocks affected production exposure, not unrelated
safe development. Do not repeat the global Monty version migration or reconstruct
completed loader/approval infrastructure to qualify another prefix profile.

Phase B's offline compiler/source-package work and Phase C's storage/migration
development may proceed while these dependencies are outstanding. Production
Recipe admission, startup-triggered generation and enabled UI Generate actions may
not. Do not substitute a private shell runner or the old per-turn VM to bypass
Gate A1. Label intermediate artifacts and surfaces as development/unavailable.

### Alignment with the simplified-v3 implementation sequence

| Prefix work | Required simplified-v3 contracts/acceptance |
| --- | --- |
| Phase A / Gate A1 | Phase 0 inventory, Phase 0a component/binding/artifact/retry contracts, applicable Phase 1–3 operator/policy paths, and Phase 3a upgrade/global lifecycle. |
| Phase B | Offline compiler work may proceed independently; installed seeding/upgrade follows shared integrity boot and Phase 6 migration contracts. |
| Phase C | Consume shared Phase 0a/3a execution/selection/recovery; prefix implements only its product records and additive migrations within Phase 6. |
| Phase C1 | Sempai reviewer reference/packet/proposal contracts, Phase 0a authoring validation and human Q2, with consumer wiring accepted in C2. |
| Phase C2 | §10 DB-only provider lifecycle, live global policy and applicable Phase 7 production-caller acceptance. |
| Phase C3 | Accepted Gate A1/C/C2 paths plus DB model specifications/designs/bindings, immutable proposal/current-content generation contracts and Prefix-Maker bootstrap; authored executable changes follow Phase 0a Q1/human Q2 and registration. |
| Phase D | Phase 5 instance operator UI/API; reachable actions depend on accepted Gate A1/C/C2 paths, with C3 additionally required for design operations. Prefix knowledge profiles do not introduce security RuntimeProfiles or product editions. |
| Phase E | Phase 7 answer/cache acceptance and §5.1 performance evidence; quality and cache reuse remain separate measurements. |
| Production cutover | Phase 8 coordinated migration and component-cutover gate after the applicable prerequisite matrices pass. |

Reuse accepted upstream v3 work and receipts; do not reimplement its lifecycle,
provider catalogue or policy engine within the prefix feature. Independent offline
development need not wait for every unrelated v3 cleanup, but production paths must
pass all contracts applicable to their consumers.

### Phase B — Base compiler and release source package

Current status: offline adapters, release/core packages, installed capture, shared
source-only stages and native scoped qualification are implemented. Remaining
steps 2–4/19 close full-library/domain parity and installation coverage; target
fit/design/serving are separate C/C2 gates. The dated entries retain earlier bounds.

Continue from the offline `base-promt-compiler.py` and shared `prefix_evidence.py`
adapter below; do not create another copied compiler. Complete installed catalogue
and release-document export, shared-engine parity and production worker qualification.
Port §4.2's v25 evidence-view and scoped-audit facilities deliberately; qualify each
profile adapter and keep Sempai consumer schemas separate. Implement the installed
source-package path, environment-independent worker inputs, model adapter capability
checks and raw-package publication rather than wrapping the shell example unchanged.
Build reproducible release source packages and seed their registration through
shared boot. Preserve original documents/code and complete structured records.
Package the dedicated Sempai compiler/procedure and full authoring/validation source
inventory in the same release process. Its standalone allowlisted checkout adapter
must become an installed immutable source-package adapter; installation cannot
depend on the developer checkout. Measure the complete selected Sempai generation or explicitly required composite
against the selected model before promising readiness. If mandatory originals exceed
capacity, report the exact requirement and keep inputs intact; no automatic guide
truncation or implicit lower coverage is accepted.

Acceptance: every mandatory guide is complete; Recipe/code/schema/association data
survives export; no HA topic leakage; missing reference/coverage fails the build;
two offline builds of the same input produce identical source/evidence/template/
leading-token fingerprints; raw originals survive failed builds. Fresh install,
upgrade and reseed preserve integrity, operator edits and retained old versions.
Also require host/path-independent content identity (including removal of current
path-dependent model/source fields from canonical identity), reproducible whole-unit
selection and exact byte/hash agreement between source and original evidence.
For reviewed strict-JSON displays, require decoded-value and embedded-program
identity plus separately verified original/display/rendered hashes; do not demand
raw/display byte equality or normalize ambiguous JSON. YAML, Python and prose remain
literal. Require reviewed conflicting/versioned sources; tutorial mutation checks
and transitive validator fingerprints. Prove zero provider/network/process discovery
through the actual worker entry point. No optional synthesis mode is invoked.

### Phase B implementation slice — offline base compiler (2026-10-11)

Implemented an explicit reviewed-inventory snapshot adapter and immutable-package
base compilation in `scripts/prefix/base-promt-compiler.py`. It preserves original
bytes, full documents/code, source inclusion/exclusion reasons and complete section
ranges/hashes. Publication is staged, content-addressed and serialized per output;
no active pointer is changed. Identical packages compile independently of worker
paths. Strict duplicate-key parsing, checksum/root checks and mandatory-guide
coverage reject incomplete/tampered inputs before publication.

`prefix_evidence.py` now owns shared literal fences and real template token probes
for all four existing domain compilers; HA/Sempai also share section segmentation,
and HA/base share the existing Ornith/Qwen server injection adapter. tomedo PDF
segmentation and Defensive's existing suffix behavior stay in their adapters.
All affected generation identities include the shared module hash. Remaining
profile-specific quality/projection features have not been unified or requalified.

Capacity optimization renders each full source document once instead of wrapping
and duplicating provenance around each section. The full section/evidence index
and all original bytes remain in the artifact. With the actual cached Ornith
snapshot `489cb97981b8654bcfcf30ce1f94ed1b62e07b53`, the regression's seven complete
mandatory guides plus one actual component render to 119212 tokens, down from
140519. An explicit 8192-token workspace reserve gives 127404 of 131072 tokens.
This is the base adapter's documented minimum/default; larger workload reserves
are selectable and capacity failure never drops source material. Domain compilers'
existing reserve defaults are unchanged. Adding the full installed catalogue and
remaining release docs may exceed this budget and must be measured separately.
No claim of full-library fit, deployed model quality or KV/recurrent cache
qualification follows from this local tokenizer result.

The retained local development build at
`/private/tmp/brassclaw-base-prefix-sfn9bqdd/receipt.json` uses the real sample
component's repository path and shorter inventory reasons: 119195 rendered tokens,
118933 stable shared tokens across 18 distinct rendered probes, plus the 8192 reserve
(127387 total; 3685 remaining). Its generation is
`579e1125967d0686f87b9ca8971b6a35526eba8b4d6cf45970396b94aaa14d33`.
The receipt pins tokenizer/template and engine hashes; it remains a development
artifact with `activation_eligible=false`, not an installed runtime acceptance.

Acceptance: four offline base tests (including actual CLI/tokenizer, reproducible
builds and failed-capacity preservation); seven HA, ten tomedo and two Sempai
source tests; one cross-profile citation test. Defensive's two pure evidence tests
pass; its seven complete-model build tests were not run because the cached snapshot
lacks the required model config. This slice changes no Rust/runtime seed/DB/provider
or remote deployment. See [copy-paste usage](../../scripts/prefix/BASE-PROMPT-USAGE.txt).

Still open: complete current release inventory; full-library owner export beyond
the scoped ordinary-workflow projection accepted below; installation-owned source/compiler qualification; bounded registered
worker; runtime generation/composite Recipes; production named storage/jobs,
consumer resolution and UI. Development packages never establish approval or
activation (`activation_eligible=false`). The local manifest is an artifact contract,
not a new installed source/approval catalogue, queue or provider registry.

### Phase B follow-up — owner-selected catalogue source export (2026-10-11)

The new operator read projection is
`GET /api/webchat/v2/catalogue/{generation}/sources`, reached through the existing
`RecipeStore` / `RebornServicesApi` facade and installed catalogue owner. It shares
capture's immutable revision/usage approval checks; it holds the owner's activation
serialization and one read-only repeatable-read transaction. A requested stale
memory generation fails with 409; catalogue bytes/checksum, DB active pointer,
exact source revisions and retained usage approvals must agree. Exporting performs
no Tool dispatch, Q1/Q2 write, candidate activation or generation publication.

`brassclaw-catalogue-source-export/1` preserves exact catalogue bytes, component
revision envelopes/checksums and each complete retained selection, including
variant/input/export/approval pins. Components are deduplicated by exact revision;
conflicting selections fail rather than silently prefer one. The global root's
pinned source is included separately as retained evidence, without claiming that
retention qualifies an authored root or establishes usage approval. Scope is
explicitly `active-ordinary-workflows-and-retained-root`; legacy validated rows
outside that owner, separately managed review workflows and staged proposals are
not exported as approved active components. Full-library migration/qualification
and a complete release inventory remain prerequisites for the final base prefix.

The offline snapshot adapter accepts `--catalogue-export` alongside the reviewed
release inventory. It checks transfer/source hashes, identity, dependency and
selection coverage and preserves all exact originals. It renders component and
selection sources once; the full transfer envelope is retained as raw provenance,
avoiding duplicate component text in the model prefix. Imported JSON is not an
approval writer and all resulting development generations retain
`activation_eligible=false`. Capacity is checked against actual token rendering;
adding this graph never authorizes omitting any mandatory source to make it fit.
The 64 MiB export capacity is a supported technical transport bound, not a token
budget or row limit; exceeding it rejects the complete export.

Focused acceptance (2026-10-11): the exact frozen
`complete-cancellation-20261011-r2` binary passes the actual authenticated operator
component-deployment/source-export regression against native PostgreSQL and the
global worker (6.83 seconds). Its actual exported input then passes all five
offline compiler tests, including path-independent identity, exact original-byte
retention, transfer corruption rejection, immutable publication and the cached
Ornith tokenizer (10.95 seconds; no skipped cases). The seven mandatory documents
plus this owner-selected graph render to 119,212 tokens with an 8,192-token reserve.
This is below the selected 131,072-token capacity; the same sources reject a
32,000-token target without publishing or dropping sources. The live model's
`/v1/models` also reports 131,072 on 2026-10-11. The preserved receipt/export/
tokenizer hashes are `/private/tmp/brassclaw-prefix-export-accepted.json` and
`target/validation/prefix-export-artifacts-20261011-r2/accepted.json`.

This is focused native export and offline quarantined-compiler acceptance, not
production prefix activation or complete library coverage. The containing
snapshot's separate expanded cancellation test failed its fixture setup and is
being repaired; it is not counted as passing source-export evidence. The preceding
strict composition lint covers the same unchanged source/export implementation;
subsequent whole-matrix and production prefix-job acceptance remain required.
[Copy-paste instructions](../../scripts/prefix/BASE-PROMPT-USAGE.txt) describe the
existing operator token, pinned-generation export and subsequent offline build.
Runtime Recipe/worker/storage/UI implementation remains in the following phases.

### Phase C — Named generations, jobs and Recipe execution

Current status: registry/generation/model/source/job migrations and stores, imports,
qualified source-only Recipe execution, durable measurements and reconnect support
exist. Remaining steps 3–10/19 extend them to every profile and the normal
target-aware/composite workflow. Add forward schema changes only for demonstrated
missing selections/lineage/contracts; do not recreate stores or edit applied migrations.
Version the existing assembly path using reusable snapshot/compiler/validation/
publication usages. Preserve durable admission, no-fallback, completion fencing and
deduplication; finish cancellation/recovery acceptance and dependency invalidation.
Implement the model-free bootstrap path and composite Recipe/adapter/job contract
above, including parent exports, evidence identity preservation and recomputed token
geometry. Inventory existing build primitives before adding the missing worker adapter.
Production dispatch requires Gate A1; offline store checks alone do not pass it.

Acceptance: real PostgreSQL tests for six seeded profiles and registered custom profiles, coherent snapshots
during concurrent activation, full catalogue export above 1000 rows, invalid-source
rejection, job restart/resume/cancel, double-click deduplication, exact request
completion, no replay after successful publish, last-good preservation and rollback.
Failed Recipe/DB operations never become Tier-2 model work.
Include crash injection before dispatch-intent commit, after compiler/publication
effects but before result recording, and after completion before reply. Assert
stable invocation counts/keys/arguments, safe unknown-result reconciliation, no
effect after failed persistence, no confirmed-effect replay, and retained exact Tool
artifacts across child/wait/retry. Missing artifacts or incompatible ABI fail closed.
Test first installation with no prefix, no provider and pinned local artifacts;
regeneration after component invalidation; missing artifacts; and zero LLM calls
through the actual bootstrap caller. Test composite build/rebuild/cancel/restart,
parent races, evidence-ID conflicts, exact parent manifests, deterministic rendering,
coverage/capacity rejection and retained independently usable parent generations.
Test the normal one-click operation with missing/outdated parents and reused parents,
parent/target changes mid-operation, partial child failure and cancel/reconnect.
Reconcile published children without replay; an outdated completed artifact remains
historical. A source-generation-only operation without a provider completes honestly
without an activation-ready badge. A target-aware operation cannot report success
until its required composite validation completes.

### Phase C foundation — named profile registry (2026-10-12)

`V139__prefix_profile_registry.sql` introduces the instance-wide
`reborn_prefix_profiles` metadata registry and six shipped profile registrations.
Names, labels, descriptions, positive metadata revisions and compiler entry names
are separate from immutable model designs, generation artifacts and consumer
selections. Compiler entry names are descriptive registration data; no filename,
row or revision authorizes executing a compiler. At this 2026-10-12 freeze,
Prefix-Maker lacked a registered compiler filename; the later adapter/V147 entry
below supersedes that gap. Installation inserts preserve
existing metadata rather than overwriting operator edits.

The existing `InterceptorConfigService` / facade / operator prefix-list API and
Prefix settings tab now read this registry. Every seeded profile appears before
its first build, with a description and explicit readiness. Existing scope-bound
base bundle fields are compatibility projections only. Base generation still uses
the existing Recipe route and is labelled `legacy-unverified`; a prior bundle or
successful prewarm timestamp cannot produce a verified readiness badge. Other
profiles are `prerequisites-required` and Generate is disabled. A direct request
for a registered unsupported profile fails with 503 before a conversation, Tool
or provider call; an unknown name returns 404. Missing registry/store support fails
closed rather than inventing a frontend list.

This foundation does not implement named immutable generations, model-specific
accepted designs, durable generation jobs, sources controls, Redesign, custom
profile authoring, composite builds, production compiler qualification or new
provider selections. Keep those in the owning Phase C/C2/C3/D steps; do not treat
metadata registration as bootstrap approval or duplicate their stores in this
registry. The existing legacy base store and running selections remain intact.

Focused acceptance passed on 2026-10-12 using the frozen source and retained
worker/test pair in `target/validation/prefix-registry-artifacts-vg1dq56v/accepted.json`:
strict product/composition lint; the authenticated operator/native PostgreSQL
registry/export/deployment regression (7.83 seconds); all five actual-export/offline
compiler checks without skips (10.53 seconds); and three UI behavior checks.
The native regression verifies six seeded registrations, descriptions/revisions,
metadata edit preservation across migration reruns, 401 for missing authentication,
503 for registered unsupported generation and 404 for an unknown profile. It also
retains the existing coherent deployment/ordinary global-Monty acceptance.
UI checks verify disabled unsupported actions, honest legacy readiness and
fail-closed older API responses. Compilation against the actual cached tokenizer
still measures 119,212 rendered tokens plus an 8,192-token reserve; this establishes
local capacity for that tested package, not complete-library fit or cache readiness.

Historical validation used the then operator-requested
`/Users/ollama/brassclaw-target2` with a separate lock. This records the old
environment only; it does not authorize another current target or build tree.
The frozen source/receipt identifies the exact tested bytes; subsequent relevant
edits need their own applicable evidence under the current development policy.
This historical slice changed no inference deployment and does not enable production named-prefix jobs.

### Phase C storage — immutable compiler artifacts and source registry (2026-10-12)

`V142__immutable_prefix_generations.sql` adds content-addressed immutable generation
storage. It preserves legacy scope bundles as `legacy-unverified` records without
changing serving pointers or inventing source/tokenizer/cache evidence. Compiler
packages carry all original artifacts, source/evidence indexes, context, template
when measured and the exact compiler manifest. The base adapter exports
`brassclaw-prefix-generation-package/1` with canonical base64 artifacts; file export
uses serialized atomic publication. Import validates strict JSON, complete artifact
inventory, per-artifact hashes, the original compiler generation identity, mandatory
source coverage, byte-identical original/evidence text and declared token capacity.
Imported packages remain `quarantined`; integrity is not qualification or activation.
The current import adapter accepts the supported base compiler contract only.

The existing prefix facade now exposes operator-only generation list/detail/import:
`GET/POST /api/webchat/v2/prefixes/{name}/generations` and
`GET /api/webchat/v2/prefixes/{name}/generations/{generation}`. Exact lost-response
retry reads the immutable prior receipt despite later profile metadata changes;
a conflicting predecessor cannot overwrite or reinterpret the retained generation.
The database rejects update/delete/truncate. Generation history/inspection and base
package import use the existing Prefix tab. Client retries retain exact request
bytes/revision and suppress duplicate dispatch; import never changes active state.

Focused native acceptance is retained under
`target/validation/prefix-registry-artifacts-jkw9aoyc/accepted.json`: actual Python
compiler/exporter -> authenticated API -> PostgreSQL -> immutable reread; tampered
packages and conflicting revisions rejected; lost-response retry retained; legacy
store untouched; update/delete prevented. Route contracts and five offline compiler
checks also pass. Two generation-history UI checks cover exact retry/deduplication
and rejection of oversized files before reading/posting. This is storage acceptance,
not a production generation Recipe, source qualification or activation claim.

`V143__prefix_source_registrations.sql` adds immutable reference registrations and
separate append-only observation storage. The existing facade exposes
`GET/POST /api/webchat/v2/prefixes/{name}/sources`, with optional exact generation
selection for its archived source inventory. HTTP(S) references preserve meaningful
version/query/fragment identifiers; embedded credentials, recognized credential
query fields and unsupported schemes are rejected. Exact component references bind
UUID/version/checksum and verify immutable source bytes. Saving makes no fetch or
model call, grants nothing and leaves collection/review pending. Duplicate identical
registrations return the original; conflicting duplicate metadata fails.

Sources opens a profile-specific panel beside Generate, with generation selection,
separate used/registered inventories, archived originals, registration form and
text-labelled availability. No availability is manufactured: until the check Recipe
and trusted observation writer are qualified, checks remain explicitly unavailable.
Registration/API/native behavior passed in the frozen receipt
`target/validation/prefix-registry-artifacts-if58k53w/accepted.json`; that capture
records its lint limitation explicitly. Current strict lint, native/model/storage
and contained-Monty checks passed in
`target/validation/prefix-registry-artifacts-87h02exi/accepted.json` and its subsequent
storage-primitive slice `prefix-registry-artifacts-zr4r4mre/accepted.json`.
Do not count the source-check,
collection/review, invalidation or Redesign workflows complete from these records.
Generation jobs, target-aware compiler qualification, per-model designs, consumer
resolution and activation remain the subsequent required work.

### Phase C model/design foundation and shared rendering (2026-10-13)

`V145__prefix_model_design_records.sql` retains immutable model-configuration
snapshots against the existing provider namespace/ID/revision history, declarative
design proposals and separate future acceptance records. It does not seed a second
provider catalogue or accept a design. The facade projects active Kohai/Sempai targets
from its existing LLM configuration owner. An overridden model never inherits its
provider's default-model context as evidence. Capture checks the exact selected
provider revision and preserves architecture/weights/tokenizer/template/cache fields
as Unknown until real artifact/effective-runtime evidence exists.

Operator routes `GET /api/webchat/v2/prefix-model-designs`,
`POST /api/webchat/v2/prefix-model-specifications` and
`POST /api/webchat/v2/prefixes/{name}/design-proposals` use the existing authenticated
facade. Proposals accept only the implemented declarative adapter/view/coverage/
reserve options; no executable code, arbitrary compiler paths or success flags.
They remain unaccepted. Model Designs in Prefix settings lists active targets,
retained settings and proposal history; selection alone writes nothing. Retain
model settings is separate from provider-assisted Generate model design, which
remains disabled until its actual Recipe/Prefix-Maker/artifact prerequisites pass.

`prefix-maker-compiler.py` now reuses the base compiler's exact evidence/token
machinery and requires all seven BrassClaw guides plus
`scripts/prefix/prefix-maker-reference.md`. Its source-backed authoring reference
separates model/deployment identities, INT4 weights/attention/recurrent dtypes,
preload/host/invocation/Recipe contexts, decoder property order, complete evidence,
stable token construction, capacity, proposal/acceptance/activation and honest
cache qualification. It contains no benchmark answer key or deployment operation.
Current knowledge does not certify an unknown model architecture. Complete raw
guides and reference stay retained; overflow fails without deleting coverage.
`PREFIX-MAKER-USAGE.txt` provides exact Linux/macOS snapshot/compile/export commands.
Import accepts this profile's complete integrity-checked development package as
quarantined. `V147` fills the previously missing compiler metadata only for the
unmodified initial registry revision; it grants no execution authority.

`prefix_render.py` contains effect-free literal rendering shared by the offline
compiler and the packaged `prefix_reference_components` PythonCode draft. The
compiler manifest pins that renderer, shared evidence engine and profile adapter.
Rendering uses typed, independently verified ordered source data, no imports or
host effects, and invocation-local arrays. Its draft is not an activated component.
Actual contained-Monty rendering checks and native model/storage acceptance passed
in those frozen receipts. Eight focused WebUI checks cover model selection/capture,
Sources open/selection/save and generation-import retry/limits. Do not infer a
production generation workflow from utility evaluation or storage acceptance.
The closed installation bootstrap owner still needs to qualify actual
storage/tokenizer usages and the complete Recipe graph independently.

Implementation direction remains Recipe-first: retain snapshot read, pure rendering,
tokenization, immutable publication and optional model evaluation as distinct reusable
usages. Rust may supply fixed immutable-artifact/store and tokenizer primitives.
If retaining an external Python runtime, pin its actual interpreter/dependency closure;
a mutable host Python pathname plus script checksums is not a retained implementation.
Do not widen the reply-only bootstrap owner to accept generic seed labels: it verifies
exact reply definitions, adapter and behavioral cases. Qualify the prefix usages through
a closed subject verifier and real independent behavior, then integrate its verified
approval into both catalogue selection and readback. Extract shared internal admission
from existing turn admission/waiting, retain durable correlation/scope, and link prefix
job metadata to those runs. No background wrapper around the blocking sender, new queue,
new workflow loop or special unapproved bootstrap executor closes Gate A1.

`literal-reference/v2-deduplicated` is an explicit supported development design
option. It merges only byte-identical complete units, retains ordered original
source/range reconstruction and renders unique neighboring qualifications intact.
It never summarizes or drops required information. `render_map.json` is independently
checked against actual original bytes/ranges at immutable package import, including
CRLF and Unicode line separators. Shared-unit IDs are rendering identities, not
document citation IDs. Default `literal-reference/v1` remains unchanged; adopting
v2 requires explicit design selection/acceptance rather than a hidden Generate change.
The frozen Prefix-Maker geometry comparison is retained at
`target/validation/prefix-maker-geometry-cdi_07q2/comparison.json`: v1 measured
120729 rendered tokens, v2 117628; both had 18 rendered probes and an 8192 reserve
within the declared 131072 context. This comparison predates the added citation-ID
clarification sentence; rerun for exact successor counts. It certifies local geometry
and original reconstruction, not deployed model quality or external cache correctness.

`builtin.prefix_generation` is now a concrete registered immutable-storage primitive
with Read and Retain operations over the existing `PgPrefixGenerations` owner. It
neither compiles nor chooses/activates a prefix. Read returns exact bytes/qualification;
Retain returns the immutable quarantined receipt. It rejects extra/activation inputs
before backend effects, bounds transport and fails without a wired backend instead
of using legacy mutable storage. Canonical `prefix_generation_components` drafts
define separate reusable usages and Recipes. Retain's revision parsing is a separate
pure PythonCode step; binding immediately precedes its one Tool invocation. The drafts
declare typed contracts, preload interfaces, stop-only failure semantics and ten
command variants. Actual linked registration/schema/image verification, PostgreSQL
backend/reread/conflict, view-v2 import, contracts and negative primitive cases passed
in `prefix-registry-artifacts-zr4r4mre/accepted.json`. This is primitive/draft evidence,
not association approval or actual qualified Recipe admission. Complete the closed
bootstrap subject/behavioral producer and its catalogue selection/readback integration
before admitting production generation jobs.

The closed qualification input must be genuinely source-backed. The installation
adapter embeds the seven actual packaged guides, archives their complete originals
and evidence ranges, and executes the shared effect-free renderer through the
captured Monty utility worker under supplied preparation limits. Its manifest says
`installation-qualification-baseline`, `documentation-only`, token geometry
unmeasured, quality unmeasured and cache unverified. It retains the actual assembly
and renderer source identities and does **not** claim the offline compiler ran.
This baseline qualifies storage behavior only; it cannot stand in for a complete
approved component-library source export or a production base prefix. Never add a
hidden fixture table, synthetic guide text or invented compiler hashes to satisfy
an approval check.

Candidate retention must compare the exact packaged Tool, Skill, PythonCode,
ToolSkill and canonical Recipe documents after immutable storage, compile the typed
`selected` variant and inspect its exports using the captured worker. Initialize
only missing native Tool settings; preserve an existing operator block. Candidate
retention itself neither approves nor changes active routing. Shared contained
admission/transport/settlement mechanics may be reused from reply qualification,
while each usage retains its separate behavior oracle and closed producer identity.

Retain qualification executes the pure decimal preparation at `0:1` **and** its
one actual storage dispatch at `0:3`; Read dispatches at `0:2`. Verify complete
SQL-reread bytes, immutable duplicate receipt/timestamp, malformed package rejection,
predecessor conflict, live-policy denial and zero dispatches on invalid decimal
preparation. Read covers exact bytes, missing/malformed identity and live-policy
denial. Check the actual invocation journal, one initial attempt, settlement and
unchanged active catalogue/legacy serving selection. Development checks cannot
substitute for committed producer evidence. Persist a subject-specific Q1 intent
before behavioral effects, retain actual observations and exact association approval,
and independently verify the durable rows on readback; incomplete intents require
reconciliation, never automatic case replay.

The retained Tool-parameter parser must preserve genuine omission separately from
Recipe consumer defaults. A Tool can expose optional operation-specific parameters
without supplying null or a synthetic default. Keep the ordinary task/step input
constructor's required-default rule; use the distinct Tool-parameter constructor
only when preparing the registered Tool contract. Supplied null/invalid/undeclared
arguments still fail, declared defaults are validated and materialized only for
missing values, and cross-parameter inequalities fail if a supplied operand's
required comparison target is absent. Do not work around this gap by assigning
multiple stable Tool identities or incompatible per-usage Tool revisions to the
same native primitive.

The current catalogue's combination-coverage check does not cover standalone pure
Recipe components through a Skill association. Add narrowly scoped canonical-workflow
coverage for Retain's exact formatter only after the complete selected packaged
Recipe has matched its committed qualification subject. Reusing the storage Skill
elsewhere does not qualify another Recipe's pure code. Do not add a generic pure-code
exemption or accept arbitrary records labeled `system_seed`.

Installation qualification groundwork is implemented and checked in
`target/validation/prefix-registry-artifacts-anxaw5bg/accepted.json` (immutable source
`prefix-registry-source-rqhphieu`, captured matching worker and native test binaries).
The installation retains both exact Read/Retain candidates and the actual linked
native Tool definition without activating them; existing Tool blocks remain intact.
Shared admission/transport/settlement and immutable proof-storage mechanics are reused
with the reply qualifier, whose ordinary matched/No-Match regression also passed.
The closed installation-owned generation proof producer now commits both exact
usage approvals before runtime readiness. It records subject-specific Q1 intent
before effects, executes Retain's parser/storage and Read through real contained
Monty and PostgreSQL, atomically retains behavior/association records and verifies
the complete readback. Existing committed proof is reused without another dispatch;
partial qualification requires reconciliation rather than automatic replay. Coverage
of Retain's formatter is restricted to its exact canonical workflow. Candidate
retention and usage qualification do not activate a generation/compiler job.

The stronger producer passed in
`target/validation/prefix-registry-artifacts-dtr9ngal/accepted.json` (source
`prefix-registry-source-aibwq6cr`, matching captured worker/test binaries, target2).
Expected rejections must be Python failures from the same Ready worker/root and the
expected Start or ResumeHost command, not a timeout, crash or unrelated transport
failure. Readback counts all run invocations and rejects another Recipe's dispatch.
The malformed submitted package's SHA must be absent before and after the cases
and on committed readback. The actual journal, exact arguments/answers, initial
attempt, full baseline bytes and immutable duplicate receipt/timestamp are checked.
Serving selection remains unchanged. Canonical preparation/coverage, startup-owned profile registration, complete legacy
archive and domain procedure checks subsequently passed in
`target/validation/prefix-registry-artifacts-iga3n4b4/accepted.json` (source
`prefix-registry-source-0jef7hzv`). The exact Read/Retain Recipes prepare through the
real catalogue owner; a different Recipe cannot borrow the formatter's coverage.
The native Tool now uses its ordinary registered catalogue UUID throughout candidate
retention, approvals and live-policy lookup. Registration metadata remains pending;
its label does not supply approval and creates no installed human-Q2 requirement.
Do not infer full compiler-job admission from this storage-usage acceptance.

The general retained Tool parser now has a distinct
`InputContract::tool_parameters_from_value` path. Omitted optional parameters stay
absent; no null/default is fabricated. Ordinary Recipe consumer-input parsing still
requires its declared default. Contract regressions, registered capability inventory,
strict affected-package lint, real native workflows and offline compiler checks pass.
The pure renderer searches literal spans/control markers instead of iterating every
ordinary character. The preserved renderer independently establishes identical bytes
on all seven guides and edge cases; `render-equivalence.json` records the source and
output hashes. Full-guide contained Monty now completes in approximately 5.8 seconds,
where the prior implementation exhausted its 30-second deadline. This is rendering
performance evidence, not a model/cache-quality claim. Validation workers derive a
finite frame from the complete encoded baseline, align their value bound to that frame
and account for both frame buffers and adapter headroom within their existing physical
ceiling; no instance memory or deployment setting is changed.

Offline base and Prefix-Maker packages now retain the complete compiler entry,
shared evidence helper, renderer and profile-adapter bytes alongside their hashes.
They reject a procedure change during compilation before publication. Their explicit
`unqualified-development-process` attestation records that byte retention is not
trusted execution or production worker qualification. The same accepted snapshot
checks actual source bytes, immutable imports, real catalogue export and local
cached-tokenizer geometry; it does not establish the serving tokenizer's identity.

Legacy import must preserve all current compatibility-row fields, not merely bundle
text/scope. Forward migration V152 archives `to_jsonb(original_row)` with a v2 legacy
contract while retaining V142's immutable historical partial records. It neither
invents previously omitted historical fields nor changes any serving pointer,
operator profile label/revision or qualification. Its existing-configuration
migration fixture and repeated-import behavior passed in the same accepted snapshot,
including every original field, operator metadata and unchanged serving generation.
Startup explicitly retains the six first-party profile definitions before qualification,
independently of WebUI construction, and preserves existing labels, compiler entries
and revisions. Repeated retention does not create generation/readiness evidence.

The new `prefix_build_worker.py` development interface consumes a bounded
`brassclaw-prefix-build-request/1` byte frame: exact source manifest/artifacts,
fixed base/Prefix-Maker profile, supported declarative view/reserve and an optional
local tokenizer target. It accepts no compiler pathname, provider, collection,
deployment or activation instructions. Sources are checked and materialized only in
an isolated temporary directory; complete output is returned as an immutable package,
never a partially published generation. The actual protocol worker bytes/checksum are
retained in its compiler provenance. Target tokenization currently permits only the
registered `hf-qwen2-fast/1` adapter, local artifact names and no remote mappings or
external file references. Other adapters require implementation/qualification.

Independent CLI reproduction and network-denied source-only checks pass locally for
both supported profiles. The actual downloaded model config/tokenizer originals are
retained at `target/validation/prefix-model-source-cacik5n7/provenance.json`; they came
from a read-only copy of the registered model host snapshot, not invented metadata.
Nine domain compiler regressions pass using those files, including a roughly 99k-token
stable prefix and preservation after a failed rebuild. The shared `prefix_procedure.py`
helper now preserves entry/helper bytes and source-change rejection for Defensive,
Home Assistant, tomedo and Sempai. Their legacy artifact formats and worker adapters
still need full shared-engine migration; byte preservation does not establish parity.

The byte worker is a development interface, not a registered qualified runtime Tool:
complete procedure/dependency/interpreter retention, resource/cancellation enforcement,
actual native dispatch, durable job inputs/correlation and closed behavioral approval
remain required before the Generate action may call it. The generic storage importer
checks an optional worker checksum against retained bytes and rejects an undeclared
worker artifact. Production model input/specification ownership must preserve tokenizer
and configuration originals independently of every prefix package; do not duplicate
large tokenizer files per generation or substitute a stored hash for retained bytes.

Generation jobs must reuse the existing host-selected `TaskEntry::InternalRecipe`
transport and global Monty service. The worker wire protocol is now 15; capture and
use the worker from the same build. Public user text cannot select this private
entry. Implement the exact internal prefix-job resolver/owner and durable job
correlation; do not add a second queue, VM or public-command fallback. Check the
actual durable invocation JSON limits as well as the primitive's 64 MiB transport
ceiling before approving large package Read/Retain execution; technical failures
must preserve evidence rather than silently clip artifacts.

Runtime admission is factored into the existing `admit_user_message_internal` boundary;
the blocking caller retains its send lock and completion/history hooks. Matched and
No-Match global-Monty regression passes with retained VM identity. Durable internal
scope/correlation retention, asynchronous admission API and prefix job references still
need implementation; the existing ephemeral one-shot scope map is not recovery authority.

### Phase C continuation — large values, model originals and Recipe compiler (2026-10-15)

The accepted `target/validation/prefix-registry-artifacts-zjnchvho/accepted.json`
freeze exercises a real approximately 10 MiB source package through typed Monty,
registered generation dispatch, PostgreSQL invocation persistence and immutable
reread. V154 widens the invocation data columns to their finite 64 MiB transport
capacity. V155, supplied by the shared resource owner, retains each invocation's
admitted data limits; preserve that contract rather than replacing it with a new
hardcoded limit. Immutable schema/revision document limits remain separate. These
checks establish large-data custody, not complete-catalogue quality or activation.

V157 adds content-addressed model artifact sets and exact specification bindings.
The operator API imports original model configuration/tokenizer/template files;
the server derives checksums and authenticated provenance, deduplicates identical
sets, and exposes immutable originals separately from the small metadata catalogue.
The model-design panel accepts fixed artifact filenames for the selected exact
specification and coalesces duplicate submissions. Stored originals do not identify
the running server or qualify a model design. Native database/route acceptance passes in
`target/validation/prefix-registry-artifacts-icy572v0/accepted.json`, together with
four focused browser-module tests. This freeze preserves a previously compiling
merged dependency snapshot plus the upload-ceiling changes; it does not certify
newer concurrently edited resource code. The accepted prefix implementation bytes
still match the workspace at that receipt. The real
40 MiB import exposed an outer-ingress mismatch: the route declared 64 MiB while
the default outer cap was 14 MiB. The default now matches 64 MiB; smaller route
limits and explicit operator-configured smaller ceilings remain enforced, with
focused ingress checks. Do not solve this by disabling body limits. Preserve
V151/V153 embedding changes and V155/V156 resource/Tool-identity changes owned by
parallel work; migration numbers must remain unique.

Source-only compilation is now being expressed as reusable class-22 components,
not as a specialized Rust compiler workflow. `prefix_compiler_components.rs`
packages an effect-free section library, the existing literal renderer, a compiler
library and five small typed stages. `source_compile_recipe_draft` sequences:

1. Validate the exact positive profile revision using the existing formatter.
2. Prepare the complete pinned source snapshot in PythonCode.
3. Invoke the reusable JSON-Tool UTF-8 SHA-256 usage, then prepare references.
4. Hash evidence identities, then render the literal or deduplicated reference.
5. Hash the complete artifacts, then construct their identity.
6. Hash that identity, then finalize the immutable package.
7. Invoke the existing immutable Retain usage once.
8. Format that actual quarantined result into an internal-task receipt; the job
   owner must verify its correlated durable row and dispatch/effect settlement.

Every independent Tool dispatch has its own immediate ToolSkill/PythonCode pair.
The `sha256_utf8` JSON primitive hashes one ordered finite batch without filesystem,
network or workflow decisions. It preserves UTF-8, newlines, ordering and duplicate
values. The canonical digest Skill/Recipe is reusable outside prefix compilation.
The five compiler stages perform JSON/Base64/formatting in Monty and never call a
Tool. `prefix_units.py` is shared with the offline compiler; all affected compiler
procedures retain its original bytes and checksum. CPython oracle checks cover
both profiles and both reference views; actual Monty and retained-graph checks
remain required before qualifying these drafts.

This first Recipe produces **source-only, unmeasured, quarantined** base/Prefix-Maker
packages. It does not implement source collection, target tokenization, accepted
model designs, provider selection, jobs, composites or activation. More than 8192
strings in one digest batch currently fails the technical operation bound; implement
qualified Recipe batching before claiming arbitrarily large complete-library builds.
Do not omit source rows/units to fit that operation. Complete procedures in the input
snapshot must come from the selected retained code graph, not caller-supplied claims
about which code ran. The generation importer checks retained procedure links but
cannot turn arbitrary caller bytes into trusted execution provenance.

See [Recipe-owned source compiler](../reborn/prefix-source-compiler.md) for the
component/input/receipt contract. Development acceptance now covers the complete
preloaded compiler graph, exact CPython/SHA oracle agreement, actual registered
hash/storage calls, duplicate publication, missing mandatory source rejection and
live policy blocking. These checks do not manufacture bootstrap approval or enable
admission. The preload interface forbids Python default arguments: the shared unit
helper takes both arguments explicitly, while its typed public contract supplies the
`.md` default. Recursive result fields must declare requiredness, and producer
constraints must guarantee their consumer's bounds before the first effect.

Model artifact originals also expose a separate capacity hazard: the imported
`tokenizer.json` configures 1024-token backend truncation. A direct low-level encoder
inherits that setting; it is not the model's context limit. The existing HF probe
path validates the complete decoded leading reference, so a truncated probe cannot
qualify it. Any new tokenizer adapter must explicitly disable truncation/padding,
retain those effective settings and check the entire rendered request, including
its tail and task/output reserve. Keep model architecture capacity, configured
provider context, tokenizer defaults and runtime cache layout as distinct evidence.
Untruncated raw guide-only views measured about 115k–120k tokens in a development
probe; that is not full catalogue coverage or a serving/cache qualification.

The same accepted freeze passes the complete source compiler through preloaded
Monty, registered native dispatch and real PostgreSQL; its output equals the
independent CPython/SHA oracle exactly. It also passes large-value persistence,
existing catalogue activation/export, model artifact import/readback/deduplication,
legacy/profile/Tool identity preservation, tokenizer-aware offline workers and all
four domain regression suites. At that freeze, Q1/approval and durable Generate admission remained
open; accepted source-only qualification is recorded below. The neutral
`SourceCompilerIds`/`source_compiler_drafts` factory supplies one component-definition
source for installation and behavioral checks, retaining the existing canonical
Retain usage rather than copying it.

Installation candidate retention, canonical JSON-list command parsing, exact
retained subgraph selection and preservation of an existing JSON Tool identity/
operator block pass in `target/validation/prefix-registry-artifacts-_knynqqf/accepted.json`.
The factory now supplies both source and digest candidates without claiming approval
or activation. Source tasks pin their own complete dependency closure; unrelated
canonical Recipes are not carried into the task manifest. Captured command strings
are parsed by explicit pure PythonCode into typed lists, never implicit coercion or
source substitution. JSON usages share a typed JSON-text transport while the native
primitive preserves structured-call compatibility.

The archive correction passes before qualification in
`target/validation/prefix-registry-artifacts-e6fzt2b9/accepted.json`: older output
`sources.json` retained input-package object names although generation originals
live under `raw/<hash>`. New `brassclaw-prefix-generation-sources/1` inventories
name actual archive files and pin `input_artifact`: the exact canonical Monty source
snapshot or exact offline input-manifest bytes. All builds retain their actual
profile frame/design, including literal-v1 base builds. Validate original/output
metadata, evidence identities/ranges/counts and reject re-signed contradictory
links; preserve legacy input-namespace manifests without rewriting old generations.
An input manifest changed during compilation must leave the last good generation
and publication state intact. This is evidence integrity, not model/cache quality.

The internal-task adapter now has an explicit host-owned selection contract and
receipt verification hook. The bounded development observer runs the actual
packaged global Python root and its normal Recipe flow; Rust no longer needs to
repeat the compiler's step order or input wiring in that acceptance test. The
observer supplies no approval and is not a second production execution service.
Its negative checks must identify the original contained Python rejection,
expected invocation/ResumeHost origin (including cooperative ResumeControl for
long pure execution), unchanged Ready root and actual Tool answer;
a crash, timeout or framing failure is not the expected rejection. Successful
completion additionally checks immutable PostgreSQL package bytes and the native
Retain answer against the returned receipt. Shutdown/reaping precedes closing the
private validation claim, including error paths. Ordinary chat completion still
requires its verified reply, while internal completion requires its verified
receipt. The exact installed source graph and this internal Python execution path pass in
`target/validation/prefix-registry-artifacts-bq4q3hc0/accepted.json`. That receipt
is explicitly scoped to the native integration test, not the whole validation
suite or later source edits. It covers exact package/oracle agreement, duplicate
retention, missing-guide rejection, live policy blocking and run-wide SQL records
matching the observed argument/answer fingerprints, selection and attempt count.
An intended pure rejection may follow a cooperative compute-slice yield; requiring
only the initial Start command would incorrectly reject that valid observation.
These checks do not enable Generate.

This historical batch used `/Users/ollama/brassclaw-target2` and immutable
source captures to identify its tested inputs. That target/build arrangement is
retired. Preserve the source/binary/receipt archives as evidence, not as new build
inputs. A preserved dependency freeze does not establish compatibility with newer
shared interfaces. For new validation, compile the current common checkout using
the configured canonical target and report the actual relevant check results.
The compiler oracle's transport capacity must use the largest individual exchange,
not the concatenation of all independently saved oracle stages.

The same complete validation freeze passes the independent closed-fixture oracle
for both profiles and render formats, including exact shared-unit origin maps,
complete originals/procedures, historical-source exclusion and rejection of a
coherently re-signed different frame. Its small synthetic originals qualify no
domain knowledge or model behavior. The native fixture-matrix producer/readback
checks are now being added as a separate development slice: four profile/view
successes, duplicate retention, missing guide, blocked Tool and invalid revision.
They must compare all invocation rows for each run with the actual observations,
recompute UTF-8 digests independently, reread immutable original packages and
receipts, and reject altered proof records in a coherent database transaction.
The evidence reader must derive the expected fixture from the closed profile/view
inventory, require unique run/root/thread identities, match the exact retained
selection and completed/failed steps, and check original-UTF-8 digest results against
independent SHA-256. Rejected pure work may legitimately fail after a cooperative
ResumeControl; rejected Tool work must retain its actual authorization answer and
ResumeHost origin. Include a complete fenced YAML original with CRLF endings and
an embedded model control token: archive bytes stay exact, and only the model-facing
literal view escapes that token using an outer fence longer than the inner fence.
These tests create neither installation approval nor catalogue activation. The eight-case native producer and coherent evidence reader pass in the scoped
`target/validation/prefix-registry-artifacts-yrg3_na1/accepted.json` freeze. The
installation writer acceptance below pins the complete packaged
graph, native implementations, captured worker, observer/validator/oracle sources
and existing canonical Retain approval; commit its exact Q1 intent before effects;
and atomically store verified behavior with its hash-usage approval. Private
qualified compiler selections are separate from routable chat Recipes: their
result is an internal receipt, so qualification must not advertise a normal chat
route. Test proof reuse with zero new dispatch, rollback-only incomplete-intent
rejection, exact association pinning and denial of public-route promotion.

Namespace synthetic fixture artifacts by the closed qualification identity.
A new binary/native qualification can otherwise collide with an unchanged
fixture package retained under an older profile revision. The same qualification
still reuses exact fixture bytes; older records remain immutable. Scope belongs
to validation metadata, never the production prefix design or model-facing text.
The scoped observation record uses `source-compiler-behavior-observations/2` and
its reader rejects a different scope; old proof bytes remain with their original
producer/subject. Treat the closed writer as accepted only after its native checks pass; incomplete
qualification intents require reconciliation without automatic effect replay.

The closed compiler writer, scoped/YAML fixtures and private approved selection
pass in `target/validation/prefix-registry-artifacts-p6m1aifn/accepted.json`.
This is the accepted source-only installation slice: exact graph/native/worker
integrity, committed intent before effects, actual eight-case behavior, coherent
proof readback, existing Retain approval reuse, atomic hash-usage approval,
zero-dispatch proof reuse and incomplete-intent rejection without modifying
immutable completed evidence. Normal matched/No-Match execution, archive integrity,
model artifact APIs, large values and all domain compiler regressions also pass in
that freeze. It does not implement production Generate jobs, accepted model designs,
provider resolution, composites, activation or cache qualification.

Next, reuse the global `InternalRecipe` entry and existing durable turn queue,
attempt retention and settlement owners for asynchronous Generate admission.
The initial normal-driver transport constructed only Chat entries, rejected internal
completion receipts, and verified result references only through thread messages.
The neutral admission/completion extension passes the complete
`target/validation/prefix-registry-artifacts-vxkw6s17/accepted.json` freeze;
the two internal-reference/outcome contract tests also passed against that exact
captured test binary. Prepare entry data and ports together, admit only host-selected
internal work, represent a verified receipt as a run-bound `result:monty.` reference
and reread the exact privately settled admission/scope. Match the persisted source
binding before admitting its trusted-internal marker. Ordinary chat retains transcript
verification; internal completion cannot carry a fabricated assistant reply.

Immutable job definitions pass the scoped native
`target/validation/prefix-registry-artifacts-dxv273th/accepted.json` freeze. V167 links
one definition to the existing turn run UUID and retains request, source inputs,
profile revision, complete compiler selection, actor and data scopes. The first capture
survives same-request retries; conflicting requests and definition mutations fail.
That receipt proves metadata/idempotency only, with no new admissions or Tool calls.
It does not qualify a Generate action.

The source-only snapshot submission/resolver passes the scoped native and
standalone-consumer freeze in
`target/validation/prefix-registry-artifacts-c5z_1d40/accepted.json`. Reuse ordinary durable ingress, the existing global service, retained
attempts, native invocation journal and result settlement. Reconstruct transport
scope from the retained definition after reconnect rather than relying on a one-shot
in-memory map. Verify the exact closed compiler approval and program bytes before
selection; never replace a retained selection with a newer compiler. Match archived
source data and compiler-procedure bytes to the actual pinned execution. Internal
builds do not advertise a provider/MCP exchange or send model requests. The native test verifies real
ResultOnly completion in the same global VM, no synthetic assistant reply, correlated
immutable output, duplicate submission without additional dispatch, conflicting
profile rejection and rejection of misidentified compiler procedures before retaining
a definition. Strict affected-library linting, the two internal-result contracts and
three actual standalone driver/cancellation/preparation regressions pass in that
freeze. This low-level snapshot boundary does not collect authoritative
sources or implement the normal WebUI Generate action. An incompatible restart still
requires retained-artifact recovery or explicit reconciliation, never a latest lookup.

Do not publish this compiler as a chat Recipe, invent a reply, copy the review
executor or create another VM/job loop. The existing reserved internal source binding
marks host-created prefix turns and prevents Tier-2 fallback. Preserve that host-only
proof; never infer it from a user's text.
The database-backed core-source capture passes the scoped native/strict
standalone freeze in
`target/validation/prefix-registry-artifacts-lbw2_5kw/accepted.json`. It reuses
class-17 revisions in `PgComponentRevisionStore` for the seven component guides,
Prefix-Maker reference and declarative source-only framing. The installation owner
compares the complete captured document graph to its actual packaged bytes. An
editable `reborn_docus` row is only a compatibility projection; reseeding never
overwrites it, and its source/status labels cannot establish packaged authority.
New packaged revisions and unapproved operator revisions remain retained separately.
The core collector reads exact package revisions plus the existing coherent catalogue
export, preserving both complete provenance envelopes. It does not scan a developer
checkout, look up newest document heads during execution, fetch network references,
qualify a target model or pretend to cover private workflows/all documentation.
`core-reference-designs.json` supplies a fixed literal-v1 source-only frame; this is
not an accepted model-specific design. Normal complete-library collection and the
remaining profile adapters are still prerequisites for the full Generate surface. The native test
checks exact originals, idempotent capture, preservation of editable Docu projections
and newer unapproved revision heads, rejection of forged packaged bytes, and real
core compilation through the existing global VM with no assistant reply or repeated
effects on reconnect. Its captured input was 1,370,747 bytes and observed completion
about 13 seconds; this is one local source-only observation, not model/cache or
performance qualification. Use a bounded large-build test observer, not the default
ten-second chat fixture wait, while keeping real task/resource limits unchanged.
The actual routing catalogue is included alongside selections so disabled Recipe
metadata is not hidden by a provenance-only transfer envelope.

Read-only job inspection passes the scoped native/strict-standalone and route
contract freeze in
`target/validation/prefix-registry-artifacts-oboa7t42/accepted.json`. It uses the existing
`InterceptorConfigService`, facade and authenticated v2 ingress. It projects the
immutable request/compiler/input hashes plus actual turn/admission/operation state;
status polling must not reload large source/compiler blobs, drive a task, fabricate
completion or repeat effects. Verify run/source-binding/scope/actor correlation and
only expose a completed result after its real admission settlement and turn completion.
A definition retained before a lost admission remains distinct from a queued run.
Record counts are observations, not reconciliation evidence or retry authority. The next metadata revision supplies the existing opaque thread/run references
for shared event/cancellation controls; do not introduce a parallel cancellation
workflow or infer that unresolved native work has been reconciled.



The normal Generate action remains gated on the complete job/source/target
contract. First-party bootstrap evidence stays distinct from authored human Q2.

### Phase C continuation — contained model encoding (2026-10-17)

The model-specific measurement prerequisite now has an offline encoding engine
(`brassclaw_model_encoding`) behind the existing retained `monty_worker` utility
boundary, not another global VM, process supervisor or Python environment.
Utility protocol 4 adds one `EncodeModel` operation; exact worker/image changes
must invalidate incompatible qualification rather than accepting an older
protocol. Assets and contexts are typed data; no tokenizer remote code, template
filesystem loader, source selection or provider calls run in this operation.

Accepted frozen offline evidence:
`target/validation/prefix-registry-artifacts-dvpfuk_0/accepted.json`.
It covers three focused engine tests, an explicitly executed retained-Ornith
Hugging Face oracle and an actual contained-worker test. The oracle uses offline
`AutoTokenizer.from_pretrained(..., trust_remote_code=False)` and the complete
retained six-file model set. It compares full rendered-byte and token-ID hashes,
counts and common-prefix hashes for long system text, tool schemas/arguments,
Unicode, reasoning/history and image placeholders, plus a separate 2,048-value
finite-float corpus. This is finite target-specific evidence, not universal
chat-template equivalence, a server/cache measurement or model answer-quality
acceptance. Python is an independent development oracle, not a runtime dependency.

Keep JSON object order all the way through nested tools/arguments. The renderer
must use Python/Hugging Face JSON spacing and escaping, rather than HTML-safe
Jinja JSON. Preserve exact numeric float parsing/printing; the initial larger
oracle exposed different token IDs without JSON float-roundtrip parsing. Saved
tokenizer truncation/padding are explicitly disabled; adding special tokens after
a completed chat template is disabled too. The long-input regression proves that
the model tokenizer's saved 1,024-token truncation does not silently shorten the
measurement. Template wall-clock functions fail closed.

The contained fixture explicitly requests a finite 1 GiB utility-worker memory
ceiling and 120-second response deadline. These are test preparation settings,
not automatic production overrides. Production must capture acknowledged limits
and visibly report insufficient capacity. The shared VM heap and task compute
account remain separate. Timeout/reap failure is never successful measurement or
permission to activate a prefix.

The `builtin.model_encoding` native adapter and canonical single-usage
Skill/PythonCode/ToolSkill/Recipe drafts are implementation work in this slice.
They resolve an exact specification/artifact-set binding from the existing model
store and share the already retained preparation worker/settings owner used by
source inspection. No second model catalogue, pool, queue or cache store is
introduced. Native registration, real PostgreSQL execution, current policy,
exact usage inspection and consumer regressions require their own acceptance
receipt before this paragraph can be promoted to a completed production claim.
These drafts are not approved/activated merely because the engine oracle passes.

Next, bind this usage into the generation Recipe's accepted per-model design,
freeze full provider render contexts/probes, compare required stable-prefix IDs
and context/output headroom in reusable PythonCode, and retain target evidence.
Keep full model/config/runtime fingerprints and actual attention/recurrent-state
cache qualification separate. A tokenizer measurement does not establish KV
capacity, LMCache persistence, salt isolation, cache hits or answer accuracy.
Normal Generate remains gated on those remaining source/design/job contracts.

The first native adapter measures the exact chat template in the retained model
artifact set. That is not evidence of the serving server's effective injection
template. Pin original model-template bytes and effective target/injection-adapter
bytes separately. In particular, an ordinary system message can follow a dynamic
tool-schema preamble: a long reference inside that message does not create a
common leading cache prefix across different tool sets. The accepted adapter and
probe workflow must cover the actual tool/no-tool, role/content, history and
thinking variants, measure complete rendered requests (never concatenate separately
encoded prefix/suffix tokens), and reject an incompatible stable-prefix claim.

Retained-task prefix lookup is also part of this boundary. A missing, expired or
poisoned run capture must return a typed prompt-assembly failure rather than read
the current active catalogue. Scope/correlation must match the captured task.
The compatibility unscoped lookup is not the retained-task resolver. Fail-closed
lookup is a prerequisite; it does not establish durable prefix/model selection
restoration after restart. Retain generation, model/target/design and complete
render-adapter identities through the existing continuation contract before
claiming that restart/resume or the new prepared-message model usage is qualified.

#### Native encoding integration and observed capacity

Frozen receipt `target/validation/prefix-registry-artifacts-0a0vlb9z/accepted.json`
now covers the three focused engine tests, offline full AutoTokenizer oracle,
actual contained-worker operation, affected-library strict lint, standalone
strict lint/consumer regressions, authenticated job routes and the native
component-deployment integration. The latter imports/reads exact model artifacts
through the real API/DB, measures the actual source-job output through the
registered native handler, checks live Tool blocking and malformed/missing
artifact failures, and prepares/inspects the real canonical encoding-usage graph.
It preserves one ordinary global root. This is native candidate-usage evidence;
it does not approve/activate the new usage or qualify Generate, serving or caches.

That frozen core source build used literal-reference/v1: its model view contained
620,802 UTF-8 bytes; the two framed Ornith request probes each contained 144,778
tokens, with 144,767 common leading tokens for those two particular questions.
The measured request is larger than the historical 131,072-token serving setting
before any response/history reserve. A fresh read-only discovery on 2026-10-19
from brassclaw2 to the model endpoint still advertises 131,072 for the same model
ID (`/Volumes/SSDE/brassclaw/target/validation/prefix-runtime-discovery-a3co1off/observation.json`). This confirms only the reported upper limit, not
long inference or current KV/recurrent layout. Preserve the complete candidate
and report Capacity blocked until a compatible target/design is qualified;
Generate must not change the operator's inference deployment. Do not activate it by ignoring that limit,
clipping text or relabeling the configured/architectural context. A larger
architecture limit is not evidence that the deployment has that capacity.
The complete retained package is in that receipt directory's
`core-generation.json`; no source or failed artifact was removed.

The successor packaged **source-only** design selects the already supported
literal-reference/v2-deduplicated whole-unit renderer. It retains original source
bytes and every origin/range mapping and emits repeated complete units once;
it does not summarize unique content or change an accepted installed model design.
Old source snapshots continue to name their original renderer mode. The resulting
size/headroom must be measured again through the actual generation/encoding path
before a model-specific design is accepted. The successor also adds a separate
effective-template encoding usage: original and supplied-template identities
remain distinct, and template measurements never approve an adapter or claim
server injection/cache compatibility.

#### Effective-template compatibility and retained-run correction

The successor native integration in
`target/validation/prefix-registry-artifacts-tvsgm75_` correctly failed rather
than accepting a bad effective-template measurement. Preserve that failure and
its complete core-generation artifact. Its source-only v2 build measured 602,082
context bytes and 141,820 framed tokens; whole-section deduplication alone does
not make the mandatory core fit the historical 131,072-token setting.

The independent reproduction identified two Python/Jinja compatibility gaps:
sequence concatenation must remain indexable/sliceable, and `None is iterable`
must be false. MiniJinja's lazy iterable semantics are not interchangeable with
the existing injection wrapper's list semantics. A version downgrade was not a
solution: inspected 2.x releases also use lazy concatenation. The pinned
`vendor/minijinja-compat` source retains the upstream license/hash inventory and
an explicit opt-in eager-concatenation patch. The encoder environment separately
corrects the None test and retains Python JSON/numeric behavior. Do not change
the effective template bytes to conceal a renderer mismatch. Dependency/adapter
changes require new worker identity and affected target qualification.

Frozen receipt
`target/validation/prefix-registry-artifacts-33w6ysc2/accepted.json` covers four
engine tests, the offline full AutoTokenizer oracle, strict engine/worker lint
and actual contained-worker checks, including safe classified template failures.
The full 12-context injected-template reproduction additionally matches the
independent Hugging Face report exactly:
`target/validation/prefix-encoding-reproduction-xk42v_67/accepted.json`.
It measures 141,807 shared leading tokens across the tested tools, role/content
and thinking variants. These are offline/contained measurements, not a native
usage bootstrap approval, server cache/state proof or model answer-quality claim.
The corrected native DB/usage integration is now accepted in
`target/validation/prefix-registry-artifacts-pct_i_6f/accepted.json` (2026-10-19).
It also covers affected-library strict lint, the closed source-compiler fixture,
internal result checks, authenticated routes and the three standalone consumer
regressions. Its complete source-only v2 context is 602,078 bytes; original-template
requests measure 141,853 tokens each and the 12 effective-template contexts retain
141,840 common leading tokens (complete requests: 141,857–142,100 tokens).
These counts belong to that exact frozen source package, not future catalogue
content. The canonical encoding usages remain candidate definitions: direct
native dispatch plus retained-graph inspection is not closed bootstrap
qualification or activated Recipe execution.

The fallible retained-run source API and ordinary matched/No-Match path passed
separately in
`target/validation/prefix-registry-artifacts-tvsgm75_/retained-prefix-accepted.json`.
Missing/expired captures now stop prompt assembly; a run cannot fall back to the
latest active prefix. Captured actor/project correlation is checked. Durable
restart restoration and complete model/design/prefix resolver identity remain
separate prerequisites; a String-returning compatibility source is not that
complete resolver.

Controlled formatting experiments were retained without promotion:
`target/validation/prefix-dedup-counterfactual-3qwnax8e` and
`target/validation/prefix-fragment-experiment-5g_nm4ai`. The latter reconstructed
all original text but still measured 139,885 framed tokens. It is not a supported
renderer or an accepted design. Do not keep splitting/rewriting prose merely to
claim a fit: the plan requires literal prose/Python/YAML, safe strict-JSON-only
display projection and explicit coverage/capacity rejection when mandatory
knowledge cannot fit. A compatible larger context/model may be necessary.

#### Reusable measured-capacity comparison (2026-10-19)

`scripts/prefix/prefix_capacity.py` and its class-22 `capacity_draft` now provide
pure `assess_prefix_capacity(inputs)`. The component consumes complete canonical
native observation bytes, expected specification/artifact/effective-template
identities and an explicit retained limits reference with context/output/additional
input allowances. It rejects identity mismatches, partial observation shapes,
noncanonical JSON, disabled full-token measurement, invalid counters, booleans,
nulls and negative allowances. It retains worker/preparation/template identity,
complete maximum request count, common-prefix count and fit/headroom/shortfall.
It never picks limits, drops knowledge, changes a model design or grants approval.

Frozen receipt `target/validation/prefix-registry-artifacts-ceyqqz_1/accepted.json`
executes that exact component in the real contained Monty worker using the native
12-context observation from the real source job and model store. The fixture's
131,072-token comparison fails fit; its explicitly hypothetical 262,144-token
comparison fits. Neither case establishes a deployed context limit, cache reuse
or answer quality. Template/artifact substitution, boolean context, null output
and negative additional-input cases stop. The same batch passes strict affected
library lint, compiler/result fixtures, authenticated routes and native candidate
integration. The component remains a packaged draft pending workflow/bootstrap
integration, not an activated Generate path.

#### Shared template adapter and preload acceptance (2026-10-19)

The offline `inject_reference` implementation now lives in the pure
`scripts/prefix/prefix_template.py` module, re-exported by `prefix_evidence.py`.
Its class-22 `template_adapter_draft` exposes `build_prefix_template(inputs)`;
there is no host call, serving edit or inference process. Offline compiler
procedure snapshots retain the new helper and the base compiler records/verifies
its dynamic `template_adapter_sha256`. Old archived packages remain readable.
Keep the entire compiler directory when moving offline scripts between machines.

Monty and CPython return byte-identical effective templates for the full retained
reference (`target/validation/prefix-template-component-i_u3awfl/accepted.json`).
The independent oracle now also requires the complete literal reference at the
start of every rendered request, rather than comparing token hashes alone.
It rejects a deliberately incorrect reference; all 12 retained contexts pass
(`target/validation/prefix-full-reference-oracle-ok9vbgv7/accepted.json`). The
model template trims trailing separator whitespace in some empty-system cases;
this does not remove reference bytes and is not a promised stable separator.

Native receipt `target/validation/prefix-registry-artifacts-gg8vy9cz/accepted.json`
passes both pure components' real preload inspection and execution, the exact
native model store/encoding path, complete-reference and HF checks, explicit
negative cases, affected strict lint, payload-serialization capacity regression,
three standalone consumer regressions and authenticated routes. Its parent
`prefix-registry-artifacts-v9nwiqdb` separately passed the four engine tests,
full offline HF oracle, contained worker and both dependency-boundary checks;
that parent's full native run failed on the subsequently corrected preload
issue and is not a full native acceptance receipt. Preserve both failed runs
(`ej4azlex` and `v9nwiqdb`) as failure evidence.

The preload loader currently rejects default arguments, including immutable
ones. `capacity_integer` therefore receives an explicit lower bound at every
call. Successful utility evaluation alone had not established loadability;
retain actual preload inspection in the candidate acceptance path. This change
does not loosen the loader, approve a Skill combination or activate Generate.
Native request serialization also counts escaped JSON bytes against captured
worker/frame limits before allocating another complete request string; oversize
requests fail as resource errors without truncation or an attempted child process.
Native-library memory containment remains a separate activation prerequisite.

The next candidate workflow is `prefix_measurement_components::measurement_drafts`:
ten explicit Recipe steps compose canonical JSON-digest and template-encoding
usages with pure parsing, template preparation, identity verification and capacity
comparison. It verifies that the supplied original template belongs to the model
artifact observation, and checks the effective template against its independently
computed digest. The Recipe serializes its exact calculation as the nonempty string required
by internal result-only completion; it does not publish a chat reply or qualify
as an ordinary chat route. Its owner must verify the resulting receipt.
It does not choose accepted
designs, validate deployed limits, bless cache/quality or publish a generation.
It still needs retained-graph/behavioral acceptance, closed first-party usage
qualification and integration with the pinned generation job before exposure.
The first graph test rejected a result-root `required` field and then a producer
that did not guarantee the encoding consumer's nonempty strings. Correct the
producer/result declarations; never weaken native validation to make composition
pass. Keep the fast full-graph/preload check ahead of native model measurements.

The ten-step candidate now passes full retained-graph/preload inspection in
`target/validation/prefix-registry-artifacts-9dbqe6sy/accepted.json` (frozen source
`prefix-registry-source-xpnr6fsq`). The same scoped batch passes strict affected
and standalone lint, native model-asset/store/handler and full-reference HF
checks, three standalone consumer regressions, route contracts and the ordinary
matched/No-Match single-root regression. The formatter input needs explicit
`required` and `checks` declarations; the pure-component constructor validates
input/result declarations before producing drafts. This establishes loadability,
not execution/approval of the complete ten-step measurement workflow.

The registration slice uses the existing primitive seeder for
`model_encoding`, including native-image verification, preservation of legacy
UUIDs/operator metadata and current blocks. It leaves the Tool pending and does
not approve or activate usages. Its registration/upgrade tests pass in the
`9dbqe6sy` scoped receipt above, including preservation of existing native
identities, metadata and operator blocks.

For closed bootstrap qualification, reuse `AcknowledgedPrimitiveSettings` and
`HostPrimitiveSettingsSource`. Source inspection and model encoding share that
same settings owner and one retained preparation worker; do not add another
bootstrap settings table, provider pool or per-model preparation supervisor.
The installation owner selects and validates startup limits, binds the contained
preparation backend before component qualification, and transitions that shared
owner once to the actual acknowledged live view before ingress. Each operation
captures one limits revision; candidate data cannot choose a worker or raise its
limits. Verify startup preparation, prohibited rebinding, live revision uptake
and ordinary matched/No-Match single-root behavior. This lifecycle slice passes
the `9dbqe6sy` scoped batch; it is not itself model-usage approval.

Implement the encoding usage's noncircular system-seed qualification separately:
exact packaged source/associations/native images, Q1 intent before behavioral
Tool calls, deterministic closed tokenizer/template fixtures with independently
known token-ID/count/hash outcomes, actual store/binding/kernel/dispatch evidence,
negative/policy/resource cases, durable effect accounting and exact immutable
approval records. Fixture model/provider records must be unmistakably private
qualification data in the existing owners, not active consumer selections or a
second registry. The selected preparation owner must already be usable for this
pre-readiness qualification; do not bypass its limits or invent caller-provided
success evidence. Qualified primitive usages do not establish real-model HF
compatibility, deployed context capacity, hybrid cache or answer quality.
Preserve incomplete qualification intents and fail closed without replay.

The closed owner is now implemented in `bootstrap_encoding_approval.rs`, using
`prefix_encoding_candidates.rs`, `bootstrap_encoding_fixture.rs` and
`bootstrap_encoding_validation.rs`. Its scoped behavioral acceptance is
`target/validation/prefix-registry-artifacts-8_yma761/accepted.json` (source
`prefix-registry-source-om94oqsa`).
It retains both exact canonical usages, verifies native and worker images,
records group Q1 intent before dispatch, executes the actual global Python
Recipe path and rereads its immutable selection/attempt/answer journal before
writing or reusing approval. Fixtures have explicit private, data-only provenance
in the existing model/provider/artifact stores; their imports remain unqualified.
Operator imports cannot supply this origin or approval fields. Original and
supplied-template cases include known full token arrays despite saved
padding/truncation settings, duplicate determinism, policy rejection, invalid
identities/assets/tokenizer/probes/template and render-fuel exhaustion.
Proof re-reading and incomplete-intent checks must establish no additional
dispatch. The existing source-compiler qualification shares the same contained
Recipe transport builder; it cannot increase selected value/frame bounds.
Keep these usage proofs separate from whole measurement-Recipe qualification,
native-library memory containment, model/design capacity, HF compatibility,
cache correctness, held-out model behavior and production Generate activation.

That same scoped batch executes the complete ten-step measurement candidate
through the actual global Python Recipe path: eight executable steps plus two
binding steps, exactly two native dispatches, the full ordered report equal to
the independent HF-verified report, and the exact internal capacity receipt.
It checks actual journal selections/arguments/answers, one attempt per dispatch,
child containment/reaping and no new approval evidence or chat reply. The full
source reference remains retained and capacity-blocked at the fixture's 131,072
context limit. This is development execution evidence, not a production workflow
approval or target/cache acceptance. The batch also passes strict affected and
standalone lint, three standalone consumer regressions, routes and ordinary
matched/No-Match single-root behavior.

Preserve earlier `xd_ryxoy` and `fom8q98m` failures: the old test counted all
artifact sets globally and rejected the two additional private data fixtures.
The corrected check counts the operator namespace's bindings; the authenticated
catalogue still must return exactly its one imported set. No fixture or evidence
was deleted to make that check pass.

The next retention slice, `prefix_measurement_candidates.rs`, gives this workflow
one installed stable graph, reusing the canonical digest and supplied-template
encoding identities instead of allocating another pair of usages. The source
compiler shares the digest identity allocator. Verify that the installed graph
matches both existing leaf approvals and retains its separate unapproved workflow
state, then qualify its complete pure logic and integrate the selected model/design
and generation job. Retention/inspection alone must not enable Generate.

That installed-graph retention slice passes
`target/validation/prefix-registry-artifacts-zxqty2cv/accepted.json`, source
`prefix-registry-source-cyg4wek4`, including execution of the actual installed
candidate and both canonical combination checks. The same scoped batch passes
native/HF checks, strict affected and standalone lint, standalone consumers,
routes and ordinary single-root behavior. Its two leaf associations remain
separate from whole-workflow qualification.

The new whole-workflow owner is `bootstrap_measurement_approval.rs`, with closed
fixture/validation helpers and storage-only `bootstrap_workflow_evidence.rs`.
It reuses `reborn_workflow_review_evidence` rather than inventing association
proof for pure logic or another approval store. The private selection pins both
existing leaf approvals before Q1/behavior; complete reviewed packaged workflow
qualification remains distinct from target/model/design acceptance. Expected
cases include fit/nonfit, duplicate determinism, pure rejection before effects,
policy and asset failure, and original-template mismatch after both completed
native calls. Verify every actual step input/result and durable invocation, preserve
incomplete intents and prove readback never replays. This slice passes
`target/validation/prefix-registry-artifacts-3tx1j8xr/accepted.json` (frozen source
`prefix-registry-source-hl_i7tx8`): all eight closed cases, proof corruption and
no-replay checks, full-reference/HF execution, affected/standalone lint, consumers,
routes and ordinary single-root regression. This scopes qualification to the
packaged workflow; no normal Generate, model quality, native-library memory
ceiling or activation claim follows.

#### Retained model/design measurement jobs (2026-10-19)

The model-specific validation prerequisite now uses the existing immutable
`brassclaw_prefix_job_definitions` and ordinary durable turn admission with
`operation=measure`, `mode=target` (V171). This is an offline diagnostic operation,
not Generate, design acceptance, a provider-capability claim or serving activation.
The caller selects an immutable source generation, profile/model design proposal,
model artifact set and explicit complete chat probes/capacity assumptions. The
storage adapter resolves each existing owner and rejects a different profile,
namespace, source evidence-view or model binding. Original bytes stay retained;
no filesystem/latest-provider lookup or source trimming substitutes inputs.

The definition stores source/design/artifact identities and the exact probe hash;
large probe/reference/template data live in its existing immutable input capture.
A lost-response retry retains the first definition; altered requests conflict.
Before dispatch, reconstruct and compare the exact capture and revalidate the
packaged whole-workflow proof and selection. The qualified ten-step Python Recipe
owns template preparation, measurements, identity checks and capacity arithmetic.
The host only admits, binds, enforces live policy and verifies the actual receipt
and immutable two-invocation journal. Completion stays an internal result with
Unknown/unverified quality/cache state; it writes no design acceptance or serving
selection. The source generation can remain quarantined or capacity-blocked.

`POST /api/webchat/v2/prefixes/{name}/jobs/{job}` accepts a measurement request
with a caller-chosen canonical non-nil UUID; the existing GET of that URL reports
progress/result and existing run APIs own cancellation. Keep authenticated route,
retained scope, no-replay, incompatible-input and full-reference/HF regressions.
The native production-path regression now passes in
`target/validation/prefix-registry-artifacts-62egl9f8/native-registry-export.log`
(frozen source `prefix-registry-source-qxi5edxv`): authenticated POST, retained
subjects, exact full-reference/HF result, two actual dispatches, duplicate POST
without replay, conflicting-request rejection, no reply or design-acceptance
write and unchanged global root. The complete scoped batch passes
`target/validation/prefix-registry-artifacts-62egl9f8/accepted.json`, including
strict affected/standalone lint, standalone consumers, route contracts and the
ordinary matched/No-Match single-root regression. The supplied full
reference requires over 142k input tokens before output/additional allowances and
fails the explicit 131072 context assumption. No content was dropped or deployed.

The generation-history UI now includes an Offline measurement panel with explicit
proposal/model-file selection, context/input assumptions, exact chat probes,
correlated job status and result. A failed POST retains the UUID/request/revision
for retry; session storage keeps only the job UUID. New-request editing is
explicit and does not cancel the old job. Frontend state regressions check retry
identity, rejected numeric coercion, model/profile filtering and terminal polling;
this is separate from browser/embedded-product acceptance.

This does not complete accepted-design selection, normal model-aware Generate,
Sources/Redesign/Create, domain migration, complete library/release coverage,
native-library memory containment or target/cache/held-out answer-quality
acceptance.

#### Declared release-document package (2026-10-20)

`scripts/prefix/release-reference-inventory.json` uses the existing offline source
inventory schema and compiler adapter. It declares root Markdown (excluding the
private local test configuration), `docs/reborn/` Markdown and crate agent/spec
maps, plus the packaged authoring/design references. The initial 201-document
inventory selects 13 references; unreconciled material is archived as
`development-only` and the explicitly superseded storage document is `superseded`.
These are conservative source dispositions, not a statement that every subsystem
has been reviewed or that all release documentation/private workflows are covered.
The accompanying inventory procedure declares the exact scope and remaining gaps.

`target/validation/prefix-release-document-package-k_cyzsry/accepted.json` verifies
two identical offline snapshots through the existing adapter, all 201 original
byte hashes and the same tested source/catalogue snapshot. The result contains
218 sources including catalogue records. This establishes reproducible package
integrity, not installation uptake, content quality or serving readiness.

The installed follow-up extends the existing class-17 reference revision owner
with compile-time embedded declared documents, exact inventory membership and
byte verification. It creates no new document/provider/approval registry and
performs no checkout reads at runtime. Existing core records and compatibility
projections stay distinct: extra archived/review-pending documents must not enter
`reborn_docus` as validated retrieval rows. Preserve operator edits and old exact
revisions through the existing revision store. Core-only source capture retains
its explicit nine-document selection; the declared-release capture selects the
broader package, preserving dispositions and original bytes. Both reuse the same
source compiler and durable job path, with quarantined output and no activation.

Installed acceptance passes
`target/validation/prefix-registry-artifacts-mn40lntp/accepted.json` (frozen source
`prefix-registry-source-b85d9bsh`): stored/compiled equality, stable distinct core/
release selections, no additional legacy projections, complete 6,339,248-byte
release snapshot compilation through the actual global orchestrator, five actual
native calls, exact archive receipt and reconnect without replay. A release request
cannot reuse a core-only job UUID; its immutable request records the distinct
capture scope. Strict affected/standalone lint, consumers, routes, HF/core
measurement and ordinary single-root behavior also pass. This does not close
missing private-library export, remaining source reconciliation/domain migrations,
per-model design acceptance or the normal Generate/serving cutover.

#### Version-aware qualified prefix workflow sources (2026-10-20)

The next source-export scope reuses the existing catalogue source owner, lock,
repeatable-read verification and DTO. The existing `/1` ordinary export remains
monoversion. The `/2` transfer additionally includes the already-qualified source
compiler and complete measurement selections; source visibility neither advertises
a callable nor activates a Recipe. Their owners must verify their actual
qualification before exporting. Missing qualification fails the expanded export,
not a partial-success claim. Other private workflows remain outside this scope.

An expanded transfer can contain different retained revisions of one stable UUID
across independent approved selections. Key records by UUID/version, verify exact
class/checksum/bytes and retain each selection's own closed dependency combination.
Reject conflicting bytes for one revision, class changes for one UUID, mixed
versions within a selection, missing dependency sources and unused source records.
Do not synthesize a latest combination. Keep version-specific logical source paths
and complete original transfer evidence. The separately retained global root has
no component dependencies today; future versioned root dependencies require an
explicit selection contract rather than guessing their revisions.

The offline adapter accepts both declared transfer contracts and keeps transfer
integrity separate from approval. The full native batch passes in
`target/validation/prefix-registry-artifacts-yvy49l3l/accepted.json` (frozen source
`prefix-registry-source-dxoiu78a`): actual `/2` export, both qualified private
selections, complete 250-source/6,602,089-byte release compilation, five native
calls and reconnect without replay. HF/core measurement, affected/standalone
lint, consumers, routes and ordinary single-root behavior also pass. Eight offline
importer/compiler tests pass against that emitted transfer, with one tokenizer
case skipped. The observed catalogue has no multi-version UUID; focused parser
mutations and the separate collector regression cover revision conflicts without
claiming those mutations are approved executable combinations. The independent
collector check passes in
`target/validation/prefix-registry-artifacts-2zj4l9zc/accepted.json` (frozen source
`prefix-registry-source-eqd7oucr`), explicitly scoped to source-transfer records
only; it is not another production-path qualification. The declared-release capture uses the expanded
scope; the core-only capture and existing public ordinary-source route retain
their narrower contract. This does not complete all private-library coverage or
model/quality/activation acceptance.

#### Versioned trailing-inventory layout (2026-10-20)

`literal-reference/v3-trailing-inventory` reuses the exact-unit renderer and
`brassclaw-prefix-render-map/1`, retaining complete original bytes, line ranges,
content-derived evidence identities and per-source dispositions. It moves the
volatile source inventory after all complete reference cards. This keeps the
leading reference bytes unchanged for inventory-only edits without hiding the
changed source decision or its generation identity. It does not guarantee stable
leading tokens after content edits or changes to shared-unit grouping.

The existing Recipe-owned compiler stages, offline adapter/worker, closed model-
design proposal schema and archive validator recognize the explicit option.
Existing defaults and accepted designs are unchanged. Qualify both profiles with
independent closed rendering oracles, exercise the option through the actual
retained global source job, preserve v1/v2 byte equivalence, and compare actual
model/template token sequences before selecting this layout for a model. This is
one formatting prerequisite; it does not solve JSON-escaped embedded programs,
capacity, native-library memory bounds, model quality or serving activation.

Offline exact-token evidence is retained in
`target/validation/prefix-layout-token-stability-lg9kms6d/evidence.json`.
Using the retained full core source package, exact local tokenizer/template and
one plain chat, changing only an excluded source's disposition reason preserves
311 leading tokens under v2 versus 140,773 under v3. Total input counts are the
same for both layouts (141,768 before, 141,774 after). Original evidence/maps stay
identical and v2 rendering equals the retained original context. This is one
metadata-mutation/tokenization observation, not inference, provider-envelope/cache
qualification, held-out answer quality or a claim that the full reference fits the
explicit 131072 context assumption.

#### Progressive stable-sharing layout (2026-10-20)

`literal-reference/v4-stable-sharing` uses the same exact original/range map and
trailing inventory. Its first occurrence of each complete unit remains literal;
later occurrences point to that original source/range. Unlike v2/v3, discovering
a later duplicate does not change how the earlier occurrence is displayed. No
near-duplicate matching, summary or clipping is introduced. Canonical source order
still matters: earlier insertions and content edits can change earlier tokens.

An offline regression appends a complete exact duplicate of a mandatory guide.
All earlier reference bytes stay identical under v4, whereas v3 changes earlier
sharing labels. Every mapped source, including the appended duplicate, reconstructs
byte-for-byte from its original units. The shared compiler, offline worker,
archive/design contracts and independent closed fixtures recognize the new option;
existing view semantics/defaults remain unchanged. Bootstrap now has twelve closed
cases, checked by exact case names rather than the obsolete integration count of
eight. V3's batch `prefix-registry-artifacts-e0tug_1k` completed source jobs and
closed oracles but failed that stale count assertion; preserve it as failed batch
evidence. A successor must pass the full required native checks before claiming
qualification. These layouts do not establish accepted design, context fit,
provider/cache behavior, source quality or model accuracy.

The full successor batch passes in
`target/validation/prefix-registry-artifacts-273hlb17/accepted.json` (frozen source
`prefix-registry-source-99sk8rmh`): all twelve closed qualification cases, both
profiles/four independent rendering oracles, actual v4 global source job, complete
250-source/6,613,880-byte declared-release job with five calls and no replay,
retained measurement/API/HF checks, strict affected/standalone lint, standalone
consumers, route contracts and the ordinary single-root regression. The source
jobs stay quarantined; installed profile defaults remain v2. This covers the
frozen integration, not subsequent concurrent provider/model changes.

The retained offline update comparison is
`target/validation/prefix-layout-update-token-stability-4ik5nu12/evidence.json`.
It uses the exact same retained core source and local tokenizer/template, one plain
chat and three explicitly experimental mutations (reason-only, appended complete
exact duplicate, late-guide newline). Earlier v2/v3 rendering remains byte-identical
to the previous renderer for all cases. Token-prefix observations are:

| Layout | Reason-only edit | Appended later duplicate | Late-guide newline |
| --- | ---: | ---: | ---: |
| v2 | 311 | 1,282 | 80,584 |
| v3 | 140,773 | 189 | 79,430 |
| v4 | 140,346 | 140,164 | 79,003 |

V4's unchanged full input uses 141,341 tokens versus 141,768 for v2/v3; the lower
late-guide token position reflects this shorter earlier rendering, not a cache
regression at a different source. Appending the duplicate uses 144,881 tokens in
v4 versus 148,078 in v2/v3. All exceed the explicit 131072 context assumption;
these are complete offline token observations, not approved content changes,
serving hits, inference quality or evidence that model capacity was increased.

#### Native Oniguruma allocation accounting (2026-10-20)

The fixed encoding backend still uses tokenizers 0.23.2 with Onig. The local
`onig_sys` 69.9.3 fork (embedded header: Oniguruma 6.9.10) adds an explicit allocator
feature used by the encoding crate. Its embedded C malloc/calloc/realloc/free
calls enter Rust's existing global allocator through checked, aligned private
allocation headers. `monty_worker` already installs `LimitedAllocator`; this adds
no second budget owner, monitoring loop or alternative encoding engine. Reallocation
retains the original allocation's ownership across scopes. A system/dynamic
Oniguruma override fails the build instead of bypassing these calls.

`target/validation/prefix-registry-artifacts-h9i6gguf/accepted.json` (frozen source
`prefix-registry-source-eg9b7k7n`) passes the actual shim build and layout/overflow/
zeroing/reallocation tests. The existing isolated allocator probe calls real
Oniguruma C region allocation/reallocation/free through the safe Onig wrapper:
VM charges survive growth outside the original scope, free refunds the original
owner, and a 128 MiB C allocation request under the existing 1 MiB limit produces
the existing worker OOM exit with no successful output. This is native allocation-
accounting evidence, not a simulated counter or total process-residency limit.

The fresh shared-code batch passes in
`target/validation/prefix-registry-artifacts-9ulny9c1/accepted.json` (frozen source
`prefix-registry-source-us77oeu_`), including concurrent provider/model changes as
captured: strict affected/standalone lint, actual full-token/HF checks, complete
source jobs and twelve-case workflow qualification, durable measurement/API/no-replay,
standalone consumers and real C allocator probe, routes and ordinary single-root
behavior. This includes the successor safety documentation; a later build-script
whitespace-only formatting edit leaves its allocator mappings unchanged. Keep native memory activation prerequisites open:
allocator metadata/retained arenas, stack and other native/runtime residency still
require the documented reserve and platform containment/qualification. This patch
does not establish an OS RSS ceiling, cache validity, prefix acceptance or answer
quality. Preserve generated upstream-binding warnings separately; do not suppress
them or swap the regex backend to hide them.

#### Shared Sempai source corpus (2026-10-20)

Release inventory revision 2 extends the existing class-17 source owner with 69
archived originals: all 61 files of the pinned upstream Monty v1.0.0 package and
its original manifest, plus current compatibility/teaching source and receipt
files and observed legacy validation/reviewer/transport references. The inventory
now declares 270 originals. All additions are `development-only`; the thirteen
selected reference rows and core-only source scope stay unchanged. This is source
retention before profile-specific compatibility/content review, not Sempai
compiler support, current worked-example approval or a new persona/schema.

The same installation owner verifies complete package membership, original hashes,
canonical paths and commit-bound upstream URLs against the embedded originals.
The original manifest is retained; upstream SDK definitions/host examples are not
promoted into BrassClaw callables. Negative integrity cases cover changed original
bytes, forged hashes/URLs, path traversal, invalid flags, missing files and duplicate
entries. No runtime checkout discovery, remote fetch, parallel document registry,
source execution or extra validated legacy retrieval rows are introduced.

`target/validation/prefix-sempai-source-archive-dli38g6q/accepted.json` verifies two
identical offline snapshots through the existing adapter, all 270 original byte
hashes, all 61 upstream file/URL pins and a source-only v4 build retaining thirteen
evidence cards. The source package is
`0c01bafbbccd68fe2e0081b9aeb634e2905f04e44ccb95f522db69392feffe7c`.
This is document-archive evidence only. Native storage/capture and forged-manifest
rejection subsequently pass in
`target/validation/prefix-registry-artifacts-oq8cc7b9/accepted.json` (frozen source
`prefix-registry-source-62an8ku2`): complete 319-source/8,499,556-byte release job,
five native calls, reconnect without replay, exact archived bytes/dispositions and
no additional validated legacy rows. The full twelve-case/encoding/measurement/
API batch, affected/standalone lint, consumers, real C allocator check, routes and
ordinary single-root regression also pass.

A documentation-only successor now explicitly separates host embedding, preload
definitions, invocation bodies and Recipe metadata. It also corrects the former
claim that every prefix compiler runs in host Python: installed compilation uses
Recipe-owned Monty stages. Frame/shape limits are adapter/revision observations;
removed allocation-count APIs do not authorize silently deleting legacy settings.
Its exact updated archive is separately verified by
`target/validation/prefix-sempai-source-archive-qqtskksz/accepted.json`, source
package `b70b09f04e0eec96533b1ec2553ae46273e484ca2e57ad897316de07315e0293`.
This successor's text is not an installation-uptake claim for the preceding binary.

The old worked-example source/manifest/receipt hashes remain consistent. Its 29
examples and stated Linux-CPython/offline-design evidence are preserved, not
re-signed as current Monty/preload/constructor or decoder approval. Bind response
serialization to the actual supplied consumer schema, not a global summary-first
or summary-last heuristic. Sempai source selection and contextual presentation,
current constructor/decoder alignment, accepted model design, prefix generation/
activation and held-out model behavior remain open work in this phase.

#### Context-aware source presentation (native source-only validation passed)

`literal-reference/v5-contextual-sharing` extends the same Recipe-owned renderer.
Conservative source-family captions distinguish binding guides, component records,
upstream SDK/API or general references, BrassClaw authoring procedures, teaching
fixtures, implementation observations and catalogue metadata. Logical paths/kinds
are declarations, not authentication or API availability. Captions constrain
interpretation and confer no approval or Tool access; unknown families remain
reference data. Full host/preload/invocation/Recipe compatibility still needs review.

Exact-unit sharing is scoped by context as well as hash. Identical bytes under
SDK/API and ordinary-reference contexts stay independently literal, preserving
both contexts. Design `/2` retains complete included-source assignments and
`brassclaw-prefix-render-map/2` retains exact per-context origin groups. All
originals, original ranges and evidence identities remain unchanged; captions are
explicitly compiler metadata rather than original quotations. Storage validates
complete assignment/map/origin integrity without reimplementing Python policy.
Existing v1-v4 rendering semantics and installed defaults remain unchanged.

`target/validation/prefix-registry-artifacts-54gj9wx2/accepted.json` (frozen
`prefix-registry-source-q27bdp7a`) passes both profiles/five independent fixture renderings,
fourteen exact whole-workflow cases, re-signed missing/mismatched context and
cross-context-origin rejection, and the real pinned core snapshot compiled through
ordinary global admission with the explicit experimental v5 layout. No-replay,
complete source/HF/API, lint, standalone native-C allocation, routes and global-root
checks also pass. The documentation successor above is included in this capture.
This option does not
complete Sempai source selection, per-model design acceptance, normal Generate,
serving/cache qualification or held-out model accuracy.

Offline exact-token evidence in
`target/validation/prefix-context-layout-token-stability-7pw731pl/evidence.json`
uses one retained source package, three explicit mutations and one plain chat.
V5 preserves 141,060 leading tokens for a reason-only edit and 140,878 for an
appended duplicate; its 142,055-token baseline exceeds the explicit 131,072
capacity assumption before output reserve. The duplicate intentionally changes
context, so V5 retains a separate literal and reaches 159,086 tokens. This is
correct contextual separation, not a regression to hide with cross-context sharing.
These observations establish token identity only, not inference/cache/quality.

#### Sempai installed source-only profile (native validation passed)

The same source compiler Recipe now accepts the `sempai` profile with v5 only.
The installation-owned declarative selection includes the complete nine mandatory
binding/authoring/compatibility guides and all pinned upstream Monty files marked
`model_reference` in the original manifest. Other upstream metadata, old persona/
transport/worked-example originals remain archived. The Prefix-Maker procedure is
excluded from the Sempai model view. No source text is summarized or silently
truncated. Sempai requires declared-release capture; core-only capture fails.

Submission/storage and offline target diagnostics reuse the existing owners.
The older standalone `sempai-compiler.py` and the separate development build worker
have not been replaced or falsely qualified by this installed-Recipe migration.
The new source-only profile does not select a reviewer persona, change the supplied
response schema, add model calls, accept a model design or activate a generation.
Offline complete-source evidence is retained in
`target/validation/prefix-sempai-source-profile-w37vwr4c/accepted.json`: 317
original records, 117 included references, all 58 upstream `model_reference`
files and two byte-identical compilations. One plain-chat exact HF probe measures
334,756 tokens, exceeding the explicit 131,072 serving-capacity assumption and
the retained config's 262,144 position limit before the 8,192 output reserve.
The tokenizer's overlength warning is preserved. This proves an incompatible
complete source design, not readiness; no source is removed to hide that result.
The offline transfer lacks the installed capture's two provenance envelopes, so
its 317 records must not be represented as the 319-record native capture.

`target/validation/prefix-registry-artifacts-q9l8y7jj/accepted.json` (frozen
`prefix-registry-source-8p1uye2m`) passes independent exact rendering,
missing-guide/old-layout failure,
real retained-worker stages and a durable full-release Sempai job through the same
global owner, with all 319 originals, five calls, reconnect without replay and no
activation. Fifteen exact compiler qualification cases, new-layout API acceptance
and old-layout rejection, complete source/HF/measurement API checks, affected lint
and the ordinary single-root regression pass. The prior guard and obsolete-API-
fixture failures are retained in `prefix-registry-artifacts-kk07cs80/failure.json` and
`prefix-registry-artifacts-7wv295fz/failure.json`
under the same artifact directory family. This scoped successor reuses unchanged
route/standalone evidence; it does not claim those were rerun. Target capacity and
held-out reviewer behavior remain separate gates.

#### Source-check prerequisites (native HTTP/storage validated; admitted workflow pending)

The source-check Recipe needs the existing HTTP primitive, not a separate crawler
or a Rust source-check workflow. The old `pc-exec-http-get` still uses runtime source
substitution. `http_get_components.rs` prepares a typed preloadable successor and
an explicit bounded variant of the existing `http-get` identity, with one call to
`host.http`, fixed GET, exact byte/time inputs and complete sanitized response data.
The owner must resolve the existing Tool/ToolSkill/Skill/code/Recipe IDs; the draft
constructor allocates no parallel HTTP family. Merge/review other existing variants
and compatible bindings before any coherent activation. Nothing writes legacy rows
or grants approval from a `system` label.

The finite declared HTTP profile uses object-of-string request headers and explicitly
serialized text bodies. These are supported native profiles, not a claim that every
native input alternative has been migrated. This usage follows no redirects and
performs no hashing, availability decision or observation write. The source-check
Recipe must keep these operations separate. Non-2xx is data; overflow/timeout/policy
failure stops without automatic retry. HTTP 200 is not a content/knowledge verdict.
Pure `prefix_source_check_components.rs` drafts separately project complete text
and classify a response plus a separately supplied UTF-8 digest against an
independently retained original hash. Unknown identity/content changes, redirects,
server/rate failures and unsupported/ambiguous representations stay neutral; missing,
authentication and empty-content outcomes are unavailable. Only exact nonempty text
and expected-hash equality can produce available content, never knowledge approval.
The caller must establish actual response/digest provenance; these pure components
perform no fetch/hash/write themselves. Raw I/O/effect evidence remains separate.

`target/validation/prefix-registry-artifacts-w_ij7pyl/accepted.json` (frozen
`prefix-registry-source-xsg96z3a`) passes strict affected-library lint, the real
retained native identity/input/preload inspection and the pure Monty classification
matrix. The earlier failure in `7zai0t32` was a test assertion using the binding
step's key instead of the executable invocation key; its failed receipt is retained.
These checks perform no HTTP dispatch and establish no bootstrap approval,
activation, observation retention or Sources readiness.

The next draft composes the existing usages into an explicit diagnostic Recipe:
validate expected identity before network effects; HTTP ToolSkill/export; pure
complete-text batch projection; JSON ToolSkill/digest export; single-digest
validation; pure content classification. Each component step has one UUID, each
real dispatch has its own binding/executable pair, and inputs/results remain typed.
The constructor neither allocates a parallel HTTP/JSON family nor writes/activates
components. Expected hash data is not source provenance or knowledge approval.
The real retained-native loopback test and composed graph now pass in
`prefix-registry-artifacts-u4w40vr0/accepted.json`: complete Unicode content,
non-followed redirects, missing responses, body overflow and live policy blocking.
This supersedes the earlier dispatch-pending status, not its retained failures.

Remaining work is exact bootstrap/graph qualification, trusted observation
projection, admitted source jobs, qualified indexed-original integration and UI uptake.
Source checking remains unavailable until that complete path is qualified.

The registered-source storage prerequisite now has a draft implementation in
`builtin.prefix_source`, backed by the existing V143 registration/observation
tables. Its only operations are an exact immutable registration read and append
of diagnostic bytes as **unqualified** evidence. Pins include profile, source UUID,
revision and registration checksum; a registration checksum is not the referenced
content checksum. Append uses the real native invocation UUID, returns the same
receipt for identical retained data and conflicts on changed data. It grants no
availability, review, job completion or activation status. Native store transport
does not fetch, hash responses or choose workflow steps. Reusable Skill/export/
ToolSkill usages and separate canonical Recipes parse typed command data and call
this primitive once; construction is not bootstrap qualification. The factory
wires the backend, but no new eligible Skill/catalogue route is seeded or activated.
Real PostgreSQL immutability, duplicate/conflict, malformed input and inert claimed-
success tests, plus retained graph/preload tests, pass in the same u4w40vr0 receipt.
These do not approve or activate a source-check Recipe or make observations
authoritative availability facts.
This prerequisite covers registered sources only. Generation-original observations
must use their exact existing generation/artifact owner; do not force logical paths
into the registration foreign key or repeatedly read a full package once per row.

#### Registered-source diagnostic Recipe (closed native candidate validated)

`prefix_registered_source_components.rs` adds a `registered` variant to the existing
HTTP source-check Recipe. Its 16 explicit component steps reuse the canonical
source read, HTTP GET, UTF-8 digest and source observation usages. Each real call
has its own binding/executable pair; four pure Python exports parse, resolve,
serialize and return data. Rust constructs definitions; it adds no workflow Tool,
storage owner, queue, endpoint or approval path. Source bindings are copied from
the actual canonical Recipes, preserving host aliases independently of display
names. Existing direct diagnostics retain their original Steps/layout/invocations.

The only task input is canonical JSON containing the exact registration identity,
response byte limit and timeout. The native read supplies the URL. A caller cannot
substitute a URL, claimed response or expected content hash. Registration identity
and content identity are separate: this fresh-registration variant has no retained
content baseline, so a successful fetch remains neutral/unverified. HTTP 200 never
creates approved knowledge or an Available indicator. Unknown types/identities and
non-web registrations fail before network dispatch.

The diagnostic stores compact classification/response metadata and a complete-text
hash, with the native immutable observation receipt. It does not squeeze the whole
HTTP response into the existing observation-row cap. The native test processes a
complete response larger than that cap, verifies exact raw content and independently
recomputes its UTF-8 hash, then verifies exact stored diagnostic bytes/checksum.
Collection/archive/review of new content remains a separate workflow; this metadata
record does not make fetched content eligible for the next prefix build.

[Seven fresh Rust cases and strict checks](../validation/2026-10-25-prefix-registered-source-check.md)
cover retained candidate execution in actual Monty, native kernel HTTP/digest/source
handlers and real PostgreSQL, plus 404, wrong pin, overflow/live blocking without
follow-up observations, override/qualification rejection, existing direct
classification and storage immutability. These private test executions do not
provide ordinary durable job admission/effect settlement, trusted observation
projection, system-seed bootstrap, association approval, availability publication
or serving activation. Consume the shared owners for those remaining boundaries;
do not widen the closed source-compiler approval exception.

#### Native Sempai target measurement (native caller validation passed)

Extend the existing durable measurement/API acceptance helper across both base and
Sempai profiles. Carry the exact completed source-job IDs through the test, retain
both resulting packages without overwriting prior artifacts, and inspect the actual
recorded encoding arguments/answer. For Sempai, independently render/tokenize those
same full-reference requests with the retained Hugging Face artifacts. Verify exact
report/receipt identity, context/output/additional-input arithmetic, no truncation,
two dispatches, reconnect without replay and no design acceptance/activation.
This closes the current Sempai measurement-caller gap; it adds no new compiler,
model worker, provider request or parallel measurement service. Do not replace real
source generations with a small fixture or freeze expected content hashes/counts
inside the compiler. Geometry and model quality remain separate gates.

`target/validation/prefix-registry-artifacts-m5jf5tis/accepted.json` (frozen
`prefix-registry-source-owlrbe32`) passes that complete native source-profile scope.
The runner verified the entire identical source inventory, toolchain, lint evidence,
worker/test binary hashes and runtime oracle mirror before reusing the binaries
from `w_ij7pyl`; `build-reuse.json` retains the exact producer link. It then executed
the real PostgreSQL/global admission/API/encoding checks and ordinary matched/
No-Match single-root regression. This was executable evidence reuse, not a claimed
new compilation or a substitute result.

Both declared-release jobs retain 319 sources and complete with five native calls,
reconnect without replay and no activation. Base measurement observes a largest
effective probe of 141,978 tokens; the complete v5 Sempai generation observes
345,129. Each durable measurement performs two actual native operations and matches
independent Hugging Face rendering/tokenization of its actual dispatched arguments.
The full packages are exported create-only in that receipt directory. The original
tokenizer overlength/model-class warnings are preserved. These are observations
for this exact source/model-artifact selection, not compiler constants or claims
about current model weights, inference quality, GPU cache reuse or restored state.
The complete Sempai reference exceeds both the explicit 131,072 serving capacity
assumption and the retained config's 262,144 position limit before output reserve.
Do not truncate or activate it: a reviewed model-specific teaching/source design
and renewed measurement/quality qualification remain prerequisites.

#### Read-only runtime discovery — 2026-10-21

The documented model endpoint currently advertises Ornith with `max_model_len`
131,072 and `/version` reports vLLM 0.31.0. Filtered read-only process arguments
show FP8 KV dtype, prefix/chunked prefill, 8,192 batched tokens, one sequence and
`--no-disable-hybrid-kv-cache-manager`; LMCache arguments include chunk size 1,056,
`--separate-object-groups` and reap timeout zero. `/server_info?config_format=json`
returns 404, so no structured effective-cache configuration was obtained.
The filtered observation is retained at `target/validation/prefix-runtime-observation-iktd1hcc/observation.json`.
No deployment files or service/container/environment configuration were changed;
the inspection script was streamed, not installed, and no inference request was made. These are reported limits and arguments, not actual
weight/serving identity, cache layout, transfer correctness or performance evidence.
The complete 334,756-token offline Sempai reference therefore exceeds the currently
advertised request capacity. Imported model-config observations must remain
separate; these development observations are not silently written into a model
specification, accepted design or reusable source package.

#### Retained model-configuration inspector (native API validation passed)

The existing artifact-detail owner now projects bounded declared configuration
fields by JSON pointer, with exact model/artifact/config identities. Its existing
GET API carries the additive observation; Model Designs exposes an explicit
read-only inspection action. No new provider registry, model call, workflow Tool,
source store or acceptance writer is introduced. Missing/null are distinct.
Per-layer, position and weight-quantization declarations stay separate from null
actual-weight, deployed-context, attention-KV and recurrent-state observations.
Malformed/ambiguous/oversized configurations remain retained and visibly
uninterpreted. Keep this data-only observation separate from accepted model-design
and effective-runtime/cache qualification. The same `q9l8y7jj` receipt passes
original file identity, unknown runtime facts, invalid/duplicate/oversized inputs
and authenticated real-artifact API reads. Six actual frontend-source state tests
plus syntax checks pass in `target/validation/prefix-model-config-ui-kk0ebg8y/accepted.json`. They cover explicit identity-bound reads,
stale/unmounted response isolation, duplicate clicks and literal declaration display.
The final UI preserves the server's observation JSON without browser numeric
reserialization and uses a generic invalid-observation error. These final UI-only
corrections are newer than the native receipt's embedded frontend and have separate
source hashes in the UI receipt. Browser/product deployment acceptance remains
open; no model/design/cache acceptance follows.

### Phase C1 — Sempai reference, review packet and component proposals

Current status: profile/source seed and v5 source-only jobs/measurements are
implemented; a fitted accepted reviewer composite and production knowledge/contract
uptake remain open. Follow remaining steps 5–9/13/16/18 and the shared v3.5 owners.

Complete Sempai compiler/source/procedure qualification from the existing seeds.
Extend the shared compiler adapters and Recipe jobs to generate the target-specific
Sempai prefix and any composite explicitly required by its coverage contract. Wire the typed review packet and actual reviewer persona/
response schema through the existing interceptor ports, forensic storage, gateway
and proposal sink. Preserve input identity, target-provider metadata and effect
history. Keep pre-call review and post-response learning separate and avoid a
parallel agent loop. Use the existing class-specific draft/Q1/Q2 paths; implement
missing association/constructor/provenance support under Gate A1 rather than hiding
it in a prompt. Production reviewer activation also requires Phase C2 acceptance.

Acceptance: six seeded profile definitions; complete source/procedure provenance;
deterministic Sempai rendering with no HA leakage; capacity failure without dropped
guides; and dedicated Sempai consumer selection independent of Kohai. Through the
production caller, test unchanged prompt echo, safe targeted prompt adjustment,
malformed packets, missing provider capabilities, structured tool-call/result IDs,
injection attempts, secret redaction and no loss of user constraints/effect history.
Use representative anonymized conversations to propose a reusable Recipe and
associated usages with actual supported schemas/draft identities. Verify Q1 failure,
pending human Q2, rejection, exact-combination approval and coherent activation;
no direct validated writes/self-approval or unsupported fields. Verify old tasks
retain their selection, failed steps never replay as Tier 2, the production reviewer
schema is not replaced by citation-benchmark JSON, and Tier-0 tasks gain no hidden
Sempai calls. Prompt improvement/proposal correctness needs actual case evaluation,
not only token/cache measurements.
Port the whole-artifact validator through the real lossless sink; include missing/
null/default/boolean/list/recursive/bounds/helper/result cases and malformed/duplicate
Recipe layouts, invented Tools/UUIDs and summary/payload disagreement. Add independent
transfer cases with different domains/field names/bounds, hold them out of teaching,
and report first-pass separately from repair acceptance. Verify grammar/schema
fingerprints and host-authorized edit constraints without forcing every Recipe to
one variant. A rejected candidate stays rejected, even if its summary claims success.
Test artifact-first/summary-last decoding through the actual production consumer,
including rejection of old reviewer order even when a canonical sorted fingerprint
matches. Regenerate incompatible planner grammars; test exact quote provenance,
missing requirements, erroneous artifact classification and invented defaults/branches.
Verify invocation audits cannot force effectful loading or module-level result
assignment into preload definitions. Validate empty proposal arrays against the
requested artifact, not against compatibility arrays or optimistic summaries.
The six v25 protocol tests and source/compiler checks are standalone development
evidence; production sink/loader acceptance and fresh first-pass/transfer/cache-path
model qualification remain prerequisites, not inferred improvements.

Validate current Skill preload definitions and their invocation artifacts separately
through the selected production Monty host/ABI: effect-free loading, dependency
order/export resolution, isolated mutable state, complete returned results and
retained task selections. Upstream Monty pure-logic probes do not certify that loader,
kernel binding or private transport. Include current MCP discovery/ordinary-chat
contracts as source-backed knowledge checks. Require correct answers that Skills
need no dedicated Recipe, reference workflows are not ownership and known Skills
are not automatically MCP-callable. These checks do not invent an external Python
execution API or treating documented targets as implemented support. Include correct
answers with unrelated valid citation IDs and wrong-but-verbatim quotes as failures;
source resolution, quotation fidelity and semantic support remain separate gates.

### Phase C2 — Minimum provider resolver before activation

Current status: provider/specification/settings/measurement foundations exist,
including the one mutable hard-limit owner. Named/composite consumer resolution,
activation and unconditional runtime guards are not accepted. Remaining steps
5–6/10–12/19–20 own this cutover. Both prefix and non-prefix limits persist and
apply with budgeting off; Kohai checks both Sempai and final answer envelopes as
specified in §5.3, with zero provider calls on a pre-dispatch size violation.

Wire immutable prefix selections into Kohai and Sempai independently through
[simplified Phase 0a](../../simplified_v3.md#phase-0a--ground-truth-komponentenverträge-implementieren) / [simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben) task snapshot/admission contracts.
The shared owner implements task/child/continuation pinning and execution fencing;
this phase adds prefix generation/design/provider references and freshness checks
to that existing boundary. Implement only prefix-specific atomic selection,
request-owned insertion, duplicate-prefix prevention, default/specific prefix
selection and capacity checks for explicitly required composites; no second task manifest/admission service.
Include the role-specific Sempai prefix and any explicitly required composite,
using its separately resolved provider/model pair and that pair's shared design.
Enforce the request-owned/default-template contract above and retain historical
task/artifact evidence without enabling template installation or a server-owned
fallback. New dispatch may not depend on a removed global role override. Implement activation/rollback and minimal warm/reuse
operations through the existing facade and provider seams before enabling their UI.
Consume the DB-only provider contract and migration/reload implementation from
[simplified §10.1](../../simplified_v3.md#101-umsetzung-und-entfernung) before this phase's model-facing production gate. That
owner implements definition readers, CLI/management lifecycle and notification
publication; this phase consumes its revision stream and tests prefix selection/
design/guard uptake. No prefix registry, lifecycle resolver or copied provider
configuration is allowed; §5.3's prefix/model settings extend the existing owner.

Acceptance: production model requests use the selected generation exactly once;
source activation races block/queue new tasks rather than serving outdated context;
ordinary replacement preserves compatible old continuations; revocation or an
incompatible changed provider format is classified. The selected role artifact
cannot be substituted, duplicated or leaked into another role's request. Test restart,
concurrent activation, both consumers, incompatible/capacity-blocked composites,
effective injection/capacity evidence and adapter-specific exact token observations
where available. Require a second-request native reuse observation only for adapters
declaring observable reuse; adapters without telemetry must pass inference/pinning
acceptance with Cache reuse unverified. `/tokenize` alone does not satisfy
completion-time generation enforcement. Gate A1 and Phase C
must pass before enabling this production resolver.
Reuse [simplified §10.1](../../simplified_v3.md#101-umsetzung-und-entfernung) DB-only/management/CLI lifecycle receipts; add only
the prefix-consumer matrix here: provider URL/model/context changes invalidate or
retain selections as specified; deactivation (including the last provider) prevents
new model dispatch without deleting generation history; reactivation does not
restore a selection implicitly. Verify prefix continuation/guard behavior and
DB failure versus Unconfigured through the shared provider owner, while ordinary
management and qualified provider-free work remain usable.
Test a provider without cache telemetry and ensure activation/inference remain
available while unsupported warm/verification actions and observations are honest.
For targets serving hybrid external hits, section 9.1 is an acceptance prerequisite,
not work deferred to Phase E: exercise cold/native/external/restored-native paths,
plain and constrained decoding, expected content and representative semantic probes.
Keep failed cache experiments out of model scores. Check bad page mapping, absent
recurrent checkpoints, changed backend/chunk/source build and previously corrupted
native state; known failures block the target. Qualify the actual configured path
without new random salts concealing it. Package upgrades invalidate backport-bound
receipts. No service/package/deployment mutation occurs from a WebUI build/warm job.

### Phase C3 — Prefix-Maker, Redesign and custom profile creation

Current status: offline Prefix-Maker/compiler metadata, model specifications, local
artifact import/inspection and unaccepted declarative proposal storage exist.
Remaining steps 7/14–15 qualify runtime Prefix-Maker and implement accepted designs,
profile/consumer bindings and assisted design/custom creation.

Build on those existing package/model/proposal owners. Complete reviewed bootstrap
designs, runtime identities/evidence, acceptance/bindings and retention contracts;
add missing forward schema only where needed. Implement compatible shared-engine
options and exact acceptance contracts.
Author the Tier-1 Redesign/Create/Generate model design Recipes,
reusing existing provider, export, validation and proposal primitives. Production
calls require Gate A1, Phase C/C2, usable Prefix-Maker and the lossless proposal path;
do not activate unsupported workflows or route failures through Tier 2.

Acceptance: bootstrap without provider/self-reference; exact Prefix-Maker injection
once; distinct design-provider/target identities; pinned input and schema references;
confirmation bound to the actual request; raw proposals and full diffs preserved;
real trial builds; invalid/unknown options rejected; mandatory coverage preserved;
capacity/missing architecture surfaced; no operational deployment changes or arbitrary
compiler execution. Check malformed/provider-failed/cancelled/duplicate requests,
recovery after a possible provider effect, stale proposal acceptance, custom-profile
identity conflicts, human rejection and Prefix-Maker self-redesign with the old
approved generation. New design selection never activates a generation or bypasses
authored Q1/human Q2. Redesign must collect current approved content/architecture and
produce a complete updated generation; acceptance publishes the exact validated
candidate together with its design evidence, without another manual Generate.
Identical approved model-specific design and source inputs reproduce output; new
approved components change content while keeping the selected design contract.

Exercise two real registered models with different tokenizer/template/context and
architecture capabilities through the production DB/provider path. Verify separate
formatting/capacity designs and profile bindings, Unknown model/cache facts, reusable
compatible defaults and incompatible overrides. Test design-provider versus target
model separation, Kohai/Sempai provider/model changes independently, same-name/different
weights, template/context/backend edits, missing design/prefix readiness, staged switch,
concurrent acceptance/switch and restart persistence. No stale incompatible generation
serves new work, no hidden LLM design call occurs on Generate, and old tasks/history
retain their references. Cache-only changes requalify runtime evidence; do not rebuild
identical reference content needlessly. Seeded/custom model designs survive upgrades.

### Phase D — WebUI vertical

Current status: six rows, generation history/import/details, Sources inventory/
registration, model/artifact/config inspection, measurement status and shared hard-
limit editor exist. Check Sources/model-design generation remain unavailable;
ordinary Generate is still legacy base-only. Remaining steps 3/5/8–17 complete
enabled actions and real browser/product acceptance without recreating this UI.

Extend the existing facade/HTTP/descriptor/runtime/frontend wiring together. Show seeded and registered custom profiles,
durable progress, generation details and independent Activate/Warm states. Update
translations and keyboard/accessibility behavior. No browser-triggered arbitrary
script, source path or environment execution.
Include composite job admission/detail controls and parent/target dependency status
within the domain/reviewer rows; activation selects a published eligible composite explicitly.
Show the Sempai row's source quality, reviewer target/readiness and proposal
validation guidance. Keep review enablement separate from reference generation.
Show Prefix-Maker readiness, Redesign beside each Generate button, the confirmation
popup, proposal/diff review and Accept redesign controls. Add the Create Prefix tab and
custom-profile draft/registration lifecycle through the same facade/API/runtime path.
Add Model Designs selection/generation/list/review/binding controls and affected
consumer readiness on provider/model changes; expose persisted historical designs too.
Implement section 8.1's Sources button/tab, manifest-backed inventory, durable source
registration/check workflows and per-source availability projections. Wire existing
fetch/artifact/store adapters through Recipes; implement genuinely missing support
in its owning layer before exposing enabled actions. Source checks/registration use
accepted Gate A1/C paths and do not require a design provider; Redesign-based collection
still follows Phase C3, while offline approved-snapshot Generate remains available.
Generate requires Gate A1 and Phase C acceptance. Activate/Rollback/Warm requires
Phase C2 acceptance for the specific registered provider target. Until then, show
disabled actions with their missing prerequisite; enforce the same gate server-side.
Redesign/Create/model-design generation/acceptance require the accepted Phase C3 path; their absence must
not disable the model-free Generate workflow.

Wire the new generation prompts through `provider-card.js` and the existing
`useProviderManagementActions.js::handleUse` / `handleUseSempai` and their
provider-state setters, not a parallel provider writer. Their current success
messages/boolean are not sufficient target receipts: consume the actual committed
selection identity and refresh the canonical current snapshot before confirming a
base job. Preserve desired/effective provider acknowledgement; a persisted Use is
not runtime uptake or prefix readiness. Reuse the design-system `Modal` with
OK/Cancel, keyboard/Escape/focus handling and localized target/profile text, plus
existing Prefix job controls for progress/reconnect and later activation.
Qualify both the missing-specific-prefix dialog and the post-Use base dialog,
including Cancel with zero generation/model calls, duplicate OK, failed Use,
changed target before OK, an existing compatible base, unavailable design/worker,
capacity/activation failure and no original-message replay.

Acceptance: real composed-route tests and browser checks for first Generate,
Regenerate, refresh/reconnect, cancellation, per-profile failures, unavailable
worker/provider, rate limiting, stale source and compatible/incompatible activation.
Navigating away must not lose a job. A failed build must not replace an active prefix.
Cover generated-parent versus ready-composite state, composite build/rebuild,
dependency/capacity failures and a valid provider lacking cache telemetry.
Cover the section 1.1 completion states through the browser and composed API: one
click builds eligible parents/composite, repeated clicks deduplicate, prerequisite
and quality/cache failures stay distinct, advanced details expose omitted-unit
reasons and receipts, and Generate never changes the effective provider selection.
Exercise popup cancel/Escape (zero admission/provider calls), explicit confirmation,
changed prerequisites, double-submit/reconnect, provider unavailable, stale/rejected
proposals, new-profile creation and custom profiles surviving reseed. Verify keyboard
focus and accessible dialog/tab labels. No button or popup implicitly accepts a model
proposal, approves a component or activates a prefix. Verify explicit registered-target selection
including models not active for a role,
separate design-provider selection, generated model-design history after reload,
validation/acceptance and explicit bindings, switch readiness for both consumers,
and Redesign returning a complete current-content generation rather than design only.
Test Sources on every seeded/custom row, inherited composite sources and pre-generation
registrations, immediate inventory plus per-source checks on opening, green/red/neutral
accessible states, check-again/dedup/reconnect and observation persistence. Exercise
real supported HTTP/content and DB/artifact adapters: redirects, soft 404/login/empty
pages, missing retained artifacts, timeout/rate-limit/auth/policy denial and full-versus-
partial content identity. Show an intact archive despite upstream failure. Save a new
source, reject credentials/unsupported schemes/conflicting duplicates, verify restart
persistence and Pending review, then prove inclusion in the next eligible Redesign or
Generate without changing a running build. Test Needs redesign, capacity/exclusion
reasons and no hidden LLM/source execution or automatic active-generation replacement.

### Phase E — Extended cache and answer quality

Current status: standalone validators/benchmarks and historical cache experiments
exist; production citation/configuration integration and current held-out quality/
cache claims remain unaccepted. Remaining steps 12–13/16/18/21 close the applicable
gates. Minimum actual injection/cache correctness remains a C2 activation gate.

Build on the accepted Phase C2 resolver. Add richer cache observations and optional
external persistence adapters. Integrate citation validation in actual answer
routes and retain domain-specific config checks; minimum provider correctness is
already a prerequisite for Phase D activation, not work deferred to this phase.
Mandatory source/artifact/teaching/configuration checks are part of the build gates
above. This phase broadens measured model/answer quality; it does not postpone those
checks or reinterpret a compiled artifact as proven expertise. Quality evaluation
is an explicit separate Recipe/job using a quarantined immutable candidate and an
accepted injection adapter, never an implicit compiler call or production activation.
It must not require an already active candidate to test that candidate or create
a first-build dependency cycle. No provider means model quality is Unmeasured.

Acceptance: exact effective `/tokenize` or equivalent adapter evidence; no duplicate
automatic/client prefix; pinned selection across concurrent changes and continuations;
context-capacity failures without silent omissions; cold/warm native measurements
only where supported, and separate external reload proof for
adapters claiming it. Unsupported cache observations remain Unverified rather than
blocking otherwise valid providers. Test source-backed component/Recipe
questions, security maintenance, HA YAML/MQTT/Modbus and tomedo API/version cases,
including unavailable functionality, fake schemas, backdated billing versions,
forged/unrelated citations and configuration that parses but is invalid. Compare
accuracy as well as cache reuse; latency/cache hits do not establish expertise.

## 12. Validation scope, rollout and stopping rule

This planning change requires source/diff/link review only. No compiler build,
database migration, provider deployment, prefix activation or remote service change
is performed by creating this document.

Implementation uses focused checks under the development policy: real PostgreSQL
for storage/job behavior, real Monty/Recipe dispatch for execution claims, affected
facade/route/architecture tests for wiring, and browser verification for the UI.
Use `LOCAL_TEST_ENV.md` before any remote/provider test. Preserve model deployments
and reuse passing evidence until relevant changes invalidate it.

Roll out behind the existing base-store compatibility resolver. Import verified
domain bundles without rebuilding them, but label unverified legacy artifacts.
Keep old generations/selections and rollback references until active tasks and
continuations release them, including generation-specific provider routing bindings.
Compatibility reads do not authorize new tasks to consume outdated/unverified
context under the final resolver. Gate A1 precedes production Recipe jobs; Phase C2
production acceptance precedes enabled activation and resolver cutover. Remove the
obsolete formatter/store path only after all consumers are
migrated. Update current docs, seeds and checksums together through supported paths.

### Coordinated upgrade and recovery

The system-wide procedure is implemented once by [simplified Phase 6](../../simplified_v3.md#phase-6--datenbank--dateisystem--und-secret-migration) and [simplified Phase 8](../../simplified_v3.md#phase-8--cutover-und-entfernung-des-legacy-modells),
with runtime drain/fencing/reconciliation and Ready-before-producers from
[simplified Phase 3a](../../simplified_v3.md#phase-3a--globalen-monty-beim-boot-starten-und-dauerhaft-betreiben). Prefix joins that procedure; it does not implement another
upgrade command, migration scheduler, backup/restore service, runtime supervisor
or shutdown/startup sequence. Use the shared owner's actual execution receipts
for these mechanisms, supplemented by the prefix integration cases below.

Prefix supplies:

1. **Inventory and additive data migration.** Register legacy bundles, named
   generations/selections/designs, sources, provider routing references, jobs and
   compiler/Tool artifacts with the shared dry-run and forward-migration owner.
   Identify scope-key collisions and unknown provenance explicitly; preserve
   stable IDs, overrides, exact approvals and references used by retained tasks.
   Do not rewrite applied migrations or silently choose an ambiguous owner.
2. **Retention and reconciliation adapters.** Supply required immutable prefix
   artifacts, operation receipts and reference checks to the shared drain/backup/
   restore procedure. Product job stages cannot stand in for run/effect records;
   unknown build/publication outcomes are reconciled through the original shared
   invocation, never automatically resubmitted by a prefix recovery worker.
3. **Post-restore and cutover cases.** Verify complete originals, accepted designs,
   compiler/native artifacts, generation IDs, old task/prefix references and both
   consumer selections. Supply authenticated Prefix facade/Recipe/guard smoke
   cases and interrupted-migration/restore scenarios to the one Phase-7/8 matrix.
   Remove only prefix-specific legacy consumers after their replacements and
   reference retention pass. Shared provider-file/scope/auth removal remains
   with simplified's owners; prefix supplies its affected-consumer checks.
4. **Product rollback.** Prefix-generation rollback changes an eligible immutable
   selection through §9's compatibility/freshness checks. An incompatible binary/
   database rollback instead invokes the shared Phase-8 restore procedure and
   compatible artifacts; a prefix pointer change never rolls back the schema or VM.

Keep failure diagnosis and authenticated management recoverable under the shared
runtime contract. Embedded-PG shutdown order, exclusive migration ownership and
secret/key handling are implemented/accepted by simplified, not redefined here.
External vLLM/Compose deployment remains operator-owned. Prefix-specific acceptance
cannot close a missing shared upgrade/recovery gate, and shared runtime acceptance
cannot establish prefix fit, source correctness or serving readiness.

Completion means all six seeded profiles and approved custom profiles can be generated/regenerated through the UI,
base-prefix inputs contain the complete approved current docs/components, source and
generation history survive failure, model calls use the intended immutable context
exactly once, and quality/cache status is supported by real receipts. Nothing in
this plan authorizes claiming simplified-v3 runtime gaps are solved by prompt text.
Each profile also exposes confirmed provider-assisted Redesign, and Create Prefix
can produce a reviewed custom profile/design. These paths preserve existing designs
and active selections until their separate acceptance/activation operations.
Redesign produces a complete updated generation with current approved knowledge;
accepted per-model designs, size/formatting rules and profile bindings persist in DB.
Model Designs lists generated revisions and can generate a design for an active
registered model. Provider/Kohai/Sempai model changes use compatible designs/generations
or report their explicit readiness prerequisite rather than serving mismatched context.

### Final one-click acceptance matrix

| Operator scenario | Required result/evidence |
| --- | --- |
| Generate a profile for a registered consumer/target | One durable operation builds/reuses eligible parents and validates the complete composite; exact source/target references and downloadable receipts are visible. |
| Generate without a provider | A valid source artifact or explicit missing-local-artifact prerequisite; model quality/cache readiness remain Unmeasured/Unverified. |
| Edit model limits from Inference, Prefix settings or CLI | One provider/model record and revisioned endpoint; all surfaces converge on readback, stale writes conflict, unrelated values survive, and budgeting off/restart does not remove the hard limits. |
| Review then send an answer with budgeting off | Kohai resolves both targets; oversized complete Sempai input makes zero review calls, and an edited prompt that overflows only after history/tools/prefix assembly makes zero final calls. Exact transformed partition/context/output checks remain unconditional. |
| Format then correct then insert | Cover distinct provider/model pairs and one pair shared by generator/Kohai/Sempai. Shared pairs use the same saved accepted design revision with separately generated role prefixes; distinct pairs use their respective designs. Assert Kohai formats only the dynamic Orchestrator prompt before review; Sempai receives that formatted prompt as review data, uses its own correctly generated reviewer prefix/design and returns the final dynamic prompt; Kohai then inserts the intended exact prefix once before it. Assert no second formatter call or return normalization after review. Record event order and original/formatted/review-return/prefix/assembled payload hashes. Reject missing/incompatible design bindings before dispatch. Disabled/unchanged review and resumed/tool continuations retain this ordering and pinned identities. |
| Populate and manually edit pair design settings | Capture available provider metadata with provenance and fill unavailable format/capability facts manually. Edit either hard input ceiling from Inference and Prefix/model-design views, including inactive targets; verify one provider map entry/revision and immediate shared-query refresh, stale-CAS conflict handling and no role-specific duplicate values. Discovery cannot overwrite operator settings silently. Show affected artifact compatibility and required regeneration/revalidation without mutating retained prefix bytes or changing provider server configuration. |
| Regenerate after changing a model's size limits | Accepted deterministic budget rules select a valid supported view with full provenance; unsupported bands require Redesign and mandatory overflow blocks. No implicit model call, silent source loss or metadata churn inside the stable prefix. |
| Repeat or reconnect during Generate | Durable deduplication, progress and retained child references; no repeated confirmed publication or automatic activation. |
| Change sources/target, cancel, or exceed context | Classified stale/blocked/cancelled outcome, preserved originals and last-good selection; no silent partial knowledge or unreviewed replacements. |
| Rebuild on another supported worker/path | Identical canonical content, evidence IDs, rendered text and leading tokens from identical pinned inputs. |
| Validate knowledge and teaching | Required coverage, complete evidence units, independently specified behavior checks and transitive validator receipts; first-pass/repair/transfer results remain separate. |
| Activate with enabled external caching | Accepted exact injection plus qualified real restore correctness; known-corrupt or incompatible paths block activation without changing the deployment. |
| Inspect an active generation | Desired/effective selection, freshness, capacity, model quality and cache observations are distinct and traceable; retained tasks keep their pinned revisions. |
| Warm a selected target | One retained correlated provider operation with cancellation/accounting, exact generation/target identity and an honest reuse observation; no activation, deployment change or replay of a confirmed warm request. Unsupported telemetry remains Unverified. |
| Cancel or confirm Redesign | Cancel makes no job/provider call; confirmation admits one pinned Tier-1 task, refreshes current approved content/architecture and returns a model-specific proposal plus complete updated candidate, without replacing the existing design or active prefix. |
| Accept a redesign | Exact reviewed model-specific design and its validated updated generation are recorded/published without another Generate click; later Generate reuses the accepted design and production activation remains separate. |
| Create a new prefix | Same-section Create Prefix tab yields a validated draft and human-accepted registered custom profile; it appears with Generate/Redesign and survives reseeding. |
| Redesign Prefix-Maker itself | Previous approved fresh generation reviews a quarantined successor; bootstrap and approval never depend on that unvalidated successor. |
| Generate model design | An explicitly selected registered target, including a not-active role model, and a separate eligible design provider yield a persisted validated proposal for the pair's shared prefix/prompt format and canonical-limit references, with separate profile-content rule bindings. Generation changes no active selection; runtime joining preserves exact prefix text. Acceptance and profile binding are explicit. |
| Change Kohai/Sempai provider or model | Resolve exact compatible model/profile/consumer design and generation, or report Needs design/generation/Incompatible; no old mismatched prefix or hidden redesign call. |
| Reopen Model Designs | Generated and historical model designs, evidence, acceptance and bindings survive reload/restart and reseeding. |
| Generate a usable formatting design | Provider facts and researched model/default-template/backend references are retained with immutable provenance. The accepted design contains exact supported compiler/prompt/join instructions and independently verified construction cases, not just labels/limits. Unknown effective template/cache facts remain explicit; no generic format or architecture guessing, server edits or hidden browsing during Generate. |
| Missing/incompatible selected prefix dialog | Show exact target/profile and OK/Cancel. OK admits the existing supported generation; Cancel starts zero jobs/model calls. No base substitute, hidden redesign or original-message replay. Missing design/coverage support shows the actual prerequisite. |
| Use then base generation dialog | After each successful Use, including Sempai, show the base generation/regeneration question for the committed/current pair. Cancel preserves selection and exposes readiness; OK generates base without replacing the role prefix. Failed/stale Use responses do not trigger obsolete jobs. |
| Dialog progress/reconnect/readiness | Repeated OK/reconnect shares the original durable job. Show generation/validation/publication/activation separately; bind exact compatible artifact before first dispatch and never resume an attempted call by replay. |
| No specific prefix selected | Match or No-Match without a specific Recipe/Orchestrator/role prefix selects compatible base for the target pair; no Tier-2-only restriction. A specific role/domain selection does not implicitly append base. |
| Specific selection missing/conflicting | Missing or incompatible explicitly selected prefix blocks the affected call; a declared selection conflict is reported, never silently resolved by base or a new priority. |
| Separate shared design from role content | Same pair uses one accepted format design with distinct profile rules/generations; changing one profile's knowledge does not duplicate or silently modify the shared pair format or canonical hard limits. |
| Open Sources | Manifest-backed used sources and pending registrations appear; bounded per-source checks report green/red/neutral with reasons and timestamps, without changing archived evidence or active prefixes. |
| Save a source | Versioned registration persists, is collected/reviewed for the next eligible Redesign/Generate, and remains visibly unused until included in a validated generation. |

Run this matrix through the production facade, composed ingress, Recipe worker and
browser path. Script-only success or a screenshot of buttons does not close the plan.

## Kohai final prefix stage and MCP provider lifecycle

Kohai first formats only the dynamic Orchestrator prompt with the stored accepted
target-pair design. If Sempai is connected/enabled, it optimizes/corrects that
already formatted prompt using its own generated reviewer prefix/design for its
own call and returns the final dynamic prompt to Kohai. Kohai then adds the
intended finished prefix unchanged exactly once before that final dynamic prompt,
without another formatting stage. Use base only
when no specific Recipe/Orchestrator/role prefix is selected. Kohai then obtains the coherent listing derived from approved
mcp-call-skill-recipes, establishes the provider's supported MCP connection and
advertises it immediately before sending the prompt. Do not reconnect by starting
a server or compile a prefix merely to establish that connection. The one MCP
server remains running during idle/disconnected periods. On the complete model
answer, clean up only this request's connection/advertisement; use the same cleanup
for failure/cancellation with owned in-flight calls reconciled. Streaming progress
is not completion. Refresh server discovery on startup/restart and qualified
Skill/Recipe activation; keep each advertised request contract and normal chat
execution snapshot coherent. MCP calls remain new ordinary chats, not direct
Monty/Rust invocations. See [the exact lifecycle contract](monty.composition.md#recipe-derived-discovery-and-kohai-owned-provider-connections).

This is lifecycle adapter planning only. Verify actual provider MCP configuration
and normal chat APIs before implementation; no endpoint/SDK field or runtime
acceptance is inferred from these instructions.


### 2026-10-24 — Budget-dependent reference-selection prerequisite

Implemented the reusable pure selection stages and
`select-prefix-design-by-measurement` Recipe constructor in
`prefix_design_components.rs` / `prefix_design_selection.py`. The existing JSON
and original-template encoding usages remain the only native operations.
Validated size bands, mandatory dependency closure, optional priorities and exact
whole-candidate counts replace estimated/additive packing. Complete originals,
unit ranges, ancestor context and omission reasons remain available. Unsupported
bands fail; no fitting candidate returns Capacity blocked without an empty
success artifact. All candidates are checked because token counts need not be
monotonic. Set membership and buffered rendering avoid avoidable quadratic work.

[The scoped validation record](../validation/2026-10-24-prefix-budget-selection.md)
covers eight Python regressions and two actual Monty/Rust-tokenizer/retained-graph
integration tests. Targeted Clippy completes with the existing parallel catalogue
warning; strict lint remains blocked by that warning and is not called passed.
Earlier failures and exact accepted source/executable identities are retained.

This closes the pure reference-selection building block, not steps 7/8 as a
whole. The resolved unit plan is distinct from the immutable accepted generation
rules. Their model/profile/consumer and authoritative-source bindings,
qualification and ordinary Generate integration remain open. Raw reference fit
is not semantic composite/chat fit, quality, cache restore or serving activation.
No provider/model hard-limit store, inference service or deployment changed.


#### Fixed rules with automatically refreshed content — successor

The same reusable component now also resolves immutable
`brassclaw-prefix-generation-rules/1` against current captured sources. Rules
select exact paths, namespace prefixes and complete-unit first lines; there is
no provider call or runtime code generation. Required coverage is checked against
the mandatory dependency closure in every supported band. Added components under
a declared namespace are included without editing rules, hashes or code. Missing
selectors and newly uncovered required units fail before hashing/encoding.
The original rules, source snapshot and resolved unit plan have separate exact
identities. A content refresh therefore need not be Redesign.

The [successor acceptance](../validation/2026-10-24-prefix-budget-selection.md#fixed-rulecurrent-content-resolution-successor)
records eleven final-source Python cases and two actual integration cases,
including new-component resolution in Monty. The selected-target lint completes
with the unrelated catalogue warning. Reuse this resolver in step 7 and the
ordinary Generate Recipe; do not build another competing rules engine. Accepted
model/profile/consumer bindings, independent authoritative coverage checks and
bootstrap qualification are still required. No WebUI or serving activation is
claimed by these candidate-stage checks.

## Open K3-2 follow-up from K4 — Use, model design, prefix Q1/Q2 and retention

**New open follow-up; not implemented by this document patch.** Existing completed
sections and original acceptance criteria remain unchanged. This section carries
only the relevant future-consumer delta from [Exact K3-2](Exact_V3_definition_update_at_K3-2.md) and the
[shared selection/lifecycle contract](../reborn/model-token-limits.md#open-k3-2-successor--retained-prefix-selection-and-prefix-free-construction-workflows).
It does not reopen K0–K3, replace a current owner's work or create a parallel
implementation. Original sections are read references; no code activation is claimed.

- Replace the unconditional Use→Base behavior for future consumers: compatible
  accepted design offers Base generation; otherwise navigate to design settings.
  Admit original-ID-bound configuration discovery with optional prefix-free B1/B2,
  showing context and editable canonical ceilings before any design Generate.
- Reuse the original artifact inspector and registered protocol/formatter adapters.
  Prefer deterministic design construction; model interpretation is optional and
  must use actual sources. Prefix-Maker-prefix readiness is no current prerequisite.
- Extend existing RecipeModelPrefix/resolver with Default, Selected and deliberate
  PrefixFreeWorkflow through capture, native capability, response evidence, recursive
  child/review execution and the final request guard. Exact §5.2 names the current
  concrete source owners; Inline/FinalBoundary is insertion timing, not omission.
- Existing Pair/Profile/Generation acceptance, target package and CAS selection
  owners are reused. Generate includes its complete Prefix-Q1; only explicit human
  Q2 activates the published candidate. No mandatory review-navigation click or
  repeated identical mechanical Q1 at approval/activation.
- Future GC must address target/source payloads and their real FK/audit/living
  references. The two-predecessor rule does not authorize deleting an active task's
  artefact. The operator has explicitly confirmed reference protection until the
  live references are released; collect eligible older payloads automatically then.
- Keep ordinary model serving unchanged until its coherent successor is qualified.
  No hidden template/service edit, second guard, copied mutable limit store or
  automatic replay. New functionality has no acceptance merely from this text.


### K3-2 successor acceptance matrix — not yet executed

The complete cross-owner cases remain in [Exact §14](Exact_V3_definition_update_at_K3-2.md#14-abnahmematrix-für-die-spätere-implementierung).
This additional matrix qualifies the new consumer delta; it neither edits the
original matrix nor broadens previous scoped acceptance.

| Trigger / case | Required observation through the actual owner |
| --- | --- |
| First Use without accepted design | Committed original Use remains; one durable discovery workflow; design dialog navigates to the exact pair without requiring Base or Prefix-Maker. |
| Use with compatible accepted design | Base Generate/Regenerate dialog; no implicit Redesign or activation. Incompatible design takes the design branch. |
| Use failure, read failure, late result, reconnect, cancel | Failed Use starts no discovery; read error is not “design missing”; obsolete results do not affect the new pair; reconnect retains original IDs without model replay. Dialog Cancel preserves Use and starts no Base/design job. Discovery has its own visible cancellation. |
| Concurrent Kohai/Sempai selection | Separate original role/pair correlation; no global last-selection modal or implicit reviewer replacement. |
| Native metadata/artifacts suffice | No B1/B2 model call. Unknown limits stay unknown; canonical manual values remain intact. Context and editable prefix/prompt ceilings appear before Generate. |
| B1/B2 needed | Exact fixed instructions, captured source/option packet, prefix-free native dispatch and deterministic strict parsing; no repair inference. Format, correlation, evidence and import eligibility remain distinct. |
| Malformed or ungrounded answer | Reject duplicate/extra keys, wrong types/echoes, foreign source/option, unresolved pointers or false quotes. Text-only semantics remain a candidate; no canonical value/acceptance write. |
| Alternate design executor, inactive/offline target | Explicit route captured before effects; default is the target pair. Reload preserves the explicit executor. Children never acquire current global bindings to replace inherited absence. |
| Default and explicit Selected | Default pins its resolved generation; Selected resolves exactly or fails. Neither missing metadata, resolver error nor incompatible selection becomes prefix-free. |
| Qualified PrefixFreeWorkflow | No Base/reviewer/catalogue/Prefix-Maker insertion and no accepted-prefix lookup; original workflow/instruction/route binding reaches required recursive child/review calls. |
| Independent normal chat/review at the same time | Keeps its own normal selected prefix; no shared client mode or context residue after cancellation. Legacy records keep their original meaning. |
| Actual prefix-free SDK body | Same mandatory final guard and actual complete request encoding; fixed instructions, sources, history, tools and framing count as prompt input. Missing measurement or hard-limit violation prevents submission. |
| Design Generate and acceptance | Deterministic construction first; real required gates enable direct approval. Failed gates keep it disabled with errors; readable View/Review remains optional. No production activation. |
| Profile Generate/Regenerate/Redesign | All required checks are generation-internal Prefix-Q1. Publish the exact qualified artifact; no second Q1 job, repair inference, automatic activation or repeated completed effect. |
| Human Prefix-Q2 | Explicit activation CAS uses the same published qualification and live compatibility checks; stale/failed Q1 cannot activate. Component/Recipe/code approval is unaffected. |
| Regeneration of an archive group | Only two distinct successful predecessors plus protected older live references. Failed candidates do not consume predecessor slots. Shared source/target payloads and actual FKs remain valid. |
| Final reference release / concurrent capture | Reconcile through the existing retain/GC owner; collect eligible old payloads without breaking new pins, resumable tasks or immutable audit hashes. No hidden trigger disable or permanent payload protection solely from a terminal audit link. |
| No installation/serving shortcut | Qualified successor seeds and original-package integrity are required; docs, pure tests, imported source-only packages or contained candidate tests do not establish serving activation. |


### Original request-output producer — scoped prerequisite accepted, 2026-10-30

The [BUILD-005 report](../validation/2026-10-30-original-model-output.md) and
[exact receipt](../validation/2026-10-30-original-model-output.json) establish the
bounded output-setting prerequisite described in [Exact §17.8](Exact_V3_definition_update_at_K3-2.md).
The finite prompt's optional positive u32 setting reaches the SDK through original
PG invocation arguments and existing response custody. Missing preserves a valid
native fallback; invalid explicit values fail. Enabled limits reject rather than
clamp. Cost estimates and accepted Prefix defaults remain distinct from the
original setting. The pure bootstrap candidate requires its typed input; its I1
admitted producer is still open.

The global Recipe fixture proves retained arguments and SDK transport with 37/79
and discretionary budgets disabled, using its existing static-route fixture.
The separate captured-provider fixture proves native PG/custody/SDK and production
final-counter rejection. These fixtures do not jointly prove a qualified Free
workflow or required child/review closure. A separate Sempai review can precede
primary rejection. The receipt records 70 fresh plus four unchanged reused tests,
0 ignored, final strict checks and the failed earlier attempts without rewriting
their outcomes.

This closes only that prerequisite. The successor matrix above remains open for
Use/discovery, successful Free qualification and its original evidence/required
review handoffs, design generation/acceptance, Prefix-Q1/Q2 and reference-safe GC.
Strict raw-answer parsing is a separate pending primitive/usage qualification;
legacy JSON parsing gains no strictness claim here. No historical completed section,
serving activation or whole-plan acceptance is changed. Future work uses Root's
final fifteen-source receipt, current source preconditions and the
[permanent central build owner](../development-build-queue.md), not the private
proposal's earlier bytes or a new Cargo window.


### Strict JSON primitive — scoped prerequisite accepted, 2026-10-30

The [BUILD-006 report](../validation/2026-10-30-native-strict-json.md) and
[receipt](../validation/2026-10-30-native-strict-json.json) establish the separate
native prerequisite still pending in the preceding progress record; see
[Exact §17.9](Exact_V3_definition_update_at_K3-2.md). The existing `builtin.json`
now offers explicit `parse_strict` through its existing bounded common reader.
The original v1 schema/operations remain unchanged; a disjoint v2 branch requires
actual text and no extra arguments, with one current-schema selection owner.

Eleven fresh tests, 0 ignored, formatting and affected connected/default strict
checks pass. Actual native registration/retention/live policy and captured bounds
are exercised, including nested/escaped duplicate keys and legacy preservation.
The initial test-only descriptor/manifest projection error remains a failed
attempt; its repair uses existing native schema/identity APIs. The new native
Tool artifact is verified separately from the unchanged worker.

This supplies no activated strict usage/association, complete B1/B2 workflow,
Free root/child/review qualification or approval/import permission. Later combined
qualification must use the current native artifact; BUILD-005 images retain their
earlier scope. The open successor matrix, shared admission/review/serving work,
Prefix-Q1/Q2 and reference-safe retention remain open. No completed historical
section or whole-plan/platform acceptance is rewritten.


### Internal model ResultOnly admission — scoped prerequisite accepted, 2026-10-30

The [BUILD-004 report](../validation/2026-10-30-internal-model-admission.md) and
[corrected receipt](../validation/2026-10-30-internal-model-admission.json), SHA-256
`682c7c94f06d43dd9eeff1d6fd347f38ba741858dbb0b756c1886d7db001e370`,
establish thirty-source/V191 runtime acceptance with twelve distinct cases,
zero ignored, and connected/default strict checks; see [Exact §17.10](Exact_V3_definition_update_at_K3-2.md).
Original claimed source determines ResultOnly independently of the preserved
model prohibition for deterministic internal work. Existing host/admission/driver/
result-evidence paths consume it without a new executor or public permission flag.
The real global-service case exercises seven retained model usages, actual SDK
protocol transport, original PG arguments/three-stage custody and durable exit
application without assistant publication or No-Match learning.

V191 preserves unknown historical origins as NULL and immutable original identity.
Only its derived projection is excluded from the old BEFORE-trigger whole-record
comparison; catalogue/model/phase/settlement guards remain. Actual old guards,
start/no-op/settlement and negative origins are tested. The existing bootstrap
admission accepts an optional actual HostFactory and deliberately remains unstarted
until its original task references are captured; failed host preparation closes
the original effect-free claim. It supplies no independent contained capture pipeline.

The receipt corrects an overbroad unchanged-dependency claim: archived worker227a
was actually tested, but Host Resources depends on changed PG migration inputs.
This is older-worker evidence, not a fresh V191 worker build. BUILD-007 centrally
rebuilds that worker and checks the integrated standalone strict JSON usage; this
next batch has no passing claim here. Original bytes and failed attempts remain.
Shared contained capture/native binding and required mid-turn Sempai custody stay
with their existing owners. No sealed successful Free proof, B1/B2 approval,
activation, serving qualification, prefix Q1/Q2 or reference-safe GC is established.
Historical monty_control capture consumers and the open successor matrix still need
their current migrations; no completed historical section is rewritten.


### Standalone strict JSON usage — scoped prerequisite accepted, 2026-10-31

The [BUILD-007 report](../validation/2026-10-31-standalone-strict-json.md) and
[receipt](../validation/2026-10-31-standalone-strict-json.json), SHA-256
`9af1b0643393e838c4daf43282fbb633f3b0187e6155f97e8414451ced8cdaaf`,
close the usage execution check pending above; see [Exact §17.11](Exact_V3_definition_update_at_K3-2.md).
Three current sources, 38 fresh distinct cases, zero ignored, formatting and
connected/default strict checks pass. One reusable Skill calls existing parse_strict
once and returns explicitly serialized text. It needs no dedicated Recipe; the
contained test consumer independently checks original input, native payload and
export text, real negative native errors and live denial without new approval rows.
The global ResultOnly/SDK regression and 34 actual service cases also pass.

The canonical worker was freshly built from current sources and remains byte-identical
to227a. This establishes current build identity; older BUILD-004 observations and
the corrected dependency limitation remain historical facts. Unapproved drafts/test
execution are not association/pure/workflow qualification, activation, Free proof,
model-design or serving acceptance. Shared contained capture/native binding, the
real closed B1/B2 subject/evidence owner and required mid-turn Sempai custody stay
open, as do the successor matrix, prefix Q1/Q2 and reference-safe retention.
Pre-readiness/development contained workers must not become parallel live VMs;
ordinary live work stays on the existing global service. No historical completed
section or whole-plan acceptance is rewritten.


### Shared native task-model capture — scoped handoff accepted, 2026-10-31

The [BUILD-008 report](../validation/2026-10-31-shared-model-capture.md) and
[receipt](../validation/2026-10-31-shared-model-capture.json), SHA-256
`b323f61288d73b5cc2b267da3499a27849e0cf23d8315a02ea7b700dd4afb81f`,
establish the bounded shared capture handoff pending above; see
[Exact §17.12](Exact_V3_definition_update_at_K3-2.md). Six current Composition
sources, nine fresh distinct cases, zero ignored, format/compilation and
connected/default strict checks pass. One existing capture/metadata owner now
serves the global factory and pre-readiness/development contained model path.
Actual original host/admission/source and persisted root lineage are checked
before capture/native binding, then the unstarted claim is rechecked after awaits.
No public lineage field, duplicate snapshot query or replacement admission.

Real PG/kernel/SDK cases cover capture-before-binding, foreign admission/re-entry
rejection, global identity/ResultOnly, ordinary/default/selected routing, inherited
children and pure observers. Original failed Run1 diagnostics remain recorded;
repairs keep the original admission Arc, safe error classification and private
persisted-parent inspector. The fresh BUILD-007 worker is reused unchanged.

This supplies no sealed successful/contained Free proof, component approval,
activation, serving acceptance or full I1/I2 closure. The actual B1/B2 subject,
independent oracle/evidence reader/native carrier and separate mid-turn Sempai
custody remain open with their owners, along with the successor matrix, prefix
Q1/Q2 and reference-safe retention. The loopback fixture proves protocol behavior,
not inference quality. Contained execution stays pre-readiness/development only;
ordinary live jobs retain the existing global service. No completed historical
section or whole-plan acceptance is rewritten.


### Native PRE-primary Sempai custody — scoped prerequisite accepted, 2026-10-31

The [BUILD-011 report](../validation/2026-10-31-related-model-custody.md) and
[receipt](../validation/2026-10-31-related-model-custody.json), SHA-256
`67060dd4fe05937006ffd8b5872f73fa89953a3a080279a59d4e09916b8f4472`,
establish eleven-source/V192 acceptance with ten fresh distinct positive cases,
zero ignored in Run2, format/compilation and connected/default/neutral strict checks;
see [Exact §17.13](Exact_V3_definition_update_at_K3-2.md). V192 is actually assigned,
not a placeholder. Related native work retains the original invocation, actual
host request and selected reviewer route/Prefix, distinct work/fence and unchanged
wire/provider/loop payloads. It cannot overwrite Kohai or reuse post-turn identities.
Reviewer output is independent of explicit Kohai37/79 and accounting estimates.

Original complete requests remain immutable in PG with only checked digests in RAM.
The existing owner retains related pending observations through primary failure or
cancellation and repeatedly reconciles them without resubmission; durable payload
buffers are released. Actual SDK/PG tests exercise all three stages, route retention,
failed review/no primary send, original-write failure and recovery after real
cancellation. Related records have no parent/store Arc cycle. The actually rebuilt
worker remains227a; unchanged Run2 inputs/images are explicitly verified.

The earlier broad selector's ignored explicit live-inference gate stays recorded
and unverified; precise positive Run2 coverage does not claim inference quality or
Serving. This is the current PRE-primary review, not POST-primary preparation or
post-turn integration. Confirmed-unsubmitted review-budget reconciliation remains
a separate open BUILD-012 accounting repair; unknown effects retain their holds.
Actual sealed Free/required-edge qualification, B1/B2 subjects, full I1/I2 and the
open successor matrix, prefix Q1/Q2 and reference-safe retention stay open. No
historical completed section or whole-plan/platform acceptance is rewritten.


### Native unsubmitted review accounting — scoped repair accepted, 2026-10-31

The [BUILD-012 report](../validation/2026-10-31-review-unsubmitted.md) and
[receipt](../validation/2026-10-31-review-unsubmitted.json), SHA-256
`1ae4bb26e00871bcdf0319ae03bc3320f9851e683a8442d1376da78305aa9380`,
close the specific accounting repair pending above; see
[Exact §17.14](Exact_V3_definition_update_at_K3-2.md). Two sources, five fresh
distinct cases, zero ignored, compilation/format and connected/default strict
checks pass. Actual confirmed-unsubmitted evidence is retained before gateway
error conversion and reaches the existing accountant's supported reconciliation.
Unknown effects retain holds; an error category is not evidence of no submission.

Actual Governor/PG/native/SDK cases distinguish a preparation BudgetExceeded
without any SDK request (separate primary/review reservations released) from a
BudgetExceeded after an actual Length response (only unsubmitted primary released;
review reservation and original provider evidence retained). No fabricated spend,
new ledger or retry. Existing custody/output/protocol regressions pass; unchanged
BUILD-011 worker inputs are verified for reuse.

This is actual caller accounting with the in-memory governor, not durable governor
crash recovery, billing verification, remote inference or Free/Serving approval.
Original technical discovery/B1/B2 packet sources, sealed workflow qualification,
full I1/I2 and the open successor matrix, prefix Q1/Q2 and reference-safe retention
remain open. No completed historical section or whole-plan acceptance is rewritten.


### Model-host compatibility in its actual owner — scoped acceptance, 2026-10-31

The [BUILD-009 report](../validation/2026-10-31-model-host-compatibility.md) and
[receipt](../validation/2026-10-31-model-host-compatibility.json), SHA-256
`ebf79a0837fe78aef49dbb52d9ec22f3255bf2afff6e32b4946e4b4ba9e652ae`,
accept the current Composition-owned compatibility successor; see
[Exact §17.15](Exact_V3_definition_update_at_K3-2.md). Final Run6 covers 31 changed/
moved/deleted paths, fresh compilation and 24 distinct cases, zero ignored.
All 22 original cases remain, with one native reply-negative case and one separate
model-metadata case. Format, CI YAML/scope, Composition lib/tests strict and isolated
Monty workspace all-targets strict pass. The actual 248-path reviewed-root hash is
`06e246dd76f24a30ca5b1ed85a17b884620c4a767d7f2c6b73d56e1696b29419`.

Tests/fixtures now consume their actual Composition owner and current prepare/bind
contracts. The unchanged capture observation record/parser is shared once via
`task_model_capture_record.rs`; it remains data, separate from the opaque native
capture. The unchanged task-model capture query belongs to the actual admission
lifecycle. No duplicate parser, dummy caller, fallback, new suppression or Free/
approval extension is introduced. Exact worker227a inputs and artifact reuse are
verified. Failed runs remain preserved. All-targets Clippy does not certify a
rerun of the entire isolated runtime suite; recording model fixtures do not prove
live inference, final Free/Serving or deployment.

Original Use/ModelSetup transaction interfaces remain private/unintegrated.
General Pure/review qualification consumes the existing validator Steps3/8–10 and
V149/V150 original submission/preparation owners, never an expanded generation
exception or second review queue. Original executor configuration/model/credential
identity and live dispatch checks remain required. Full I1/I2, Use/design flow,
prefix Q1/Q2, reference-safe retention, durable recovery and BUILD-010 remain open.
Historical completed sections and old evidence stay unchanged. BUILD-009 source
freeze has ended; permanent central Cargo ownership remains in force.


### Bootstrap Python test entrypoint — scoped source correction accepted, 2026-10-31

BUILD-013 moves the existing unittest.main below both classes, preserving all
23 test bodies. The actual repository command
`python3 -B scripts/prefix/test_bootstrap_answer.py` passes23/0skipped, including
the eight Prefill cases previously bypassed by direct script execution. See
[Exact §17.16](Exact_V3_definition_update_at_K3-2.md),
[build queue](../development-build-queue.md) and [handoff](../development-handoff.md).
This is source-only Python evidence; no Cargo/product/native/Q1/Pure/Free change.
Original BUILD-009 evidence and all prior plan bytes remain unchanged. Actual
review/pure approval and Original Use/ModelSetup integration stay open.


### Review model selection, A/S target and offline oracle — scoped successors, 2026-10-31

[Exact §17.17–17.19](Exact_V3_definition_update_at_K3-2.md) records the following
bounded successors; previous bytes/evidence stay unchanged:

- BUILD-014 [receipt](../validation/2026-10-31-review-model-selection.json), SHA-256
  `347be24f5ae84c02360a84769d44d50ff6cbaf45a7b951fe6153248dd299ec6e`:
  five sources, compilation, nine distinct tests/0ignored and format/connected/
  default strict. One existing model-input serializer feeds Engine and V150.
  Original Recipe/variant comparison preserves absent versus explicit Default,
  Selected profiles and Free declarations; checksum-correct forgeries fail and
  originals restore after successors. Preparation metadata grants no dispatch,
  Q1/Pure/Free or Use-bootstrap authority.
- BUILD-015 [receipt](../validation/2026-10-31-validation-target-documents.json), SHA-256
  `b78e1f69bf5a92f4fac3da2242f75c8d9a1d905a9b583ddca41e77fab2cc6a10`:
  21 text paths, compilation, three native cases/0ignored and connected strict,
  actual baseline/reference/deployment with319 release sources. Integrated target:
  authored A uses own function approvals/shared Skill-code subject and proven
  compatible successors without unchanged-consumer reapproval; shipped S retains
  controlled exact usage/prerequisite qualification without installed Q2. Actual
  joint-subject/writer/compatibility/publication and independent S support remain
  open in existing owners. Do not build an obsolete generic A closure-reapproval
  engine or expand the narrow generation exception. The receipt retains its
  original Python warning; only the next successor addresses it.
- BUILD-016/017 [receipt](../validation/2026-10-31-model-oracle-runtime.json), SHA-256
  `dc2da85e0b2d9299eb4bcfa83cdc97dff80c454511ac50931c40c0f6a83f0f90`:
  four explicitly run offline Rust/HF cases plus the actual native/global base/
  Sempai consumer, five distinct/0ignored. Exact unchanged compilation/strict
  reused; isolated pinned test interpreter and actual HF backend count without
  padding/truncation. Original model/template/tokenizer/context remain unchanged;
  oversized measurements still fail the native context guard. No warning filter,
  production renderer, server change, inference or platform deployment claim.
  Two factual model-input metadata texts are included without new API/authority.

Prefix-owned B1/B2/Packet/schema/Goldenbytes are unchanged. Workflow-scoped Free,
original executor/configuration/credential/finalguard requirements, complete I1/I2,
independent pre-prefix Design release, prefix Q1/Q2, reference-safe retention and
recovery remain open. These scoped documents/tests activate none of them. Source
freeze ended; central Cargo ownership remains permanent. No historical section is
rewritten or whole-plan/platform acceptance claimed.


### Joint function subject — retention successor accepted, 2026-10-31

BUILD-018 [report](../validation/2026-10-31-joint-function-subject.md) and
[receipt](../validation/2026-10-31-joint-function-subject.json), SHA-256
`6ad7eea42eb374a09d05c48055f794045467cca477bb715ca4fee13f5c2e06a5`,
accept the existing review owner's strict submission/3 and actual V193 upgrade.
Sixteen sources, fresh compilation,15 distinct native/0ignored,7 UI/0skipped,
format and connected strict pass. See [Exact §17.20](Exact_V3_definition_update_at_K3-2.md).
Optional function_partner retains two changed Skill/code roots or one changed root
with exact unchanged context, per-root base/head CAS and one V149 intent. Staging
is atomic; second-root conflict rolls back the first. Original /1-/2 bytes remain.
Typed subject/2 preserves both sources/bases; legacy single-class preparation
rejects the new joint subject at Engine and Store, preventing partial validation.

This is original subject/context retention, not actual complete function rules,
Q1, human Q2, atomic own approvals, compatibility publication or S/Pure/Free
qualification. Context retention itself grants no partner approval. Next work
uses the existing Validator Steps3/8–10 owner and real run/attempt/interpreter/
settlement, without another queue/subject/store or generation exception. Full
Use/ModelSetup/design/prefix lifecycle remains open. B1/B2/Packet/Golden and all
historical plan bytes remain unchanged. Batch freeze ended; Root retains Cargo.
