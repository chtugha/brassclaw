# Budget-dependent prefix reference selection

The [shared contract](model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26)
separates the pair's format/limit design from the profile-bound reference-selection
rules below. The current profile proposal is not a second pair design/settings
owner. Its content rules need typed links to the shared accepted pair design and
canonical policy in the successor path. Base is the normal selection only without
an explicit Recipe/Orchestrator/role prefix; composites require explicit coverage.
The selected reference still needs generation-time target-specific formatting;
Kohai formats only the dynamic Orchestrator prompt first; a connected/enabled
Sempai corrects that formatted prompt and returns the final dynamic prompt. Kohai
then inserts the intended finished prefix unchanged exactly once before it,
without formatting the return again. Sempai's own call uses its own generated
prefix/target-pair design; the same pair shares the accepted design revision. No server template/configuration installation is implied.
These are remaining integration requirements, not new powers of this selector.
The [explicit handoff acceptance](model-token-limits.md#explicit-handoff-between-the-two-model-requests)
belongs to the Prefix owner; reference selection does not format Sempai's return
or insert a runtime prefix.

`scripts/prefix/prefix_design_selection.py` implements a deterministic, effect-free
selection stage. `brassclaw_skills::prefix_design_components` provides its retained
PythonCode definitions, the six-step `select-prefix-design-by-measurement`
Recipe and its optional captured-source variant described below. They reuse the existing native UTF-8 digest and model-encoding
usages. It introduces no provider call, settings store, Tool or approval shortcut.

This stage selects **reference text**, not a serving-ready prefix. An accepted
model design, canonical policy, trustworthy source snapshot and exact model
artifacts must be supplied by their respective owners. Actual chat injection,
semantic prefix/dynamic partition measurement, quality checks, composite fit and
the last pre-dispatch guard remain separate requirements. These definitions do
not extend the closed source-compiler bootstrap exception or enable Generate.

<a id="retained-model-design-declarations"></a>

## Retained profile-rule proposals (current storage facts)

The existing model-design proposal endpoint/store now handles the versioned
`brassclaw-prefix-design-proposal/2` declaration alongside historical version 1.
It preserves `profile`, `specification`, `rationale` and the exact original
`proposal_bytes`. Its `design` has exactly `adapter: "whole-unit-reference/1"`
and `generation_rules`, containing the rule object described below. The existing
table, provider/model specification reference and immutable proposal identity
remain the owners; no second registry, endpoint or model-limit record is added.

Retention remains **unaccepted-proposal**. The native declaration checker rejects
unknown/missing fields, invalid selectors, unsupported integer representations,
invalid inactive size bands, missing/cyclic dependencies and profile substitution.
Its graph check uses an adjacency/in-degree traversal without repeated full scans
or a group-count cutoff. Actual selector matches, unit ownership and mandatory
coverage require the captured-source Recipe and its independent authoritative
profile floor. A syntactically valid declaration proves none of those properties,
nor teaching quality, cache compatibility, envelope fit or serving readiness.

Version 2 contains no legacy `reserve_tokens` or copied model caps. Size bands
describe the design's supported domain; the canonical provider/model revision
continues to own its hard limits, even with optional budgeting disabled. A later
accepted binding must retain the original proposal identity and canonical nested
rule bytes/hash separately from each source-dependent resolved unit plan. It
must not rewrite the original proposal when canonicalizing its nested rules.

Version-1 proposals remain readable with their existing write checks. The legacy
complete-reference measurement capture explicitly rejects version 2 before job
admission; missing legacy geometry never becomes a zero/default reservation.
The WebUI keeps these proposals visible under model designs and explains their
exclusion from that diagnostic selector. Target-aware Generate, acceptance and
source/profile-floor bindings remain open. The four declaration/proposal tests,
two actual Monty/tokenizer integrations, existing HTTP/PostgreSQL caller with
v1/v2/retry/cycle/zero-admission cases, ten affected WebUI tests and strict affected
lints pass. See the [scoped successor validation](../validation/2026-10-24-prefix-budget-selection.md#versioned-proposal-successor--24-october-2026),
including its raw-log preservation limitation. These checks do not qualify a
complete accepted design or serving workflow.

## Design data

All JSON strings use UTF-8, sorted object keys, no extra separators and no ASCII
escaping. Two explicitly versioned input forms are supported.

`brassclaw-prefix-generation-rules/1` is the reusable rule set. It contains
`profile`, trusted `header`, ordered size `bands` and nonempty
`required_coverage`. Each band declares groups with `id`, `required`, `priority`,
`dependencies` and nonempty `selectors`. A selector has exactly:

- `path`: a canonical relative file or namespace;
- `match`: `exact` for a file, or `prefix` for a namespace ending in `/` (the
  explicit empty prefix selects the whole eligible inventory);
- `first_lines`: exact complete-unit first lines, or an explicit empty list for
  every unit under the selected path. No regex, generated Python or body slicing.

Every declared selector must match current source units. Overlapping selectors
inside one group form a union; different groups must own distinct units.
`required_coverage` uses the same selectors and requires all matching units to
belong to the mandatory dependency closure in **every** supported band. New
components under a declared namespace are therefore included automatically;
uncovered mandatory additions or disappeared/renamed required sources block the
build. A similar-looking sibling namespace is not included accidentally.
Resolved units sort by canonical source path and numeric original line position.
These checks enforce the supplied coverage contract; they do not prove that a
provider's proposed contract is adequate. Acceptance and every target build must
also compare it with the independent authoritative profile/source requirements.
An unreviewed proposal cannot weaken that floor by changing its own selectors.

`brassclaw-prefix-unit-design/1` is the explicit resolved form. It contains:

- `profile`: the selected profile identity;
- `header`: the reviewed trusted reference instructions;
- `bands`: ordered, nonoverlapping inclusive token ranges. Every band contains
  `minimum_tokens`, `maximum_tokens` and `groups`.

This latter form is the **resolved unit plan for one captured source inventory**;
it does not itself select newly introduced units. Normal target Generate must
bind reusable generation rules and retain their resolved unit plan separately.
The pure rule resolver is implemented; trusted accepted-design/database bindings
remain unfinished. With generation-rule input, `design_sha256` identifies the
unchanged rules and `resolved_design_sha256` identifies their source-dependent
resolution. The exact `resolved_design_bytes` are retained too. The explicit
`design_format` distinguishes these cases. Never label an ordinary content
refresh as Redesign, or accept a different rule set merely because its candidate
fits. Model/profile/consumer acceptance remains a separate prerequisite.

Every group contains `id`, boolean `required`, nonnegative integer `priority`,
`dependencies` (group IDs) and `units` (complete source-unit references). A smaller
priority sorts first; equal priorities sort by ID. All bands must have at least
one required group and valid, acyclic dependency graphs. Undefined or multiply
assigned units, missing dependencies, overlapping bands and invalid unused bands
are rejected. A budget outside the declared ranges requires Redesign.

Graph traversal uses adjacency/remaining-dependency counts and sorted waves.
It finishes each ready wave before sorting the next, preserving the earlier
candidate order rather than allowing a newly ready child to jump forward.
Closure uses a membership set alongside its ordered output. The
[large-graph successor](../validation/2026-10-25-prefix-graph-scaling.md) verifies
all four-group edge subsets, a 1,001-group chain in actual Monty and complete
candidate byte equivalence against the archived implementation. This changes
graph-checking cost, not knowledge selection, formats or model token limits.

`brassclaw-prefix-design-sources/1` contains `sources`, each with a canonical
relative `path`, exact full UTF-8 `text` and its `sha256`. Unit references are
`path#zero_based_index`, as produced by the existing `source_units_core`
segmentation. Executable files and fenced code stay whole; child documentation
units retain their complete ancestor preambles. New or changed sources require
coverage reconciliation against the accepted design; an old index is not proof
that a unit still fulfils its teaching obligation. This stage preserves every
unit's source hash and original line range for that subsequent check.
Dispositions describe direct unit cards. Ancestor preambles can additionally
appear as context even when their own card is not selected; use the complete
retained sources/resolved plan for that distinction, rather than treating
`not-selected` as proof that those bytes are absent from the rendered reference.

`prefix_budget` and `policy_ref` are immutable build inputs resolved by the caller
from the canonical provider/model revision. They are not editable limits stored
in a second location. Use the existing model-budget planner to derive the budget
from the actual prefix ceiling and context reservations. `special_tokens` comes
from the pinned model artifacts, including an explicit empty list when appropriate.
Untrusted source text and provenance labels escape these tokens; original bytes
are unchanged. The accepted trusted header owns its instructional formatting.

## Execution and output

Before accepting a proposed design, compare its required dependency closures with
an independently resolved profile floor. The existing `prefix_design_stage`
export accepts `phase: "coverage"` and canonical `data_bytes` containing
`design_bytes`, `sources_bytes` and `floor_bytes`. The floor has format
`brassclaw-prefix-coverage-floor/1`, the exact `profile`, and nonempty
`required_coverage` using the same complete-unit selectors. It must come from
the installation/profile authority, never from the proposing model's own rules.
The comparison rejects absent required sources, profile mismatches, malformed or
overlapping bands, and missing floor units in **any** band's mandatory closure.
Namespace selectors also cover newly captured components under unchanged rules.

The canonical `brassclaw-prefix-coverage-comparison/1` result retains the exact
three inputs, required unit identities and mandatory groups per band. It proves
only comparison of those supplied inputs. It does not authenticate the floor,
verify actual source digests, qualify source completeness, measure fit or approve
a design. Those gates remain with the existing capture/native digest/acceptance
owners. This helper is available in the reusable library; the six-step candidate
Recipe has not acquired an authoritative floor resolver or acceptance step.
The target-aware draft/Generate workflow must invoke it with its pinned trusted
floor before accepting the candidate. Removing a required unit to fit a smaller
budget must fail rather than weaken that floor.

The existing source-compiler library now supplies the pure `design-inputs` stage
and its typed `prepare_prefix_design_inputs` constructor. It reuses
`prefix_prepare`'s existing per-profile mandatory source requirements; there is
no second configurable list and no proposed-design input. Pass the exact
authenticated source `snapshot_bytes` and the native digest batch for the
ordinary preparation's complete `texts` list. It recomputes preparation from the
snapshot instead of accepting a caller-edited intermediate state, derives the
floor's complete-file selectors and emits included original texts/hashes as
`brassclaw-prefix-design-sources/1`. Excluded originals and disposition reasons
remain in the returned unchanged `snapshot_bytes`.

This stage does not itself authenticate snapshot provenance or digest results.
The existing captured-source/job/native result owners must supply them, and the
selection's digest verification must still run. Its retained source declarations
and a new compiler-stage constructor are not ordinary Generate wiring, complete
domain coverage, accepted design or approval. Defensive, Home Assistant and
tomedo remain unsupported by this core-source stage until their own capture and
coverage adapters are qualified.

1. Prepare the mandatory dependency closure in deterministic dependency-first
   order. Append optional groups, with their full dependencies, in stable priority
   order. Every candidate extends the previous candidate's text. There is no
   truncation, provider summarization or data-dependent change of teaching rules.
2. Hash the exact supplied rules/design, resolved unit plan, original inventory,
   original texts and complete candidate texts through the existing digest usage.
3. Encode every complete candidate through the pinned model-encoding usage, with
   truncation and padding disabled. Unit token counts are never added together.
4. Recompute the candidate bank and verify original digests, model/artifact
   identities, complete probe coverage and exact rendered-byte/hash matches.
5. Select the fullest fitting priority prefix of groups. All candidates are
   checked: real tokenizers can assign a longer text fewer tokens. If none fits,
   return `capacity-blocked`, preserving the complete measurement and disposition
   map. No empty success artifact is substituted.

The result is canonical `brassclaw-prefix-design-selection/1` JSON. It retains the
exact observation, policy reference, original-design/resolved-plan/inventory
identities, selected groups and units, complete selected text and token count.
Every original unit has a selected
or omitted disposition with its original hash and line range. A successful result
is labelled `reference-selected`; it asserts neither quality nor final-envelope
fit, approval or activation.

Native hash/encoding transport limits still apply. Oversized batches fail; this
implementation never silently drops groups or sources to meet a transport bound.
A qualified batching/composition Recipe is needed for larger candidate banks.

## Captured-source candidate variant

`captured_design_selection_drafts` optionally extends the same Recipe with a
`captured` variant while preserving the existing `selected` diagnostic variant.
It reuses source preparation, native digest usages, the profile-derived floor,
every-band comparison and the original six-step selection tail. The three native
usages have explicit binding/execution steps. Comparison output supplies the exact
design/source inputs to candidate preparation; a failed comparison stops before
candidate encoding. The derived serialized strings have truthful nonempty result
contracts so the retained compiler can establish those bindings before effects.

The final pure wrapper calls the existing selector and returns canonical
`brassclaw-captured-prefix-design-selection/1` with `selection_bytes`,
`comparison_bytes`, the original complete `snapshot_bytes`, and an explicit
candidate-only qualification. The selected inner result can be `capacity-blocked`;
the comparison and excluded originals remain present in that case.

This constructor and its retained assembly do not authenticate source/native
custody, accept a design or register an ordinary Generate job. Trusted admission,
exact model/policy/artifact bindings and qualification remain prerequisites.
Neither variant is a serving or final-envelope-size guarantee. The
[scoped source acceptance](../validation/2026-10-25-prefix-captured-selection-and-sources.md)
records actual native assembly, pure operations, browser scope and limitations.

## Development checks

With the real Hugging Face `tokenizers` package installed, run from the repository
root on Linux or macOS:

```sh
python3 scripts/prefix/test_prefix_design_selection.py
```

The regression cases exercise exact fitting boundaries, blocked capacity,
dependency closure, presentation bands, invalid unused bands, stable leading
bytes, complete fenced examples, ancestor context and measurement substitution.
They also check automatic component inclusion under unchanged rules, separate
resolved-plan identity, input-order-independent rendering, namespace boundaries,
coverage failures and malformed selectors. Actual Monty execution additionally
resolves an added component under unchanged rules.
WordLevel and Unigram measurements use actual tokenizers, including a real
nonmonotonic token-count case. The 1,001-unit inventory regression checks that
selection and dispositions preserve the complete source rather than applying a
row cutoff. The Cargo integration target `prefix_design_selection` additionally
checks the same Python in Monty, native Rust tokenization and retained Recipe
assembly/preload. Build that target with `skills-db,root-llm-provider` enabled and
the trusted packaged Monty worker alongside the test executable (in Cargo's
profile directory for an executable inside `deps`). These development checks do
not activate the new graph.
