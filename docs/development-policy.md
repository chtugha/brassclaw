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

**Single-agent workflow — user instruction, 2026-11-03:** the agent in the
current user chat is the sole development, integration and build owner. This
supersedes previous delegation, multi-chat coordination, build-window handoffs
and development build-queue requirements. Do not spawn/delegate to agents,
create per-agent worktrees/branches, or dispatch work to other chats. Use the
current checkout and perform implementation, review and validation directly.
This changes development coordination only; BrassClaw's product architecture
and its Recipe-defined child/subagent workflows keep their existing contracts.

**Narrow two-chat exception — explicit user instruction, 2026-11-03:** one
additional user-owned chat may implement the Codex/Auth portion of Bob 3 in a
separate Git worktree and use the qualified Modal compiler. The leading chat
keeps Bob 1, Bob 2, shared Q1/driver lifecycle, native Cargo/tests and final main
integration. The user explicitly approved that worktree. Do not expand this into
further delegation. The second chat runs no local Cargo or remote tests, writes
only its assigned source/contracts and commits on its own branch without pushing
or merging main. Necessary eligible Modal builds within this assignment have
the user's location authorization; retain the existing resource/cost rules and
one remote job at a time. Independent source snapshots/caches and per-environment
locks permit that compiler to overlap the leading chat's one native Cargo job.
The leading chat reviews/merges its commits and runs actual native acceptance;
neither branch's receipt certifies the other branch's different inputs.

Use the common repository-root checkout and Git index for all current work.
Compile that source directly with one canonical Cargo target; on this host it is
`/Users/ollama/brassclaw-target`. Native Cargo executions are serial under
`/private/tmp/brassclaw-target-cargo.lock`. Do not restart the retired target2
or mutable validation build mirrors. Never replace current source with an older
frozen projection.

**Independent build environments — user instruction, 2026-11-03:** native and
Modal compilation may run concurrently with different settings. Serialize jobs
within each environment, not globally across the two hosts. Native Cargo keeps
its canonical target and lock; Modal sync/submission uses
`/private/tmp/brassclaw-modal-build.lock` on this Mac and an immutable source
snapshot with its separate remote cache. Both settings and limits are documented
in [the build environment contract](build-environments.md). This supersedes the
former global-Cargo serialization rule and one-off parallel-host flag requirement.
It does not change source custody, paid-build authorization or local-only tests.

The sole agent runs Cargo, including build, test, Clippy, metadata, fmt and clean.
The former [development build queue](development-build-queue.md) is a historical
record, not an active submission or scheduling mechanism. Preserve earlier
receipts and unresolved work; queue retirement does not mark that work complete.

Before implementation, identify the existing subsystem/plan owner, affected
paths, reused interfaces and proposed shared-contract changes. Check overlap
and implement shared responsibilities once. Component review/activation
requirements remain independent of development review.

Record intended behavior, actual source changes, affected packages/features,
required checks and applicable prior evidence in the existing work/validation
record. Review the connected changes and fix causes before rerunning failures;
no inter-agent request, handoff or queue entry is required.

Collect coherent, reviewed changes before starting the next compilation.
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
not make competing Cargo executions within one environment or edits to its
captured inputs safe. The isolated Modal environment may compile concurrently.

While a native batch runs, its complete captured dependency/input set is frozen. The
sole agent may inspect source and work on paths outside that input set. Changes
to captured inputs wait until the batch is terminal or remain unapplied patches.
Do not build another local tree/target or overwrite newer source with archived
bytes. Reconcile actual processes, locks and retained inputs before taking over
work left by an earlier chat; never force-release an active build's ownership.
Release the source freeze after the batch is terminal.
For a Modal batch, the admitted remote snapshot stays frozen; later local
checkout edits cannot change it. A simultaneous native batch retains its own
input freeze independently. Source capture/admission itself still requires
stable local bytes and fails if they change during synchronization.

On this host, Codex configuration supplies `CARGO_TARGET_DIR`. Invoke Cargo directly,
for example `cargo test -p package_name test_name`, without an inline target assignment
or an explicit `/bin/zsh -lc` wrapper. Background runners inherit that configured
environment rather than overriding the target themselves. The one-target,
per-environment serial-execution and disk-space rules still apply.
Do not clean build caches routinely; clean only when required by the disk-space
rules or a diagnosed stale/corrupt artifact problem. Other hosts use their
configured canonical target rather than copying this host's absolute path.

Keep immutable historical source/binary/receipt archives for evidence reuse and
failure diagnosis. They are not competing development trees or build inputs.
Preserve actual needed executables before cleaning disposable caches; do not
clean the repository's entire `target` directory because it contains evidence.
Follow the unchanged free-space rules and choose checks for the merged claim.
A combined-root build does not retrospectively broaden older frozen receipts.

### Modal usage and cost policy (2026-11-03)

**Activation prerequisite — user instruction, 2026-11-03:** until a suitable
Modal compiler image has been successfully qualified and explicitly activated
for the requested build target, run Cargo locally without asking Local or Modal,
including commands expected to take at least 60 seconds. Authentication,
available storage, a local image definition or a successful image build alone
does not establish activation. Missing, disabled or inapplicable activation keeps
the local path available without a Modal prompt, source upload or cloud-readiness
probe. An explicitly commissioned image setup/qualification job may run under
its own authorization before activation; it does not enable general offloading.

Local execution is the default. Modal is eligible only for necessary compilation
or static-analysis work whose expected local command runtime is **at least
60 seconds**, and only when the expected complete remote turnaround (source
transfer, provisioning, cache restore, command and result collection) improves
on local execution. Eligibility is not an instruction to offload every long run.
Use recent applicable timings for the same packages, features and warm/cold cache
state. If there is no credible estimate, keep the work local and use its normal
required run to inform future estimates; do not launch duplicate timing probes.
The threshold chooses execution location before launch, not a timeout or a reason
to restart a local command remotely after 60 seconds.

**Build-location choice — user-confirmed, 2026-11-03:** before an eligible non-test
compilation or Clippy batch expected to take at least 60 seconds locally, ask the user to choose
Local or Modal only when a qualified, explicitly activated image is available
for that command's target. State the estimate and recommend local
when remote turnaround/cost offers no advantage. Wait for the answer; do not
interpret account authentication or a previous batch's choice as permission to
choose Modal automatically for the next batch. Short runs and test execution/compilation
remain local under the rules below. Source byte size is not the threshold.

**Temporary explicit Modal choice — user instruction, 2026-11-03:** after
verifying the qualified Apple build image, this chat is authorized to use it
for its necessary eligible compilations while the other chat finishes its
validation build. Do not ask Local/Modal again within that explicit scope.
The general independent-environment contract above now permits isolated
remote/local overlap; do not interfere with local validation's source, artifacts
or processes.
Keep one remote build at a time, exact immutable remote inputs, separate target
caches, the 60-second eligibility rule and all test work local. Once that
validation work is finished or the user changes this instruction, return to
the ordinary per-batch location choice above.

**No speculative spending — user instruction, 2026-11-03:** do not provision
workers, build/publish compiler images or run paid sizing/benchmark jobs merely
to prepare for possible future builds or answer a question. Reuse existing
storage/images. Any necessary paid setup must be disclosed with its estimated
cost as part of a concrete eligible build's Local/Modal choice, or obtain an
explicit request for image setup and qualification; wait for that authorization
before starting it. Do not solicit a general build-location choice while the
image is inactive. Account authentication and a prepared local script
are not spending authorization. Source sync runs for an actual compilation or
explicitly requested storage setup, not every edit or planning response.

Before an authorized remote compilation, the Modal controller synchronizes exact
source through the existing `modal_source_sync.py` implementation. Standalone
`python scripts/modal_source_sync.py` remains available for authorized storage
setup/sync and uses the same remote lock. Local
Cargo does not require a Modal source sync or availability check. Initial
authorized setup uses `--create`; use the **`brassclaw` Modal environment**
explicitly (the helper's `--environment` default). The default Volume is
`brassclaw-build`, configurable with `--volume` or `BRASSCLAW_MODAL_VOLUME`.
Do not inherit the account's active `main` environment. This is storage transfer
only; it starts no worker and executes no Cargo/test command. The helper uses
the independent Modal lock and publishes a verified `/source/base.tar.gz` and base
manifest once, then only changed-file overlays in `/source/repository.tar.gz`
and the complete `/source/manifest.json`, retaining `/cache` separately. The archives
includes tracked and non-ignored pending files, safe symlinks and executable
modes. A fresh extraction represents deletions without stale working files.
No local `target/` or Git-ignored credentials are synchronized.

Remote compilation requires successful sync/readback, unchanged captured inputs,
base/overlay digest/member verification and fresh materialization before the
direct Cargo command. Apply exact deletions/replacements from the current
manifest and verify every final path, byte hash, symlink and executable mode;
never compile the overlay alone or leave stale base entries in the source tree.
Retain the original HEAD/manifest with results; never compile an old
remote archive after a failed sync. If Modal is unavailable, report that state
and continue an authorized local build; do not hide the sync failure or start a
replacement paid worker. The controller holds the Modal lock continuously across
unsettled-job preflight, in-process sync, readback, submission and terminal receipt
collection. Do not invoke the standalone sync CLI inside a lock-owning controller.
Recheck the captured source identity before submission. A disconnected unresolved
job blocks remote sync/new submissions even after its OS lock is released.

**Tests stay local regardless of duration.** Do not run unit, integration,
browser, end-to-end, behavioral or acceptance tests on Modal, or move a test suite
there under a benchmark/smoke/setup label. Test-binary compilation, including
`cargo test --no-run` and `cargo build --tests`, also stays local so its artifacts
remain usable by the local test environment. A mixed build-and-test command or
runner stays local; do not submit it to Modal on the strength of its build time.

Clippy is local by default. A demonstrated long Clippy batch may use Modal only
under the same 60-second and total-turnaround rules. `cargo clippy --tests` checks
test-target code without running tests and is static analysis; it does not permit
invoking a test executable or using a test runner remotely. Verify the complete
command, wrapper and build-script behavior before submission: custom compilation
steps must not execute tests on Modal. Formatting, metadata inspection, short
checks, scripts and document validation remain local. Do not add unnecessary
targets/checks or pad a batch merely to cross the threshold. Do not rerun passing
local Clippy remotely merely to obtain a second receipt.

For an eligible job, record the source identity, exact command/target/features,
timing basis, selected CPU/RAM and estimated compute cost in the existing
validation record; capture actual duration/cost when available with its outcome.
Request only resources justified by the workload and measured scaling, preserve reusable
toolchain/dependency caches, and release paid workers after their necessary work.
No GPU is needed for ordinary Rust compilation. Required checks are never omitted
or weakened to meet the time/cost policy.

The sole agent controls Modal submissions directly under the independent remote
lock until remote Cargo is confirmed terminal. Native Cargo may run concurrently
under its native lock with different settings and a separate target cache.
A disconnected client is not proof that the remote job stopped; reconcile its
actual state before admitting another remote build or changing remote source.
Local work remains available when remote state is unresolved.

**Local tests may run concurrently with remote Cargo.** Test compilation still
uses the native Cargo lock and its canonical target; execution-only native
runners retain their existing backend/test-resource locks. Preserve exact
application/worker/test artifacts, fixtures and source inputs for each claim.
Do not interrupt or bypass an active native build's locks/frozen inputs to enable
concurrency. The remote worker uses its already captured immutable snapshot;
local results establish only the source/artifacts actually tested, even if Modal
is compiling a different snapshot. No tests or test compilation move to Modal.

The local checkout remains authoritative. Remote jobs use an exact isolated
source snapshot with original tracked/pending input hashes, not a second editable
development tree or an old projection restored over the checkout. A persistent
remote source directory may receive one-way changed-file updates between jobs,
including deletions, after its prior job is confirmed terminal. Verify the full
resulting input manifest before compiling; do not sync into a running job's
inputs or overwrite local changes from the remote directory. Preserve a separate
remote target cache with platform/toolchain/profile-aware identity; do not sync
local and remote `target/` directories or mix Linux artifacts into the local
macOS target. Persistent storage and transfer allowances are distinct from
compute cost and must use actual current account pricing, not an assumption that
all storage/transfer is free. Return logs and exact source/
artifact identities through existing evidence owners. Remote Linux compilation
or lint does not establish macOS/Windows acceptance or local test completion.
Do not copy operator/provider credentials into the source snapshot. This policy
does not authorize provisioning or compiling as part of a documentation edit.

**Evidence retention format (2026-11-02):** each validation run writes
`source.json` (per-file SHA-256 manifest of all git-tracked files plus HEAD
commit hash), `applied.json` (before/after SHA-256 for changed files),
`original-runner.py`, per-step log files, `completed.json`, and the compiled
test binaries. `accepted.json` is written by a separate post-run acceptance
step. The runner must not copy the full working tree, `crates/`, `docs/`,
root plan files, or changed-file copies (`owned-source/`) into
`target/validation/`. Binary SHA-256 hashes in `accepted.json` are the
immutable artifact identity; the binary is retained for re-execution
capability for the accepted run only. Binaries from failed and superseded
intermediate runs are disposable after the receipt is written.

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
