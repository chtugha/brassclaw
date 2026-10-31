# Exact K3-2 implementation — partial source delivery, 29 October

The operator requested implementation of
[Exact K3-2](../plans/Exact_V3_definition_update_at_K3-2.md), authorized coordination
with the active Prefix owner, and confirmed protection of referenced generations
until their live references are released. This is a progress receipt, not complete
Exact acceptance, installed uptake, a serving activation or deployment.

## Delivered source

Thirteen documents have additive future-consumer sections. Their pre-existing
bytes were preserved, including all five protected plans. Original-byte SHA-256,
added-span SHA-256, exact new headings and bounded test receipts are in the
[input manifest](2026-10-29-exact-k3-2-source-progress.json). The original preimages
remain under `/private/tmp/brassclaw-exact-k3-2-docs`. All thirteen prefix/preimage
comparisons and local file links passed. No Cargo was run for documentation.
The pending D14 acceptance matrix and propagation of the subsequently confirmed
retention detail await the existing embedded-document source window; the older
append's description of that detail as proposed is superseded by Exact §1.1.

The three fixed B1/B2 prompts are extracted exactly from Exact §4.3 into
`scripts/prefix/bootstrap/v1`, with a closed recursive answer schema and SHA-256
manifest. The response schema is shared and factored rather than repeating the
entire object for every field/status; its canonical request representation is
3,111 bytes. It does not replace native cross-field/provenance checks.

`scripts/prefix/bootstrap_answer.py` provides one pure typed-answer validator,
RFC-6901 source-view resolution and quote/scalar checks. Unknown values never
become zero. Text quotations alone require semantic review; different sentences
do not establish contradiction. Scalar type/value mismatches, extra fields,
wrong echoes, incomplete/duplicate requirements and invented candidates fail.
Returned values are not import permission. Native dispatch correlation, exact
numeric bounds, source hashes/target/scope, option applicability/protected values,
CAS and persistence must be satisfied by the consuming retained workflow.

`crates/brassclaw_skills/src/prefix_bootstrap_components.rs` contains pure class-22
draft constructors using the existing retained input/result and model-prompt
profiles. **This new file is outside the module tree, uncompiled and unseeded.**
It creates no Tool, queue, model client, prefix bypass, approval or active path.
Its two Rust tests are written but have not run. The current JSON Tool's ordinary
`parse` operation uses `DuplicateKeys::LastWins`; it must not be substituted for
the existing native `value_contract::strict_json` at the raw response boundary.

## Checks and limits

`python3 -m unittest discover -s scripts/prefix -p test_bootstrap_answer.py`:
15 passing cases. They were additionally exercised with the existing contained
Monty utility worker: 53 evaluations, 40 expected rejections, no test failure.
This is pure algorithm/protocol coverage, not an admitted provider execution.

The actual Rust constructor's format literal was separately rendered and tested
in that worker: twelve evaluations cover B1/B2, exact fixed instructions, hostile
source text remaining data, extra packet fields, foreign response schema,
unknown task, missing identity and invalid hash. This proves emitted Python behavior, **not Rust compilation**.
The exact worker/source hashes and scoped counts are in the manifest. Utility
scripts remain `/private/tmp/brassclaw-exact-bootstrap-{monty,prompt-monty}.py`.
Rustfmt passes for the new Rust file. No existing build input/module export was
changed for these checks; no installation, DB, provider or deployment was touched.

## Current integration constraint and remaining work

The active chat “Fix base_prompt composition on brass” explicitly holds a source
freeze over its connected Prefix acceptance, final measurement, gateway,
admission, response/catalogue, review and WebUI-facade package. It will supply a
terminal interface/source handoff; the Auth owner has the next serial build slot.
No Cargo, module export or embedded-source edits may interfere with that window.
The Prefix owner was notified of the thirteen earlier document appends and will
capture their new hashes, rather than reuse old exact-byte evidence.

After that handoff, integrate against its actual qualified successor: finish
I1's original Use→discovery persistence, I2's existing three-state retained binding
and mandatory prefix-free final measurement through children/reviews, I3's
noncircular design generation/acceptance, I4's real facade/UI, I5's integrated
generation Q1 and separate human activation, I6's now-confirmed reference-safe GC,
and I7's connected acceptance. The pure files do not close any of those packages.
Do not introduce a placeholder native approval, empty prefix, permissive guard,
second parser/executor or current Prefix-Maker requirement to connect them.
