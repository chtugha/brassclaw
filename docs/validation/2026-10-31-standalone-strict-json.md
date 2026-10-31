# Standalone strict JSON usage — 31 October 2026

[The receipt](2026-10-31-standalone-strict-json.json) binds three centrally
reviewed/rebased sources, original Exact001 proposal, current application/native
images, fresh canonical worker build and the sole Screen run. All checks pass:
38 distinct tests, zero ignored, format and connected/default strict Clippy.

The reusable Skill draft uses one existing `builtin.json` operation and needs
four real Tool/ToolSkill/PythonCode/Skill identities, with no dedicated Recipe
identity. Its preloaded export calls `parse_strict` once with the entire input
text, then returns text using the declared `json.dumps` options. The native JSON
payload, exported text and original lexical input remain distinct. Input is
typed data and never becomes Python source. No new parser or Rust Tool is added.

The actual retained/Monty/kernel/PostgreSQL consumer checks eight cases: full
whitespace/instruction-looking Unicode input, arrays/finite numbers/null, nested
and escaped-equivalent duplicates, trailing/malformed input and live Tool denial.
Positive cases independently check native payload and export checksums. Negative
cases require the actual native terminal error and an incomplete step, rather
than passing on a later receipt mismatch. Original arguments, one attempt, no
model/reply, unchanged approval counts and policy restoration are checked.

The current global model ResultOnly/SDK and existing parser regressions pass.
The freshly built canonical worker is byte-identical to archived227a4b…; its
current-source build is now observed rather than inferred from a dependency
assumption. Thirty-four actual worker-service cases also pass, including task/
child lifecycle, cancellation, live settings and completed child-heap release.
BUILD-004's original progress and exact older-run observations remain unchanged.

These are unapproved reusable component drafts and a contained test consumer.
Execution does not create Q1/Q2/association/pure/workflow approval, ordinary
activation, MCP publication, Free proof, model-design or serving qualification.
The text algorithm is explicitly defined, not a claim of byte-preserving archival
canonicalization or unrestricted numeric representation. The model transport is
an SDK loopback fixture, not real-inference quality acceptance.

Shared contained model capture/metadata/native binding and separate mid-turn
Sempai ModelWork custody remain open. Current standalone model_host adapter
migration and the legacy deprecated DocType bridge/suppression still need their
existing compatibility/library owners. Full K3–K7, validator/library/prefix/
recovery/upgrade/platform/deployment/release work remains open. No tag, commit,
push or installation is claimed.
