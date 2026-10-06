# Development reasoning and validation policy

## Recipe architecture checks

For component changes, use [recipe.md](../recipe.md). Review one referenced
component per Recipe step, internal PythonCode composition, typed bindings and
result handoffs. Verify IBS resolves the newest approved versions consistently
at task start and pins the complete transitive manifest in BuildInstruction.
Retain old immutable versions for running/suspended tasks; activation does not
invalidate them. Current global Tool policy is independent. Documentation alone
cannot establish runtime enforcement, Q1/Q2 activation or production acceptance.

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
first search for existing Recipes, PythonCode and ToolSkills. State why a new
Rust primitive is necessary if the component library cannot express the change.

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
| Component data, prompts or PythonCode | Check schemas, references, intent routing, Q1 and the relevant execution path. Preserve Q2 where required. |
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

Fix warnings introduced by the change, including those surfaced in unchanged
consumers. Report unrelated baseline warnings separately; do not suppress them
or expand the task to clean up the workspace. Strict full-matrix CI/release
checks remain authoritative. If baseline warnings block a strict check, report
the limitation instead of claiming success.

## Reuse evidence and stop

Capture command output once. Record the tested commit or relevant diff, package,
features, toolchain/profile, result and limitations. Passing evidence remains
usable until a relevant code, dependency, configuration or toolchain change
invalidates it. A newer prose edit does not invalidate unrelated runtime evidence.
Do not rerun a command to retrieve output, satisfy another checklist containing
the same command, or produce a second success message.

Keep target directory, feature set and build profile stable during a validation
batch. Coordinate work so checks cover a stable relevant diff; do not certify
concurrent edits made after the check. Queue Cargo work sharing a target directory
rather than starting competing builds. Before a rerun, read the complete previous
diagnostics, trace their causes and fix the coherent set of related issues.

Stop when the change is reviewed and its required relevant checks pass. Broaden
or repeat checks only for new changes, failures, acceptance requirements or a
remaining uncertainty. Report skipped, blocked and unverified checks honestly.

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
