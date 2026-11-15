# Legacy seed review retirement — 2026-11-15

## Change

Shared component boot collects definitions from the actual packaged constructors
for Tools, ToolSkills, Skills/protected prompt rows, PythonCode, Recipes, Actions
and ExtensionCatalogues. It compares the complete semantic projection, including
metadata and references, before retiring an obsolete legacy queue request.
Generated identity/accounting/delivery fields are classified explicitly; an
unknown semantic column prevents retirement. Catalogue child arrays compare as
sets, matching the existing append contract.

Only untouched state-1 requests are eligible: no proposal, rejection count,
feedback, validation errors, Q1 seals, Q2 actor, original queue binding or immutable
authored submission. Scope/class/name and definition must match the packaged
constructor. A source label or matching code body alone does not suffice.
Changed definitions, other scopes and genuine review work remain active.

V216 adds an immutable `reborn_retired_seed_review_requests` archive. Queue,
component and canonical package snapshots retain their full values and checked
SHA-256 identities. Archive and active-queue removal commit together under the
existing queue-key/row/component locking order; failure preserves the request.
An exact canonical legacy pending component projection becomes validated using
the inherited package status, without executing Q1/behavior or recording Q2.
Previously retained evidence, submissions, findings and graduation receipts are
not rewritten. Repeated startup does not archive an already retired request again.

The ordinary empty-queue path uses a batched queue scan rather than a database
lookup/lock per shipped component. Canonical expectations exist only in the
private shared boot task context. Ordinary authored queue recovery is unchanged.

This implements automatic cleanup on startup with the new source/schema. No
existing operator/development/production database was accessed to apply cleanup
manually. Definitions that cannot be proven against this package remain intact.

## Plans

Active instructions in B1/B2, path_to_v3.5, validator_v3, simplified_v3 and the
Composition, Prefix and relevant v3.5 supporting plans now place package
qualification in development/release before provisioning. Installation transfers
validation and performs technical identity/integrity/loading checks. Dated
implementation observations and original receipt names remain historical evidence;
they do not instruct a fresh instance to repeat qualification. “Installation
owner” identifies the existing software responsibility, not a new human approver.

## Validation

Acceptance passed. Raw evidence is under
`target/validation/seed-review-retirement-20261115-r2/` and
`target/validation/seed-review-retirement-20261115-r3/`; the latter's
`accepted.json` records the final scope, source comparison and retained binary
identities.

The first check was deliberately interrupted after identifying that PostgreSQL
marks `convert_to` STABLE and therefore disallows it in a generated column. The
corrected archive uses ordinary checksum columns with database CHECK constraints,
following the existing review-evidence pattern. The interrupted run remains
under `seed-review-retirement-20261115-r1`; it is not accepted evidence.

Both corrected strict Clippy graphs passed (default library and skills-db all
targets, `-D warnings`, Rust 1.100.0). Native test and worker compilation passed.
Changed/added local links resolve and `git diff --check` passed.

Six actual PostgreSQL caller regressions passed on r2, with zero ignored cases:
fresh/repeated seed boot with queue writes forbidden and historical reviews
retained; authored orphan-queue recovery failure; idempotent root boot and changed
prompt refusal; retired L013 history preservation; fresh/repeated global startup
with no qualification admissions or model fixtures; and ordinary global
match/No-Match/MCP/policy/cancellation/restart behavior.

Source inspection then corrected only the new test's setup from
`submit_retained` to `submit`: its authored candidate is new and must first be
staged, rather than assumed already retained. The incorrect setup was not run as
an acceptance case. All-target strict Clippy and native compilation passed for
the corrected r3 test. No production input or existing test case changed;
`evidence-reuse.json` compares r2/r3 manifests and preserves the six applicable
results without rerunning them. The default non-test lint and unchanged worker
were also reused under that exact comparison.

The corrected actual PostgreSQL retirement case passed in 3.68 seconds. It
archived seven exact untouched seed requests, one for each table, preserving
their full queue snapshots and checksum identity. It restored an exact pending
PythonCode projection, retained a queued operator edit, an additional metadata
edit, historical review findings, an immutable authored subject and a
foreign-scope row carrying a system label. These retained queue rows and the
authored subject remained byte-identical. Repeated startup did not duplicate
archives; immutable archive deletion was refused. An injected real database
archive failure preserved the original active request; after removing that
fixture fault, normal startup retired it successfully.

All seven selected tests passed, with no failures or ignored cases. Each used
its own temporary PostgreSQL instance outside the sandbox after the previously
recorded, unchanged `shmget` blockade. Orderly fixture shutdown completed. No
PostgreSQL, Monty worker or Composition-test process remained. All owned Screen
sessions ended in `/private/tmp/mcp-ipc-01a1dc5d-sockets`; final notifications were
accepted and actually received in this chat. The intentionally interrupted first
check is separately identified above and is not a passing run.

Compiler and fixture inputs stayed frozen. Another task changed the standalone
`unnecessary_stuff_v3_at_bob1.md` report; its before/after identities are recorded
in `post-run-source-delta.json`. It is not referenced from crates, Cargo.toml or
.cargo and does not invalidate these checks. Tested executables are retained
under each evidence directory's `binaries/`, independently of subsequent Cargo
output replacement.

The separately reviewed startup-transfer baseline was integrated by Root as
`71c3057a6ac0f65d730aa220dd9970f6f9a67ac6` while r2 was running, without changing
the frozen worktree. At acceptance, this follow-up's V216, cleanup and plan changes
were uncommitted. The subsequent local commit reuses these exact source inputs
and results. No push or manual application to an existing instance was performed.
