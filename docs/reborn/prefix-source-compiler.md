# Recipe-owned source compiler

The source compiler is packaged as **unqualified component drafts**. It currently
produces quarantined, source-only base-prompt and Prefix-Maker packages. Generate
jobs, accepted model designs, target tokenization, activation and cache warming
are separate work; these drafts do not enable their WebUI actions.

`brassclaw_skills::prefix_compiler_components` supplies reusable PythonCode for
whole-source segmentation, compilation stages and receipt formatting. It reuses
`prefix_reference_components` for rendering, `utf8_digest_components` for one JSON
Tool usage, and `prefix_generation_components` for immutable retention. The
source compilation Recipe orders those components; Rust performs hashing,
storage, transport, integrity and kernel checks.

## Input and provenance

The `brassclaw-prefix-source-snapshot/1` input contains `profile`, `release`,
`header`, `evidence_view`, complete `sources` and three original `procedures`.
Each source declares its canonical relative path, kind, disposition, reason and
complete UTF-8 text. The seven component/architecture guides are mandatory;
Prefix-Maker additionally requires its dedicated reference. Component-library
coverage is not implied by including those guides.

The owner must build that snapshot from its retained source/design/code selection.
Caller-supplied procedure text does not prove which compiler ran. Keep the complete
retained graph and original procedures with the task; do not look up latest during
execution. The compiler preserves originals by content hash, full evidence cards,
source inventory, rendering/range data, procedure files and immutable manifest.
Literal v1 and exact-unit-deduplicated v2 are supported; neither silently summarizes
or clips originals.

## Execution

1. Validate the canonical positive decimal profile revision.
2. Prepare complete sources and the first ordered hash batch in PythonCode.
3. Hash through `digest_utf8_texts`, then prepare references in PythonCode.
4. Hash evidence identities, then render in PythonCode.
5. Hash all artifacts, then prepare the generation identity in PythonCode.
6. Hash the identity, then finalize the package in PythonCode.
7. Retain once through the existing immutable-generation usage.
8. Format the actual quarantined retention result as the internal task receipt.

Every hash and retention dispatch has its own ToolSkill/executable pair. Pure
stages have no artificial Tool binding. Runtime values remain typed data; the
compiler does not execute source references or turn them into Python programs.
Loading the library performs no usage. Hashes use exact UTF-8 bytes without
Unicode or newline normalization. One hash batch permits at most 8192 strings;
exceeding that technical bound fails the build. A qualified batching workflow is
required before claiming larger complete-library coverage; removing sources to
fit the bound is not supported.

The final string is `prefix-generation:<profile>:<retained package SHA-256>`.
It is neither approval nor proof of task completion. The admission owner must
verify the actual correlated immutable row, dispatch/effect records, cancellation
and settlement before reporting completion. A lost result never authorizes replay
of Retain or of the whole Recipe. Source-only manifests remain unmeasured,
activation-ineligible and cache-unverified.

## Development checks

From the repository root on Linux or macOS:

```sh
python3 -m unittest discover -s scripts/prefix -p test_prefix_compile_logic.py -v
```

This checks both profiles and both render formats against complete current guides
and independent SHA-256/original-byte assertions. It is CPython evidence only.
Native tests additionally inspect and run the same packaged definitions in Monty,
prepare the exact retained Recipe graph, and exercise registered hashing/retention
with PostgreSQL, duplicate retention, missing-source rejection and live policy
blocks. Those development tests create no approval or active catalogue entry.
Installation qualification must use the existing closed bootstrap owner and actual
durable receipts before production admission is enabled.

## Archive references and input evidence

New output inventories use `brassclaw-prefix-generation-sources/1`. Their
`artifact` fields resolve directly to the retained `raw/<SHA-256>` originals.
`input_artifact` points to `source_snapshot.json` for the Monty stages or
`input_sources.json` for the offline compiler. The former preserves the exact
canonical task snapshot; the latter preserves the exact original input manifest,
including its original formatting and input-package object names. Profile design
metadata is retained for literal v1 builds too.

Historical `brassclaw-prefix-source-package/1` output inventories remain readable;
their `artifact` fields belong to the input package namespace. Resolve their
archived originals by the established raw-by-hash layout rather than interpreting
those old fields as generation file paths. Immutable historical artifacts are not
rewritten. The importer checks new archive/input relationships, citation identities,
full ranges and counts independently of the generic package hashes. None of these
integrity checks qualify knowledge or execution.

## Internal Recipe completion

Compiler work produces a durable receipt instead of a chat reply. The task owner
selects one exact inspected program and typed input snapshot. The global Python
root reads that selection and runs its prepared Recipe flow; the Rust adapter
executes only the requested step. An internal task cannot resolve chat intents,
select an additional named Recipe, publish a reply or invoke unrelated root ports.
Its receipt must be checked against actual native answers and durable generation
bytes before completion is accepted. A nonempty string alone is insufficient.

The development observer uses that same Python flow in a bounded contained
worker and closes its private claim after shutdown/reaping. It retains observations
without granting bootstrap approval or exposing a production worker service.
Negative checks distinguish an intended contained Python rejection from transport,
resource or process failure. Native invocation records must cover the whole run,
match the selected Recipe/binding and observed argument/answer fingerprints, and
retain their actual attempt counts. Production Generate admission still depends
on closed installation qualification and durable job integration.
