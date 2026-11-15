# 05 — Skills and their preloadable interfaces

## 1. Purpose and definition

A Skill is one reusable Tool-usage pattern: prose plus explicitly associated
PythonCode exposing a preloadable function interface. Declare public names and
signatures, input/result/failure contracts, associated code and private
helper/constant dependencies. Loading defines functions; invocation performs the
usage. A larger multi-Tool domain belongs to an Extension/ExtensionCatalogue and
its workflows to Recipes. Pure PythonCode needs no zero-Tool Skill.

[skills.md](../../skills.md) governs this definition, approval and compatibility;
[toolskills.md](../../toolskills.md) governs binding descriptors. This source map
was checked on 2026-11-15. Class labels and separate tables are storage/consumer
classifications, not alternative architectural Skill kinds.

## 2. Location — verified sources

| Source | Symbols / role |
| --- | --- |
| [association_contract.rs](../../crates/brassclaw_skills/src/association_contract.rs) | `SkillAssociation`, `FailureContract`, `AssociationApprovalDeclaration` and `ComponentRevisionRef`. |
| [component_revision.rs](../../crates/brassclaw_skills/src/component_revision.rs) | Immutable component revisions and `RetainedComponentSnapshot`. |
| [value_contract.rs](../../crates/brassclaw_skills/src/value_contract.rs) | Recursive input/result value contracts. |
| [retained_tools.rs](../../crates/brassclaw_engine/src/memory/retained_tools.rs) | `prepare_retained_tool_program` checks unique selected Skill/code/ToolSkill/Tool/callable associations. |
| [retained_reviews.rs](../../crates/brassclaw_engine/src/memory/retained_reviews.rs) | `retain_prepared_tool_reviews` reads exact selected usage-review records. |
| [retained_preload.rs](../../crates/brassclaw_engine/src/executor/retained_preload.rs) | `PreloadDeclaration` and `ExportDeclaration` for `python-preload/2`. |
| [retained_source.rs](../../crates/brassclaw_engine/src/executor/retained_source.rs) | Inspects exact source and resolves the selected export/dependency environment. |
| [pg_skill_store.rs](../../crates/brassclaw_reborn_composition/src/pg_skill_store.rs) | `PgSkillStore` / `NewPgSkill` operate on the existing prose store. |
| [pg_association_review.rs](../../crates/brassclaw_reborn_composition/src/pg_association_review.rs) | `prepare` / `approve` handle the current exact association human-review path. |

## 3. Storage, association and reuse

Skill prose is stored in `reborn_skills` (classes 1–3); implementing executable
code is stored separately in `reborn_python_code` (class 22). ToolSkill (class 13)
is Rust-side binding metadata; Tool (class 0) is the primitive. Recipe (class 21)
orders usages and pure logic. ExtensionCatalogue (class 23) documents the domain.
`SKILL.md` is an import/export representation, not another Skill architecture.

`skill-association/1` is the explicit technical relationship between one Skill,
entry-point PythonCode, ToolSkill, Tool and callable, with recursive usage
contracts and fixed/computed argument declarations. It is not approval. Its
input/result declarations must agree with code and the primitive adapter; matching
names or adjacent prose/code arrays cannot establish the association.

The retained resolver checks exactly one matching association for the selected
code/ToolSkill/Tool/callable tuple. Multiple matches are ambiguity, not a reason
to pick the first. Export selection is separate; different export names alone
do not disambiguate that tuple. Reuse one usage when its contract fits; do not
copy primitives or code just to evade unsupported selection. A Skill requires
no owning/canonical Recipe or public command. Consuming/test workflows supply
their own inputs/order and applicable workflow evidence.

## 4. Preload and invocation contract

1. Select one coherent activated approved graph at task start. Pin exact
   interface, code, dependencies, usage evidence and Tool implementations.
2. Resolve declared code dependencies, recursive contracts, symbols and imports;
   reject missing/incompatible references, cycles and collisions before effects.
3. Load definitions in deterministic dependency-first topological order, breaking
   ties by **stable UUID** within the selected graph. This is an explicit
   [governing requirement](../../skills.md#selection-dependencies-and-loading-order).
4. Loading executes no Tool/model call, I/O, task scheduling or effectful
   initializer/default. A top-level `def` is not by itself proof of safe defaults.
5. Keep task-local export resolution tied to the selected owning usage, symbol,
   revision/checksum and binding. Invoke with typed data and capture the returned
   step value. The Recipe's explicit invocation or a valid declared default export
   supplies selection; never infer it from prose or array order.
6. Prepare the matching ToolSkill immediately before its executable Recipe step.
   Every actual host dispatch independently checks current global Tool policy,
   cancellation, attempt freshness and technical constraints.

Code and genuinely immutable constants may be shared. Mutable defaults, arrays,
closures over task data, inputs and result objects stay isolated per task,
attempt and invocation. New activation does not overwrite an old task's exports.
The current ordinary retained runner allows one bound Tool call per executable
step and stop-only failure, as detailed in [04-ibs.md](04-ibs.md).

## 5. Approval paths and supported profiles

### Existing exact-review profile

The unchanged `skill-association-approval/1` record identifies exact reviewed
association bytes and the selected revision closure with Q1/behavior and human Q2
for authored use, or verified bootstrap evidence for S. Existing authored exact
records retain their original supported meaning. `pg_association_review.rs` and
retained readers do not imply general own-functional successor publication.
A parsed record, queue state or caller-supplied successful report cannot establish
trusted provenance. Task selection records remain separate from approval.

### A — binding own-functional successor, implementation still open

For installed authored changes, Skill and implementation code form **one functional
review subject**, retaining separate immutable IDs. Validate prose, actual exports,
contracts, behavior and effects together. If one changes, its unchanged approved
partner is context; if both change, coordinate their own approvals atomically.
Already approved Tools, ToolSkills and helpers are prerequisites, not new review
candidates. Own Q1, observed behavior and human Q2 are required before activation.

Compatible A successors preserve contracts still needed by valid consumers for
new use. Unchanged consumers keep approvals without an extra A combination review.
The existing catalogue owner must connect successor subjects, compatibility
checks, review writers and all affected activation/task/restore/private/Prefix/MCP
readers before publishing that profile. Do not relax today's readers in advance
or append invented fields to the unchanged exact formats.

### S — controlled shipped qualification

The installation owner independently qualifies the Tool/native implementation,
then its ToolSkill binding, then Skill/implementing code together and consuming
workflows in dependency order. Reuse already qualified prerequisites. Retain exact
Skill-usage evidence; a closure list identifies tested bytes and does not grant
individual approval to every dependency.

S uses noncircular integrity/Q1/behavior qualification outside the normal authored
queue, with null human Q2 and no additional installed/external maintainer action.
A `source='system'` label cannot select S. An operator/Sempai edit of a shipped
component uses authored review. Mixed graphs retain each revision's provenance;
compatible shapes do not transfer exact S evidence to changed bytes. Upgrades
preserve overrides and retained historical selections.

These distinctions are specified in the
[own-functional approval contract](../../skills.md#own-functional-approval-and-successor-compatibility).

## 6. Publication and remaining work

Current ordinary publication uses complete qualified Recipe graphs. Independent
retention/submission does not prove independent unused-library activation. A new
Recipe needs its own workflow qualification, without repeating unchanged usage
Q1/Q2 per invocation. General independent pure approval and broader A publication
remain with the existing validator/catalogue owners; see
[the pure-review contract](../reborn/pure-component-review.md) and
[implementation index](../implementation-status.md).

Only intended MCP publications need a qualified MCP-call Recipe/variant and exact
command grammar. Exports/private helpers do not automatically become MCP tools.
The server's catalogue/chat bridge is separate from code loading: a listed command
enters ordinary chat, not direct Python execution. Discovery and in-flight command
contracts follow [skills.md's MCP contract](../../skills.md#mcp-and-acceptance).

## 7. LLM-relevant summary

A Skill is one Tool usage with prose and associated preloadable code. Load exact
declarations dependency-first with stable-UUID ties and no effects; invoke the
selected export with typed isolated data. ToolSkill binding grants nothing.
Associations, approval, workflow qualification, publication and live permission
are separate. Existing exact profiles remain valid; own-functional A successor
publication is still open. S requires verified shipped provenance and independent
prerequisite qualification, never merely a system label.
