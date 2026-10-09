# Development reasoning and validation policy

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
Each real Skill has a canonical execution Recipe with a matching command.
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
This is a binding target, not proof of implemented loader/store/runner support.
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


## Recipe architecture checks

For component changes, read the four ground-truth guides:
[recipe.md](../recipe.md) for workflows and persisted IBS syntax,
[skills.md](../skills.md) for usages, associations, recursive contracts and retries,
[tools.md](../tools.md) for primitives, retained implementations and live policy,
and [toolskills.md](../toolskills.md) for binding preparation. These component
contracts and the binding targets in [simplified_v3.md](../simplified_v3.md)
take precedence over historical examples and older subsystem descriptions of
scoped authorization, operation approvals or per-chat orchestrator lifecycles.

Review exactly one UUID per component step, internal PythonCode composition,
typed bindings and result handoffs. Matching and IBS/composition must use one
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

Distinguish individual component validation, exact-combination approval and task
selection. Follow skills.md's `skill-association/1` and separate trusted
`skill-association-approval/1` contracts. Authored combinations require successful
trusted Q1, human Q2 and observed behavioral evidence for the exact revisions
and dependency graph. Only verified first-party `system_seed` bootstrap permits
null Q2 evidence, with its required Q1/integrity and behavioral evidence; source
or status labels alone do not qualify. A new combination needs its required
approval evidence even when schemas remain compatible. Unchanged components
and already selected old combinations do not require reapproval solely because
replacements exist. Neither approval nor a manifest grants Tool permission.
Documentation alone cannot establish enforcement, activation or acceptance.

This is the authoritative policy for development iteration and validation.
Read it before changing behavior. Root and crate guides provide architecture,
contracts and commands; their command lists are not a checklist to run after
every edit. This policy supersedes older blanket development test/lint rules.
Subsystem acceptance criteria, kernel enforcement, Q1 and human Q2 remain
binding. The root disk-space checks, cleanup thresholds and NVMe target rules
are unchanged.

## Reason before compiling

Use the development LLM's reasoning capability to diagnose, design and review
the change. BrassClaw's runtime Tier-0/LLM-minimal rules govern product
execution, not the reasoning used to develop it.

Before editing, establish the expected behavior, the cause of the problem,
affected callers and contracts, and any unresolved assumptions. For a capability,
first search for existing Recipes, variants, Skills, PythonCode and ToolSkills.
Justify a new Rust Tool with the exact missing system primitive after considering
an explicit sequence of existing usages. Rust infrastructure work may be needed
for runner, binding, persistence, lifecycle or kernel support. Repair those gaps
in their owning layers; they do not justify hiding a user workflow in a new Tool.

Read actual signatures and implementations. Trace data, errors, ownership,
wrappers, feature gates, persistence and side effects through the affected path.
Find all implementations when changing a trait. Review edge cases and complete
a coherent implementation before compiling. Do not use compiler failures to
discover interfaces that are already available in source. Small experiments
are appropriate when source inspection cannot resolve a concrete uncertainty;
state what the experiment will establish.

Before making the change, reason through the exact argument and return types,
ownership and borrow lifetimes, mutability, async/Send requirements, imports and
visibility, feature-gated code and every affected trait implementation. Check
the intended edits against neighboring working code and the actual dependency
APIs. Resolve predictable compilation errors in this review before editing.
After editing, inspect the complete diff for the same issues before the first
targeted compilation. Reasoning reduces avoidable failures; it does not replace
the real compiler when changed Rust requires verification.

Before each additional executable check, identify the changed behavior or
unanswered question it covers. Do not stack check, build, test, all-feature
lint and release build by habit. A test build may already cover the required
compilation; a separate binary build is necessary when exercising that binary.

## Choose evidence by the claim

| Change | Required local evidence |
| --- | --- |
| Ordinary documentation or development policy | Inspect the diff, check relevant links and consistency, and run `git diff --check`. No Cargo execution solely for prose. |
| Component data, prompts or PythonCode | Check schemas, references, intent routing, exact associations and combination approval, Q1 and observed behavior through the relevant execution path. Apply authored Q2 or the verified trusted-bootstrap evidence contract as appropriate. |
| Seeder source or embedded runtime content | Validate the affected seed/integrity and execution contracts. These changes can require recompilation even without a new Rust function. |
| Rust behavior | A focused regression or existing behavioral test through the relevant caller, plus affected-package linting with the relevant feature configuration. |
| Database semantics, migrations, security or runtime lifecycle | Focused real integration/production-path acceptance, including relevant failure cases. Source reasoning alone cannot certify these claims. |
| Shared contracts, dependency edges, build configuration or release | Expand to affected consumers, architecture checks and the applicable acceptance matrix. Release builds belong to release/performance work. |
| Validation scripts, hooks or CI | Review the actual execution path and use relevant real checks. Do not add a second validation framework just to reduce compilation frequency. |

Use real implementations and verify real results. Do not introduce fake commands,
mocks, stubs or simulated success to avoid required verification. Caller-level
coverage needs PostgreSQL when the real path uses it; pure logic can be tested
directly without adding substitute services. Add one meaningful regression at
the layer that catches the bug; do not duplicate helper tests merely to increase
test counts. Existing coverage may suffice when it already exercises the
regression; explain that evidence. Honor documented subsystem acceptance.

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
rather than starting competing builds. Before a rerun, read the complete previous
diagnostics, trace their causes and fix the coherent set of related issues.

Stop when the change is reviewed and its required relevant checks pass. Broaden
or repeat checks only for new changes, failures, acceptance requirements or a
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
