# Shared pair construction drafts — 2026-10-26

Scoped acceptance: [terminal receipt](../../target/accepted-evidence/prefix-pair-construction-20261026/run-1/accepted.json), SHA-256 `c0787c0db04746247b0362bfddf1836b5b8bfd8921158d68f1c333443bb4874f`. The exact 22-file manifest, captured whole-source identities and copies, frozen six-file model inputs, actual commands/logs and retained worker/test images accompany the receipt. No formal predecessor run failed.

## Implemented vertical

V180 extends the existing immutable `reborn_prefix_design_proposals` owner rather than creating another registry. Only the new `brassclaw-prefix-pair-construction-proposal/1` format may have a NULL profile. Existing profile-specific proposals remain separately readable, while the old composite acceptance FK deliberately cannot accept a pair draft. No acceptance writer or active-design pointer was added.

The existing product service/facade exposes the instance-operator POST `/api/webchat/v2/prefix-model-designs/proposals`. Model designs imports a UTF-8 JSON draft under its captured specification and displays `pair_proposals` separately from role proposals. The original text is retained without JSON normalization, replacement decoding or silent BOM stripping; output is escaped text. Concurrent file events coalesce, invalid/oversized files fail without dispatch, and generation remains unavailable. Canonical provider/model limit editors and values are unchanged.

The strict data contract requires exact specification/artifact identities, original-file hashes and JSON pointers, a selected observed template-value hash, source-referenced Prefix/Dynamic/Special-token/Memory instruction blocks, explicit supported/unsupported modes and worked cases. It checks the literal fixed-prefix and prefix+formatted-prompt equations and mode coverage without executing a template, instruction or program. Current runtime cache facts must remain explicit null; supplied status/code/limit fields are rejected. The full supported field contract is in [the existing artifact guide](../reborn/prefix-model-artifacts.md#shared-pair-construction-drafts) and the shipped Prefix-Maker teaching reference.

## Actual checks

Six fresh Rust cases passed, zero failed/ignored:

- One pair-contract unit case exercises consistent Unicode literals, explicit Unknowns, invalid joins/references/pointers/coverage, missing fields, duplicate JSON keys and rejected settings/code/status copies.
- Two retained profile-proposal cases preserve the old v1/v2 contracts and current supported Sempai content rules.
- Two complete operator-descriptor cases verify the exact route table, methods, authentication/effect/body policy and additive route.
- One actual native HTTP/PostgreSQL/global-worker integration case (359.64 seconds) imports the six real frozen model files and checks the new full path: exact successful draft/readback/retry, unchanged role/settings/specification/artifact/profile-proposal metadata, foreign namespace rejection, incorrect original/template hashes and evidence pointers, incorrect literal joins, and rejection by the old role endpoint. Actual DB assertions prove NULL-profile storage, immutable records, the scope CHECK and rejection by the exact old acceptance FK. It also executes the existing packaged-reference/source-job/compiler/import/diagnostic paths.

Eight fresh UI cases and the existing offline Prefix-Maker compiler/export test passed. Strict affected-tests lint passed in 92.92 seconds, default composition lint in 26.40 seconds, format in 4.71 seconds; changed-file whitespace passed. Actual worker/composition/descriptor programs were preserved before Cargo exited, and all guarded source/model hashes matched at completion. There was no routine clean or target change. The original UTF-8 file-input UI harness was adjusted before this formal batch to distinguish its two separate inputs; no product workaround was used to make a fake backend pass.

The migration was exercised by the actual freshly migrated native PostgreSQL backend with legacy-profile records and the new draft. This is not a deployed in-place production-upgrade acceptance. The full native case is the existing public/store/package owner; the UI unit fixture supplements, rather than replaces, that real backend evidence.

## Explicit limits and next steps

Source existence/hash/pointer and literal-case consistency are structural evidence. They do not establish authoritative content quality, that proposed frames/prompt text match the default template or active provider mapping, correct tokenization, cache layout/reuse, context fit or model answer quality. The actual accepted, deterministic machine-readable formatter/options and compiler/Kohai readers are still missing. This draft cannot be used as an accepted design.

Future model-design generation must use the existing admitted Recipe/component owners to collect/archive provider and official documentation evidence, derive supported construction options, independently encode/qualify worked cases, retain the proposal and write trusted acceptance/bindings. Extend this same owner for the broader evidence/accepted-design contract; do not copy settings or pair formats into roles or silently execute instruction text. Ordinary Generate, Redesign, final joining/guards and serving selection remain open.

The native historical diagnostics remain unqualified for default-template serving and retain `fit=false`: Base's largest full probe is 169,223 tokens; Sempai's is 375,611. The tokenizer warning `375611 > 262144` remains visible. Nothing was shortened, activated, or made to fit by increasing a limit. No remote/provider/vLLM/Compose/venv/systemd/cache configuration was changed.
