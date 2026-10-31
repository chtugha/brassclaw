# K3 filesystem secret generation CAS — scoped acceptance, 2026-10-26

Accepted receipt: `target/accepted-evidence/k3-secret-cas-aba-20261025/run-4/accepted.json`, SHA `64ff6fc625d960663c142fad2a3904f298007e9f0d6121156eec1357bfe75f83`. The evidence directory retains the original package start date; final acceptance is26 October.

904 distinct positive Rust cases: 216 freshly executed in final run4 and 688 exactly qualified run3 cases. No zero-case or ignored result is behavioral acceptance. All21 applied source/test/embedded hashes match the final receipt.

## Behavior and actual owners

- Generic `VersionAndBody` checks the same selected version and encrypted body inside native InMemoryBackend mutex custody or the existing PostgreSQL transaction/row lock. It detects delete/recreate that restarts a path at version1. It does not identify metadata or an identical restored body as a new physical lifetime.
- FilesystemSecretStore captures body identity from the same backend read as its version and authenticates the existing encrypted reference. Stale body/version returns no write; actual errors remain errors. Existing secret formats, AAD and encrypted reference algorithms are unchanged.
- Explicit EntryBodyCas support is checked by mount-capability validation. Old Any/Absent/Version wire forms remain exact; new compound values round-trip and Debug/errors redact bodies/checksums. Local unsupported conditions cause no writes.
- Secret/session CAS and resource snapshots preserve unsupported preconditions as errors, with no unconditional Any retry. Actual Local transition and public persistent resource-limit callers preserve original bytes after rejection. Other legacy compatibility helpers remain explicitly in existing K3 step6.
- The existing LoopSupport architecture walker prunes excluded directories before traversal and propagates source-read errors. The actual same four-case target now finishes0.51s versus215.61s; this is a local observed target comparison, not a platform-wide performance claim.

## Execution and correction history

- Run1 compiled the connected package; one new raw-contract path failed known-root validation. Both new invalid fixture roots were corrected, retaining the failure log.
- Run2 passed the actual memory ABA and Local checks. The existing architecture walker traversed excluded target/evidence. Its correction changed source, so the runner rejected run2 source stability rather than accepting it. The attempted termination found the old child already gone; no test result was fabricated.
- Run3 rebuilt/reran actual storage/consumer checks with native authorization integration enabled and distinct identically named test targets preserved. Its683 cases and five actual native secret-reference cases passed. It then selected zero cases with a filename-shaped filter; that result was rejected.
- Run4 changes runner filters only, verified against the compiled test list. No protected source or compiled artifact changed. Its688 positive prior cases are explicitly qualified; remaining actual native/public/global/architecture tests and current strict checks run freshly. No repeated storage or composition build was needed.

Fresh final groups:

- `actual-composition-manual_token_flow-tests`: 3.
- `actual-composition-product_auth_durable-tests`: 48.
- `actual-composition-pg_auth_product_services-refresh_tests`: 18.
- `actual-composition-oauth_provider_client`: 13.
- `actual-composition-oauth_dcr`: 19.
- `actual-composition-llm_config_service-provider_login`: 8.
- `actual-manual_tokens`: 10.
- `actual-webui_v2_product_auth`: 42.
- `actual-webui_v2_product_auth_4201`: 41.
- `actual-auth_callbacks`: 7.
- `actual-auth_lifecycle`: 5.
- `actual-global-current-packaged-content`: 1.
- `actual-reborn-dependency-boundaries`: 1.

Checks:

- `affected-root-consumers-strict-lint`: passed (92.08s).
- `default-composition-lint`: passed (27.40s).
- `final-format`: passed (4.68s).
- `diff-whitespace`: passed (0.19s).

Cargo1.96.0 is serial under the common lock and uses the configured `/Users/ollama/brassclaw-target`; no target2 or compile mirror. Actual source/features/commands and retained executables are in source/progress/change-manifest/qualification records. The then-retained worker has SHA `d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`. Its inherited qualification checked529 listed paths plus the accepted lock delta. **Follow-up correction,26 October:** that list omitted actual normal/transitive Resources/Common/ModelEncoding/Filesystem dependencies and therefore does not establish complete current-worker source equivalence. The global case is an observed result with that retained binary; a newly built current worker and fresh global caller acceptance are required in the next connected package. This limits worker applicability, not the independently executed native storage/CAS/public results, and does not rewrite the historical receipt. Prefix subsequently built the actual current `ec89af721b3d35e95f5cdf398b955389d2c8dc39c5f168fdd1c78cdfae66c6cb` worker and passed its ordinary global Match/No-Match caller; see [that scoped report](2026-10-26-prefix-token-partition.md). Root will compile the worker again after its next filesystem contract change and retain a complete conservative common-source inventory rather than reuse the incomplete list.

## Remaining boundary

This closes the existing K3 step2 write-CAS ABA prerequisite. It does not implement immutable original/retained manual input, account/flow/interaction/result publication, all-writer deletion fencing, lease-generation capture, DCR disposition, issuer expiry, setup/Codex, remaining fixed-setting/compatibility consumers, K3–K7 or full-plan/platform/deployment acceptance. Existing RootFilesystem delete remains unconditional recursive deletion; single-leaf selected deletion and full custody remain explicit existing step2 work. No commit/tag/release/SSH or component activation is claimed.

### Current-worker follow-up closed,26 October

The [selected-entry deletion acceptance](2026-10-26-k3-conditional-entry-delete.md)
now genuinely builds the actual common-tree worker and freshly executes both
retained global Match/No-Match and live model selection callers. This supersedes
the missing current-worker equivalence proof for the current tree; it does not
retroactively turn the historical530-file list into a complete dependency set
or rewrite the904-case receipt. Its independent native CAS regression also runs
again with current native code. Full plans remain open.
