# B2-R01 retained-selection contract repair — source handoff

2026-11-13. Package chat, isolated worktree
`/Users/ollama/.codex/worktrees/71a0/brassclaw`, branch
`codex/b2-r01-retained-contract`. Base Main:
`1bc98c0d284e4f966b7c1d72c48ade78deb39866`.

This is a self-reviewed source checkpoint. Nothing in this report claims Rust
compilation, executed unit/PG tests, component approval, catalogue activation,
installed uptake, full B2-02/Bob completion or deployment acceptance. Root owns
native acceptance, integration and push. No nested agents were used.

## Imported foundation and custody

`3a634bf1d` imports exactly the eight-file final B2 package from
`codex/b2-02-retained-selection-v2` / `e0be06d8fbe9c08abd551ac522034255a2e90826`
(code `78147448b`) with a conflict-free squash on current committed Main.
The original branch/worktree/history remain intact. The imported older handoff
is historical; this successor records the corrections and remaining limits.
Direct comparison with the old branch tip would include unrelated Main history;
only its actual delta from `9bb386191` was imported. B1's guarded writer and V214,
B3 UI/source, Root status/architecture documents and Cargo manifests/locks are
unchanged. The successor below renames and corrects the still unpublished V215; no applied migration is rewritten.

## Architecture and review resolution

Recipe/PythonCode/Monty retain sequencing and typed task data. This package is
passive wire, retention and reconstruction infrastructure. Skills owns one
version-dispatched structural contract, including static relationships to an
already retained exact snapshot. Existing revision/review/catalogue owners still
authenticate graph bytes, evidence, compatibility and activation. No label,
profile, parsing result or retained selection grants approval or Tool permission.
The kernel's live policy and the one-global-Monty boundary remain unchanged.

The four component guides govern these boundaries. The development policy,
current implementation index, v3.5 roadmap, Bob-2 owner plan, Engine/Skills/PG
maps and Composition guide were consulted. General independent approval,
qualified validator selection and coordinated new-profile publication remain
with their existing B1/B2/B3 owners.

1. `/1` no longer passes after a tag-only check. Its eight required base keys
   and two existing optional keys are closed, including nested wire structures.
   Unknown keys and `/3` extensions under `/1` reject. Absence, explicit Default,
   original whitespace/bytes and SHA-256 of those exact bytes remain distinct.
   Base revision checksums keep the historical lowercase-hex codec; new native
   component references retain their existing byte-array codec.
2. The first shared decode/build receives `ContractLimits`. The common bounded
   JSON reader rejects duplicates and classifies resource exhaustion as Capacity;
   the existing supported schema-depth ceiling remains. The retention store uses
   the returned Value and typed references rather than decoding all bytes again.
   There is no new quota, truncation or relaxed old boundary. Existing validator
   capture and byte-reconstruction callers now pass their captured policy.
3. The common owner validates Recipe/component references, exact root membership,
   one revision per UUID, canonical unique step order, variant wire form, typed
   input-layout schema, whole-value inputs, prior-result edges, invocation shapes,
   passive association/functional pins and model-input envelopes. After the real
   revision store authenticates the graph, `require_snapshot` checks actual
   variant/layout/model identity, exact linked step order, component classes and
   selected implementation/export. The original IBS step-link grammar/types move
   intact to Skills; Engine delegates and preserves its old API/error diagnostics.
   Advanced preload signature/behavior, typed producer/consumer compatibility,
   interface-checksum provenance and approval authentication remain with their
   existing owners; this does not introduce substitute validators for them.
4. All affected Rust sources, including inherited unformatted code, are directly
   formatted using Rust 1.100 rustfmt. No lint/warning suppression was added.

The ordinary compiler still emits `/1`. The existing full native `/3` reader/
publication cutover is NOT supplied here: in particular the existing validator
restore's exact `/1` reconstruction comparison still refuses `/3` rather than
silently dropping functional pins. No latest lookup, downgrade, Tier-2 or Rust
loop fallback was introduced. B1's authored writer remains protected closed.

## APIs and actual consumers

- `retained_selection_contract::parse_selection_version(bytes: &str,
  limits: ContractLimits) -> Result<SelectionVersion, SelectionContractError>`.
  Both variants carry a private validated `ParsedSelection` (directly for V1,
  nested for V3). `selection()` borrows its typed recipe/components/raw Value;
  `into_value()` transfers that already decoded Value to the real consumer.
- `SelectionVersion::require_snapshot(&self, &RetainedComponentSnapshot)
  -> Result<(), SelectionContractError>` performs shared static cross-checks.
- `build_v3_bytes(base: BaseFields<'_>, association_approvals, functional_approvals,
  model_input_binding, limits) -> Result<(String, [u8; 32]), SelectionContractError>`
  and Engine `build_v3_selection(..., limits) -> Result<RetainedRecipeSelection,
  RetainedSelectionError>` now require explicit captured limits. These new APIs
  have no ordinary production activation caller; the actual caller inventory is
  the Engine adapter, store and added tests.
- `RetainedRecipeInstruction::retained_selection_with_limits(ContractLimits)`
  preserves the existing canonical serializer. `retained_selection()` delegates
  with its historical default for unchanged callers. `RetainedValidatorCoverage::
  selection_bytes` uses the already supplied policy instead of a second default.
- `RetainedValidatorSelection::parsing_limits()` and
  `RetainedInstallationValidatorSelection::parsing_limits()` retain the actual
  policy used by their read. These are private-construction ephemeral fields,
  NOT wire fields. Both existing validator restore adapters pass it into exact
  reserialization. Both structs have one existing construction site, updated.
- `brassclaw_skills::retained_step_link::{StepRange, StepBound, parse_step_link}`
  owns the unchanged grammar. Engine's public range types remain re-exported and
  `instruction_builder::parse_step_link` maps to the same `IbsError::ParseError`.
- The real `review_selection_store::verify_program_selection` and the existing
  installation reader both use the same structural owner. DB graph selection
  remains `PgComponentRevisionStore::read_exact_in_transaction` under its existing
  snapshot policy. No Skills-to-Engine edge or Cargo dependency was added.

## Regression sources and checks actually performed

Fourteen grouped Skills unit tests exercise complete passive fixtures built from
actual immutable revision envelopes: `/1` bytes/optionals, closed fields, hex
base references versus native arrays, exact membership, isolated malformed
fields/identities, duplicate steps/keys, prior-result/input binding forms,
selected exports, both profiles, deterministic builder roundtrip, and original
limits above the old byte bound plus exact byte/node/depth failures. Every
mutation helper first verifies its complete positive baseline. These are wire
checks, not approval qualification. The new Engine suite has three real-PG cases:

- Actual Engine serializer and builder -> real retention store; checksum-correct
  unknown and `/3` fields under `/1` reject before insertion; exact original
  retention/readback and same-ID conflicting bytes are preserved.
- A real retained Recipe/selection above the old 262144-node policy uses its
  original existing preparation profile on first parse/store. Readback with a
  later smaller caller policy and exact recompilation keep the original policy,
  graph, bytes and checksum. No fabricated runtime success/evidence is used.
- The real V103/V106 tables keep all FKs/checksum/immutable triggers while their
  genuine predecessor format constraints are reconstructed in a private test
  transaction, then actual V215 SQL is applied. Old `/1` rows stay byte-identical,
  `/3` inserts work, unknown tags identify the exact successor CHECK constraints,
  bad checksums fail and immutable originals cannot be overwritten. These raw
  migration fixtures certify no Q1 outcome or approved workflow.

Actually executed: source/caller/feature inspection, conflict-free import,
small source-edit/identity scripts, direct rustfmt formatting and direct
`rustup run 1.100.0 rustfmt --check --edition 2024 --config skip_children=true`
over all twelve affected Rust files (exit 0), `git diff --check` (exit 0), and
an existing-migration/V215 byte comparison at the first checkpoint (exit 0).
That earlier V215 identity statement is superseded by the format-collision
correction below. No previously applied migration changed. No Cargo, build, test, Clippy, Modal,
source sync/probe, compiler-image operation or lock-file removal was executed.
The test source is not yet compiler/native evidence.

## Exact supervisor acceptance commands — NOT EXECUTED

Use the existing canonical target and OS FD lock, Root's source freeze/receipt
workflow and disk gate. Do not create another target. Skills has `db-store`,
Engine has `skills-db`, Composition uses `postgres,skills-db`; there is no guessed
`integration` flag for these selectors. The native test rig supplies real PG.
Existing Composition/worker checks need their actual retained trusted companion.

```text
cargo test -p brassclaw_skills --lib selection_contract_tests -- --nocapture
cargo test -p brassclaw_engine --lib memory::instruction_builder::tests -- --nocapture
cargo test -p brassclaw_engine --features skills-db --test retained_selection_contract -- --nocapture --test-threads=1
cargo test -p brassclaw_engine --features skills-db --test retained_instruction model_input_binding_is_pinned_with_exact_recipe_and_never_added_to_legacy_selection -- --nocapture
cargo test -p brassclaw_reborn_composition --features postgres,skills-db native_qualified_pure_source_restore_preserves_original_preparation -- --nocapture --test-threads=1
cargo test -p brassclaw_reborn_composition --features postgres,skills-db native_qualified_pure_source_upgrade_preserves_v1_and_rejects_unqualified_v2 -- --nocapture --test-threads=1
cargo clippy -p brassclaw_skills --features db-store --all-targets -- -D warnings
cargo clippy -p brassclaw_engine --features skills-db --all-targets -- -D warnings
cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings
cargo test -p brassclaw_architecture
```

Require nonzero selected counts and actual backend/worker execution. Root may
bundle compilation and reuse unchanged artifacts/evidence according to the
policy; these commands are the relevant acceptance scope, not permission for the
package chat to execute them. Other ordinary/MCP/child/private/publication `/3`
readers, full Q1/behavior/human Q2, historical native images, installed upgrades
and durable recovery remain unqualified/unimplemented in this package.


## Binding format-collision correction after Root review

The original source checkpoint `546f153b2` is superseded for format ownership.
Root identified a missed private-codec inventory. Read-only inspection of Q/N at
`8ed62ca7b` confirms an existing **reserved** `monty-retained-recipe-selection/2`
codec in Skills `retained_selection.rs` and Engine `retained_instruction.rs`.
It carries exact capture preparation bytes/checksum, flat/structured execution
profiles, optional flow/workflow qualifications and pure qualifications. Its
functional own approvals are an ARRAY with hexadecimal revision references.
The rejected B2 predecessor claimed that same tag for a MAP with native array
references and no Q capture/profile envelope. They are incompatible; neither
private code nor a format claim establishes readiness, approval or acceptance.

Read-only `git grep` inventory over all 72 local branch tips finds retained `/1`
and `/2`; `/3` occurs only in this package's previous rejection fixtures, not an
existing positive codec. Thus this package now reserves **`/3`**, independently
of Q's exact `/2` meaning. Names are `FORMAT_V3`, `ParsedSelectionV3`,
`SelectionVersion::V3`, `build_v3_bytes`, Engine `retained_selection_v3.rs` and
`build_v3_selection`. There is no compatibility alias advertising the rejected
B2 `/2`, and no decoder silently relabels Q bytes. The `/3` flat own-approval
profile remains a bounded successor of the existing `/1` base, not a lossy
conversion of a captured Q `/2` task. Ordinary production emission stays `/1`.
An authored function changing only its Skill does not require an invented own
approval for unchanged implementing code/S prerequisites. Per-root wire carriage
remains passive and actual B1 context/compatibility authentication stays required.

Shared base implementation/adoption boundary:

```text
validate_base_wire(raw: &Value, limits: ContractLimits)
    -> Result<ValidatedBaseWire<'_>, SelectionContractError>
ValidatedBaseWire::recipe() -> ComponentRevisionRef
ValidatedBaseWire::components() -> &[ComponentRevisionRef]
ValidatedBaseWire::require_snapshot(&RetainedComponentSnapshot)
    -> Result<(), SelectionContractError>
```

This borrowed boundary validates the existing flat base and preserves the source
Value. Format tags, capture, flow and new evidence fields are deliberately left
to their version owner. It never returns a versioned selection or a qualification.
The `/1`/`/3` decoder invokes the SAME internal base checker once after its bounded
JSON decode, and both public base view and versioned reader use the SAME static
snapshot helper. A later Q adoption must reuse this boundary rather than retain
its parallel base/ref/order logic. `recipe-input-layout/2` and structured flow
remain explicit missing support at this boundary; extend this same owner with
qualified schemas before adoption, without dropping Q fields or lifting guards.
No Q/N/R/T Rust runtime, private approval writer or migration was imported.

Adjacent inventory: `component-validator-selection/1` and `/2` already exist in
Main/Q with their actual outer preparation/source-qualification meanings;
`installation-validator-selection-record/1` is unchanged. Q additionally reserves
`monty-flow-qualification-subject/1`, `monty-flat-qualification-subject/1`,
`recipe-input-layout/2` and `recipe-flow/2`. This package allocates none of them
and does not change any outer-format claim into approval. Its unknown-version
fixtures now use `/999` so `/3` is not simultaneously called unsupported.

DDL is now `V215__retained_selection_v3_format.sql`, with named `/1`-or-`/3`
constraints. Before dropping anything, it explicitly refuses an existing `/2`
format gate (`23514`, `retained selection /2 requires coordinated format adoption`).
It neither accepts unknown Q shapes nor silently removes or narrows a Q gate.
The real-PG migration regression includes this exact guard and unchanged original
rows. Private Q/N already has unpublished V214 structured-custody, V215 capture
policy and V216 catalogue-original SQL. Main V214 remains B1's protected applied
owner; V215 is this package's reservation. **Root must coordinate the private
migration-number conflicts and the eventual combined format gates before adopting
that separate stack.** This package does not rename or rewrite Q's files.

New passive regression: a full flat base with Q-shaped `/2` capture/profile and
ARRAY/hex pins passes only the shared base boundary, unchanged. This versioned
reader rejects it, rejects relabelling it `/1` or `/3`, and refuses structured
input-layout `/2` at the flat boundary. This is not a positive Q qualification.
The native test selector remains `--test retained_selection_contract`; its three
case names now include `selection_contract_pg_v3_first_parse_and_original_limits_survive_reduction`
and `selection_contract_pg_v215_upgrade_preserves_v1_bytes_and_integrity_guards`.
All Cargo/PG/Clippy checks listed above are STILL UNEXECUTED. Subsequent direct
format/whitespace and source-name/format inventory are the only new checks.

Required Root plan precision: replace B2-02's tentative own-carrier `/2` name by
this bounded `/3` and its explicit MAP/native wire, record Q `/2` as a separate
reserved capture/qualification contract, and route future common base/static
reader adoption through this Skills owner. Q capture/profile/ARRAY-hex semantics
and old `/1` bytes must remain exact. Full connected readers, B1 trusted approval,
qualified publication and any combined captured own-functional profile remain
separate prerequisites, not consequences of parsing or this renaming.


## Root review corrections — 2026-11-14

`7711c2052` remains the format-ownership checkpoint; this correction additionally
resolves its Public-export and Base-copy findings before any Cargo execution.
The `/1` and `/3` wire schemas, optional-field presence, model/variant/layout
meaning, exact original byte/checksum domains and Q `/2` reservation do not change.
There is no compatibility alias or new format allocation.

**One name-grammar owner.** Only the small passive
`brassclaw_skills/src/preload_identifier.rs` from pinned private Q source
`8ed62ca7b` is reused. A source comparison proves its `identifier(&str) -> bool`
body identical to Main's previous Engine function. Engine's public
`retained_preload::identifier` API now delegates to it, preserving private/helper
symbol semantics. The contract's existing `is_valid_python_identifier` name is
a downstream alias of that same general rule; its copied keyword list is removed.
No Q runtime, approval or other stack source is imported.

The shared `public_export(&str) -> bool` adds the existing Main restriction
`!name.starts_with('_')` to the general rule. The actual Engine
`PreloadDeclaration::parse` public-name gate and all three selection sites
(functional pin parsing, explicit invocation maps, exact-snapshot default-export
resolution) call this one function. General implementation symbols, private
helpers and constants continue to use `identifier`, including valid leading
underscores. Early rejection of an invalid private Public-export selection does
not reinterpret it as another historical export or change any valid old pin.

**Borrowed Base view.** `ParsedSelectionV3` no longer stores an owned Base map.
`parse_value` does not construct that map for either `/1` or `/3`, so the ordinary
historical parse no longer clones a potentially large input-layout/constant tree
and immediately discards it. `/3` also keeps only its existing raw Value, exact
bytes and necessary typed reference/pin records, with no second Base graph.

```text
ParsedSelectionV3::base(&self) -> BaseFields<'_>
BaseFields::from_value(&Value) -> Result<BaseFields<'_>, SelectionContractError>
BaseFields::get(&self, &str) -> Option<&Value>
build_v3_bytes(BaseFields<'_>, association_approvals, functional_approvals,
               model_input_binding, limits) -> Result<(String, [u8; 32]), SelectionContractError>
Engine build_v3_selection(BaseFields<'_>, association_approvals,
               functional_approvals, model_input_binding, limits)
               -> Result<RetainedRecipeSelection, RetainedSelectionError>
```

The view is a borrowed projection. A fresh builder view requires exactly the
seven base fields; it cannot silently strip extra capture/format metadata from
an arbitrary record. Builders still create a new document through the existing
canonical Value serializer, then release that newly assembled tree before the
single validation decode. This is separate from retaining a duplicate Base tree
in a parsed original. The real retention store continues to move the already
parsed Value, without a second decode. All actual builder callers are updated;
the Engine native fixture uses the borrowed view and releases its superseded
fixture Value. No artificial heap cap, allocation-count assertion or synthetic
OOM test was added. Existing above-old-byte and real-PG above-old-node regression
sources remain unchanged in their limits and behavioral assertions.

Additional tests are source only: the fourteenth Skills group isolates private
pin names (using a valid default-export baseline without an explicit invocation),
private explicit invocations and private exact-graph default exports. Valid public
names and valid private identifiers remain positive. A real
`PreloadDeclaration::parse` unit case accepts a public name with private
implementation/helper/constant symbols, while rejecting `_private`/`_helper` as
public names with all other fields unchanged. This is parser evidence to execute,
not source/behavior qualification.

Additional required supervisor selector, alongside the preceding acceptance set:

```text
cargo test -p brassclaw_engine --lib executor::retained_preload::tests::public_names_reject_private_selection_but_keep_private_implementation_and_helpers -- --exact --nocapture
```

Latest checks: direct Rust 1.100 rustfmt on all fourteen cumulative affected Rust
files, whitespace and source/caller/grammar-identity inspection only. Cargo,
Clippy, all unit/native/PG tests and Modal remain UNEXECUTED. Full reader
activation, trusted approvals, migration adoption and native acceptance remain
with Root and the existing owners.
