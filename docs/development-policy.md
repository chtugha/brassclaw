# Development reasoning and validation policy

> **Policy reviewed — 2026-10-24:** use [the combined implementation and ownership
> index](implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


## Scope and precedence

This is the authoritative policy for development iteration and validation.
Read it before changing behavior. Root and crate guides provide architecture,
contracts and commands; their command lists are not a checklist to run after
every edit. This policy supersedes older blanket development test/lint rules.
Subsystem acceptance criteria, kernel enforcement, Q1 and human Q2 remain
binding. The root disk-space checks, cleanup thresholds and NVMe target rules
are unchanged. Development reasoning and tests do not grant component approval
or permission to activate or deploy a component.

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
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](../skills.md#preloadable-function-interface-binding-v3-target).
Selected retained loader/store/runner support is implemented; this contract
does not certify every workflow or the complete migration. See
[the current implementation status](implementation-status.md).
Legacy step-body implementations must be migrated to this interface before
being accepted as updated v3 implementations.


## Recipe architecture checks

For component changes, read the four ground-truth guides:
[recipe.md](../recipe.md) for workflows and persisted IBS syntax,
[skills.md](../skills.md) for usages, associations, recursive contracts and retries,
[tools.md](../tools.md) for primitives, retained implementations and live policy,
and [toolskills.md](../toolskills.md) for binding preparation. These component
contracts and the binding targets in [simplified_v3.md](../simplified_v3.md)
take precedence over historical examples and older subsystem descriptions of
scoped authorization, operation approvals or per-chat orchestrator lifecycles.

Verify that each component step references exactly one stable component UUID,
and that internal PythonCode composition, typed bindings and result handoffs
follow the four guides. Matching and IBS/composition must use one
consistent activated, approved catalogue generation. Pin the matched Recipe
UUID/revision/checksum, embedded variant identity, exact `step_link`, selected
step order, input layout, all component/transitive dependency revisions/checksums,
association approval identifiers and actual retained Tool implementations with
their adapter/ABI contracts. BuildInstruction remains ephemeral; preserve its
selection through the supported durable task snapshot/continuation contract.
Running/suspended tasks, children, retries and resumption retain that selection;
replacement does not invalidate it or permit a fresh latest lookup. Missing or
incompatible newest active combinations fail before effects, without silent
downgrade or Tier-2 fallback. Current global Tool policy is checked independently
before every dispatch across all retained versions and aliases.

Distinguish own functional approval, exact shipped usage qualification and task
selection. The binding successor contract is
[skills.md](../skills.md#own-functional-approval-and-successor-compatibility).
Installed-instance authored changes (A) require complete own Q1/behavior/human
Q2. Skill and implementing PythonCode share one functional review with separate
immutable IDs; an unchanged approved partner/dependency is context, not a new
candidate. Compatible successors preserve still-needed consumer contracts;
unchanged consumers retain their approvals without an extra A combination review.

Controlled shipped `system_seed` qualification (S) retains exact Skill usage
evidence after independently qualified Tool, then ToolSkill prerequisites.
It needs integrity/Q1/behavior, not additional installed or external human Q2;
its dependency closure does not approve every member retroactively. Provenance
belongs to each revision/usage, including mixed graphs. Source/status labels
cannot establish S or override authored edits. Current exact `/1` authored
profiles retain their original meaning until all required successor readers and
writers support coherent publication. Do not first finish the superseded generic
A closure-reapproval design. Neither approval nor a manifest grants Tool
permission. Documentation alone establishes no implementation or activation.

## Skill/Recipe contract correction and evidence applicability (2026-10-28)

A Skill needs no dedicated Recipe. Distinguish exact usage review, actual consumer/
test workflow qualification, current complete-Recipe deployment and optional MCP
publication. Current source readers already separate usage combinations from
Recipe-owned `mcp_call`; this correction adds no standalone library activation,
broader export inventory or direct execution API. New acceptance cases belong to
their existing validator/composition/prefix/upgrade owners, not a new test service.

Preserve existing workflow IDs, old selections and historical evidence. Updated
generated prose and embedded guides change immutable content/package checksums.
Do not overwrite approved DB revisions, reuse old exact-byte receipts for changed
subjects or claim installed uptake from source edits. Ship reviewed successors
through existing revision/bootstrap/upgrade paths; unsupported upgrade remains an
explicit gap, never a reason to mutate originals or bypass integrity. Reassess only
affected evidence; unchanged execution logic does not require a blanket remigration.

## Shared source and central build ownership (2026-10-30)

Use the common repository-root checkout and Git index for all current work.
Compile that source directly with one canonical Cargo target; on this host it is
`/Users/ollama/brassclaw-target`. Cargo executions are serial under
`/private/tmp/brassclaw-target-cargo.lock`. Do not restart the retired target2
or mutable validation build mirrors. Never replace current source with an older
frozen projection.

The human-designated leading Root/Auth chat keeps Cargo and build ownership.
Do not pass build windows between chats. Other chats send proposed work and
ready validation requests to that chat; only the leading chat runs Cargo,
including build, test, Clippy, metadata, fmt and clean. This supersedes historical
window handoffs. Use the [central build queue](development-build-queue.md) for
current requests, outcomes and source/evidence references; the leading chat is
its only writer. This is development coordination, not a product runtime service.

Before shared implementation, identify the existing plan owner, affected paths,
reused interfaces and any proposed shared contract changes. The leading chat
checks overlap and chooses one implementation of shared responsibilities.
Central compilation alone cannot prevent duplicate architecture. Component
review/activation requirements remain independent of this development review.

A request supplies its intended behavior, source before/after hashes or a bounded
patch, affected packages/features, required checks and applicable prior evidence.
The leading chat reviews and integrates compatible requests, batches only where
failure diagnosis and evidence remain clear, and records exact inputs/results.
Failures return to the author for connected repairs; authors submit the successor
to the same queue rather than starting their own Cargo run.

Collect the ready, reviewed, compatible patches before starting the next compilation.
Compile shared affected artifacts once, then reuse them for the necessary selected
cases. Do not start a new compile for each small patch or repeat passing checks
whose inputs remain applicable. Keep independent failures attributable; a bundle
does not turn unrelated changes into one acceptance claim.

Run long validation batches in detached Screen sessions and continue implementation
in the foreground. Check progress after useful work intervals, on failure or on
completion; do not spend the foreground turn polling an unchanged build. Capture
only the actual compiler, embedded-content and runtime inputs required by the
batch, allowing unrelated source work to continue. Changes to those captured inputs
remain private patches until that batch ends. Screen detaches the process; it does
not make concurrent Cargo executions or edits to captured inputs safe.

While a batch runs, its complete captured dependency/input set is frozen. Other
chats may inspect source, develop bounded patches outside the checkout, and edit
explicitly assigned paths outside that input set. They must not change captured
inputs, build another tree/target, or introduce a parallel owner/interface. Private
patches are proposals, not competing canonical trees. Apply them against current
preconditions after review; never overwrite newer source with archived bytes.
Release the source freeze after the batch is terminal; retain Cargo ownership.

On this host, Codex configuration supplies `CARGO_TARGET_DIR`. Invoke Cargo directly,
for example `cargo test -p package_name test_name`, without an inline target assignment
or an explicit `/bin/zsh -lc` wrapper. Background runners inherit that configured
environment rather than overriding the target themselves. The one-target,
serial-execution and disk-space rules still apply.
Do not clean build caches routinely; clean only when required by the disk-space
rules or a diagnosed stale/corrupt artifact problem. Other hosts use their
configured canonical target rather than copying this host's absolute path.

Keep immutable historical source/binary/receipt archives for evidence reuse and
failure diagnosis. They are not competing development trees or build inputs.
Preserve actual needed executables before cleaning disposable caches; do not
clean the repository's entire `target` directory because it contains evidence.
Follow the unchanged free-space rules and choose checks for the merged claim.
A combined-root build does not retrospectively broaden older frozen receipts.

## Reason before compiling

Use the development LLM's reasoning capability to diagnose, design and review
the change. BrassClaw's runtime Tier-0/LLM-minimal rules govern product
execution, not the reasoning used to develop it.

Before editing, establish the expected behavior, the cause of any reported problem,
affected callers and contracts, and any unresolved assumptions. For a capability,
first search for existing Recipes, variants, Skills, PythonCode and ToolSkills.
Justify a new Rust Tool with the exact missing system primitive after considering
an explicit sequence of existing usages. Rust infrastructure work may be needed
for runner, binding, persistence, lifecycle or kernel support. Repair those gaps
in their owning layers; they do not justify hiding a user workflow in a new Tool.
Limit exploration to the affected behavior, shared contracts and necessary
consumers. Reuse source knowledge already obtained in this session while it
remains current; reread when relevant edits or unresolved questions require it.
Do not repeatedly inventory the whole repository for each small edit. Still search
for other affected instances when diagnosing a shared defect or changing a shared
contract; bounded exploration must not leave known consumers unfixed.

Before changing an interface, read its actual definition, the implementation
selected by the relevant feature configuration, and the affected callers.
Trace data, errors, ownership, wrappers, persistence and side effects through
the affected path. Find all implementations when changing a trait. Do not infer
APIs, fields or behavior from similar names or memory, or use compiler failures
to discover interfaces already available in source. Resolve relevant assumptions
through source inspection before relying on them. When inspection cannot resolve
a concrete uncertainty, run a small targeted experiment and state what it will
establish; this may require compilation before the work package is complete.

Before making the change, reason through the exact argument and return types,
ownership and borrow lifetimes, mutability, async/Send requirements, imports and
visibility, feature-gated code and every affected trait implementation. Check
the intended edits against neighboring working code and the actual dependency
APIs. Address predictable type, ownership and visibility problems in the planned edit.
Reasoning reduces avoidable failures; it does not replace the real compiler when
changed Rust requires verification.

## Implement and validate coherent work packages

This sequence adds no requirement for a separate planning document, approval or
subagent; existing authorization and component approval requirements still apply.
Scale the analysis to the change; a small fix needs only a brief review.

1. **Define one bounded behavior change.** Identify the expected result, affected
   contracts and callers, existing applicable evidence, and the required checks
   from the table below and subsystem acceptance criteria. Include the producer,
   data handoffs and consumers needed to make this change complete. Split unrelated
   changes into separate packages; do not batch an entire plan before testing.
2. **Complete the connected implementation.** Update the affected contracts,
   implementations and callers together, including necessary regression coverage.
   Avoid compiling after each small edit. Compile earlier when a concrete unresolved
   question blocks further work or when a shared boundary needs an intermediate
   check before dependent changes would make failures difficult to isolate.
3. **Review the complete relevant diff before validation.** Check argument/result
   types, ownership, feature configurations and affected callers. Trace normal,
   missing, empty, invalid and interrupted cases where applicable. Check error
   propagation, retained revisions, cancellation and completed or unresolved effects
   when affected. Confirm the intended production caller reaches the change.
   Correct source-visible defects together and identify remaining questions that
   require execution. A mental walkthrough is not observed behavioral evidence.
4. **Run the smallest sufficient validation batch.** Cover the required behavior,
   remaining uncertainties and affected-package linting where required. An existing
   caller-level test may cover several properties; add separate tests only for
   important cases it does not reliably exercise. Do not routinely stack check,
   build, test, all-feature lint and release build. A test build can supply the
   compilation evidence for its actual targets/features; build another binary only
   when that artifact or a different configuration is required. A batch may need
   several commands and must not omit required acceptance cases to save time.
5. **Repair causes before rerunning.** Read the complete available diagnostics,
   trace the causes and fix the connected set of defects before the next expensive
   run. Review the corrections, then repeat failed checks and any previously passed
   checks whose applicability those corrections invalidate. If the same failure
   recurs, revisit the diagnosis before another rerun; do not repeat an unchanged
   command without a specific reason, such as a resolved environment failure or
   an explicit investigation of nondeterminism. Do not start a new
   compilation for every individual diagnostic fix.

For checks that compile/link code, run native workers or external services, or
exercise a broad suite, identify the requirement or unresolved
question it covers and why existing evidence is insufficient. Keep this rationale
brief in the validation record or progress update; do not create a new reporting
framework. Stop validation for that package after its required applicable checks
pass, then continue other authorized implementation work. Broader combined-product
and release acceptance still runs at its required integration/cutover milestones;
a local work package is not a substitute for those gates.

## Choose evidence by the claim

Apply every row relevant to the change. One check may satisfy several rows;
overlapping requirements do not require duplicate executions. The table defines
development evidence and does not replace component review or release gates.

| Change | Required local evidence |
| --- | --- |
| Ordinary documentation or development policy | Inspect the diff, check relevant links and consistency, and run `git diff --check`. No Cargo execution solely for prose. |
| Component data, prompts or PythonCode | Check the affected schemas, references, routing, associations and behavior through the relevant execution path. Stored drafts and development tests do not establish activation. Before activation, require complete own A function/workflow Q1/behavior/human Q2 or verified S integrity/behavior and applicable exact usage evidence; existing exact authored profiles keep their actual supported scope until cutover. |
| Seeder source or embedded runtime content | Validate the affected seed/integrity and execution contracts. These changes can require recompilation even without a new Rust function. |
| Rust behavior | A focused regression or existing behavioral test through the relevant caller, plus affected-package linting with the relevant feature configuration. |
| Database semantics, migrations, security or runtime lifecycle | Focused real integration/production-path acceptance, including relevant failure cases. Source reasoning alone cannot certify these claims. |
| Shared contracts, dependency edges, build configuration or release | Expand to affected consumers, architecture checks and the applicable acceptance matrix. Release builds belong to release/performance work. |
| Validation scripts, hooks or CI | Review the actual execution path and use relevant real checks. Do not add a second validation framework just to reduce compilation frequency. |

Use real implementations and verify real results. Do not introduce fake commands,
mocks, stubs or simulated success to avoid required verification. Caller-level
coverage of a PostgreSQL path needs actual PostgreSQL; use the actual backend
for filesystem or other backend-specific claims. Pure logic can be tested
directly without adding substitute services. Add focused regression coverage at
the layer that catches the bug; one test may cover several cases, while distinct
failure modes may require more. Do not duplicate helper tests merely to increase
test counts. Existing coverage may suffice when it already exercises the
regression; explain that evidence. Honor documented subsystem acceptance.
Verify that the intended tests actually ran: a zero exit code with zero selected
tests, skipped database tests or unavailable dependencies does not establish the
behavior. Record those gaps as unverified, not passed. Do not remove a required
case merely because its environment is unavailable.

For dispatch, retry, cancellation, continuation or recovery changes, apply
skills.md's exact failure contract and tools.md's durable effect/recovery contract.
Only genuine No-Match enters Tier 2; matching/composition errors and begun Recipe
failures remain distinct. Preserve invocation counts, original deduplication keys
and arguments, pinned selections and confirmed/unresolved effect status across
waits, reclaim and recovery. Required dispatch intent/count persistence precedes
effects. A timeout or missing completion record does not prove no effect occurred.
Retries need explicit eligible outcomes and trusted safety evidence; recheck live
policy, cancellation and attempt freshness before dispatch. Never replay completed
effects because output validation, a later step or reply persistence failed.
Fatal VM recovery must fence the old generation and reconcile recorded effects
before supervised replacement; no whole-Recipe replay or per-chat/Tier-2 fallback.

Fix warnings introduced by the change, including those surfaced in unchanged
consumers. Report unrelated baseline warnings separately; do not suppress them
or expand the task to clean up the workspace. Strict full-matrix CI/release
checks remain authoritative. If baseline warnings block a strict check, report
the limitation instead of claiming success.

## Reuse evidence and stop

Capture command output once. Record the claim established, tested commit or
relevant diff, actual caller/test target and Cargo manifest path, package,
features, toolchain/profile, result and limitations. Where relevant, identify
component revisions/checksums, association approval records, retained implementation
artifacts, database migration state, catalogue generation and effective settings
revision. Separate test workspaces need their own applicable evidence; a root
workspace result does not cover them automatically.
Record shared environment and source details once per batch and reference them
from individual results. Reuse existing logs and receipts rather than creating
parallel evidence formats or copying full outputs into multiple documents.
For ordinary documentation-only changes, review the diff, relevant links and
consistency and record the whitespace result; no runtime manifest or new evidence
archive is required. Embedded runtime documents follow the seed/content row above.

Passing evidence remains usable until a relevant code, dependency, runtime data,
artifact, configuration or toolchain change invalidates its applicability. A newer
prose edit does not invalidate unrelated runtime evidence. If a binding contract
or acceptance claim changes, preserve the historical result but reassess whether
it establishes the new requirement. Do not rerun a command to retrieve output,
satisfy another checklist containing the same command, or produce a second
success message.

Keep target directory, feature set and build profile stable during a validation
batch. Coordinate work so checks cover a stable relevant diff; do not certify
concurrent edits made after the check. Queue Cargo work sharing a target directory
rather than starting competing builds. A Cargo lock serializes builds, not source
edits. Coordinate with other writers so relevant compiler inputs stay unchanged
during validation; if they change, identify which results must be repeated.
Unrelated edits do not automatically invalidate the entire batch.

Historical snapshot-replay procedures do not authorize restoring an old snapshot
over the shared checkout or reviving a retired build mirror. If files in the
canonical checkout were restored with preserved timestamps, Cargo may reuse stale
artifacts. In that case, finish any active build, verify the intended current
contents (including removed inputs), and invalidate affected build fingerprints
or give restored inputs fresh modification times before rebuilding. Hash equality
alone does not prove that an executable was rebuilt. Preserve any earlier result
with its limitation; do not clean unrelated caches to address a localized problem.

Finish a work package when the change is reviewed and its required relevant
checks pass. Continue remaining authorized work; finish the overall task only
when its requested implementation and acceptance are complete. Broaden or repeat
checks only for new changes, failures, acceptance requirements or a
remaining uncertainty. Report skipped, blocked and unverified checks honestly.
Identify whether evidence covers a helper, constrained draft-validation path,
candidate integration or ordinary production caller. A prerequisite change can
be complete while activation, production wiring or the v3 cutover remains open.
Passing real worker/database tests on a candidate path does not certify ordinary
startup or whole-task completion. Production/cutover claims require the applicable
Phase 0a, Phase 3a and Phase 7 acceptance and Phase 8 cutover gates in
simplified_v3.md through the actual supported product path.

## Iteration, submission and acceptance

During iteration, use source reasoning and the smallest relevant checks. Before
submission, format/review the final diff and reuse passing targeted evidence.
Existing hooks, CI and release acceptance checks remain in place. Do not repeat
their entire command lists during every intermediate edit. Expand local validation
when affected contracts, failures or acceptance requirements justify it, and keep
real production-path coverage for database, runtime and security changes.

Development-only policy lives in this unembedded file and `.claude/rules/`.
The architecture guides and executable component seeds remain embedded and
integrity checked. Keep frequently revised development procedures here; do not
remove runtime seed verification to avoid compilation. Component data changes
applied through supported stores can avoid a binary rebuild; edits to Rust seed
constants cannot. Externalizing executable seed data is a separate architecture
change requiring its own integrity and upgrade contract.
