# Configured inactive model targets — common-source qualification, 26 October 2026

Status: accepted for the unchanged five-file Prefix-owner package and its stated
metadata/UI boundary. No model-design acceptance, generation, activation, final
request guard or production serving qualification.

The existing model-target reader now includes registered provider defaults and
models with canonical configured limits even without an active Kohai/Sempai role.
It reuses ProviderDefinition.model_token_limits and the existing revisioned
settings owner. Selecting/capturing one target changes no active role. The real
operator HTTP/PostgreSQL case covers original target/revision/limit capture,
same model on another provider, unknown targets, stale revisions and immutable
retries. Six UI cases cover read-only selection/import, refreshed revisions,
late response fencing and configured inactive capture.

## Actual evidence and reuse

Final receipt:
`target/accepted-evidence/prefix-configured-model-targets-20261026/run-4/accepted.json`
SHA-256: `a8b26c13acbef6a14433d919e3507b810bc4ff40feaea23e1ee85ee70b1ddc8d`.
Five author-file hashes and the approved 20-document package (six release-indexed)
are checked in the current snapshot; the known Root-authored indexed changelog
entry is separately identified. This is validation, with no Prefix author edits
by Root. The normal current checkout and inherited one Cargo target were used.

R3 actually executed one positive native HTTP/PG/global/encoding case in 353.63s,
strict affected checks in 73.30s, current worker Cargo build in 0.46s and current
test build in 195.92s. The worker command reused the valid cached image; it was
not a claimed fresh compiler execution. Actual worker SHA:
`a617985957ccb8c5f3f0263f3ffe9135d9e352a86b8235d4bf5e39eba7e431a8`.
All six R3 UI cases passed too, but Root's external counter assumed TAP while Node
emitted the spec reporter. That R3 batch remains FAILED and its receipt/logs are
unchanged. R4 qualifies only the four positive R3 compiler/native checks after
exact relevant source MEMBERSHIP/hash equality, matching current documentation,
identical Python dependency freeze and retained executable hashes. R4 records
this explicitly in evidence-reuse.json; those Rust checks are NOT freshly rerun.

R4 freshly runs the six UI cases with explicit TAP output (all passed), default
strict Clippy (19.32s, -D warnings), formatting (4.51s) and whitespace. It makes no
zero-test, ignored-test or whole-workspace/platform/deployment claim.
R1's source drift and R2's genuinely missing native model-file prerequisite remain
FAILED. Neither is substituted for the native success above.

## Genuine model/oracle prerequisites

The native case uses the existing six immutable offline originals from
`target/validation/prefix-model-source-cacik5n7`, all checked against provenance
SHA-256 `03774b905c00ff5fb55cd2214c304887c30d2f5cdc3dfc22a7e5120b553143d6`.
No target2 script, build mirror, provider files, SSH, template installation or
service change was used. The historical runner was read only to identify its
actual prerequisites.

Global Transformers5.1.0 lacked qwen3_5 and produced a model-type mismatch warning.
The cause was corrected in a fully isolated test environment, without changing
global packages or immutable model files: Transformers5.20.2, Tokenizers0.23.3,
Torch2.10.0 and Jinja3.1.6. The actual Qwen3_5Tokenizer loads, dependency checking
passes, and the repository's real minimal HF oracle succeeds with zero stderr.
An initial inherited-package experiment exposed conflicts; complete isolation
resolved them. A writable private pip cache fixes its unrelated path warning.
The [official Qwen3.5 documentation](https://huggingface.co/docs/transformers/en/model_doc/qwen3_5)
and [published Transformers version](https://pypi.org/project/transformers/5.20.2/)
provide upstream context; the local checks prove this exact test environment.
No warning filter, model-type removal or generic-tokenizer substitute was used.

The complete current diagnostic base reference measures a largest probe of
169,299 tokens; the Sempai reference measures 374,136. Both diagnostic capacity
receipts are fit=false, with no design acceptance or activation. The genuine HF
warning for Sempai exceeding its declared tokenizer maximum262,144 remains in
the native log. No truncation or inflated maximum hides the missing fitted
model/profile design. Supplied-template mechanical comparison is historical
infrastructure, not qualification of request-owned default-template serving.

## Remaining ownership

The human-approved documentation separates pair design, profile content and
finished fixed prefix; establishes normal base default selection, the exact
Sempai/format/join order, linked canonical limits, generation confirmations and
concrete evidence-backed construction instructions. Applying/embedding these
contracts is not implementing their runtime design/serving behavior. The Prefix
owner retains that work and its next private source-evidence package. Shared
K3 Auth custody and K4–K7/full-plan acceptance remain open with their existing
owners. No Rust agent-loop fallback or new registry was introduced.
