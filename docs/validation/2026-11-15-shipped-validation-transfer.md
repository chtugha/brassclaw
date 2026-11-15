# Shipped validation transfer — 2026-11-15

## Result and scope

The canonical PostgreSQL/skills-db installation path inherits validation of
components developed and shipped with BrassClaw. Fresh installation and restart
do not run their Q1/behavior qualification workflows, create synthetic model or
tokenizer fixtures, or submit shipped seeds/global root for installed review.
Exact package/component/artifact identity and loading checks remain effective.
Authored changes still use their own review path; current Tool policy remains
independent of component approval.

`shipped-validation-transfer/1` records explicitly identify provisioning as their
validation origin and set `installation_execution: false`. They contain no
fabricated local observations. Inherited Q1 and behavior evidence have distinct
identifiers; system-seed association approval retains null Q2. Transfer authority
is private to canonical installation, not supplied by caller labels or an
instance-wide approval flag. Persisted revisions and program selections must
match the canonical shipped definitions exactly. Historical records, authored
overrides and retained task selections are preserved.

Explicit development tests can still execute actual native qualification. These
test-only entry points are separate from ordinary installation. Existing
qualification functions retain this development role.

## Binding documentation

The dated superseding contract is recorded in AGENTS.md, CLAUDE.md, the four
component guides, simplified_v3.md, validator_v3.md and development policy.
B1-05 and path_to_v3.5.md identify the changed qualification timing. The canonical
reborn-feature skill and its Claude mirror carry the same rule.

The historical C.2 builtin design already stated that builtins ship validated.
The later installation-owner wording assigned implementation responsibility; it
did not identify an additional person who must approve every installation.

## Validation

Final raw evidence is in
`target/validation/shipped-install-transfer-20261115-r3/`, including source and
artifact manifests, commands, logs, per-case results and `accepted.json`.
The exact tested Composition binary and Monty worker are retained under its
`binaries/` directory with SHA-256 identities in `accepted.json`.

Both strict Clippy checks passed with Rust 1.100.0: default library and
skills-db all targets, each with `-D warnings`. Native test and worker compilation
passed. All nine selected tests passed, with no failures or ignored cases:

- Private transfer scope and isolation from unrelated/spawned tasks.
- Fresh and repeated installation: zero qualification task admissions, review
  requests, component review submissions, model specifications and artifact sets;
  unchanged inherited records across restart and working source-review readback.
- Shipped seed boot with review-queue writes forbidden; historical reviews retained.
- Actual authored pending-review recovery failure still prevents successful boot.
- Idempotent root boot and rejection of a changed prompt.
- Retired L013 history retained without becoming an approval decision.
- Global runtime match/No-Match, MCP, policy, cancellation and restart regression.
- Explicit native encoding qualification with actual worker and durable journal.
- Explicit native shipped History qualification and reuse of original evidence.

The unit case ran inside the sandbox. PostgreSQL cases used dedicated temporary
instances outside it after a recorded sandbox `shmget` denial; no existing
database was used. Each fixture's orderly shutdown completed. No associated
PostgreSQL, worker or Composition-test process remained. All associated Screen
sessions ended in the recorded socket directory. Final desktop notification was
accepted and actually received in this chat.

Compiler and fixture inputs remained unchanged. Another chat added the standalone
`unnecessary_stuff_v3_at_bob1.md` report during validation; it is not referenced
from crates, Cargo.toml or .cargo. Its observed hashes are recorded as an unrelated
source-manifest delta, rather than claiming the entire repository was unchanged.
No rerun was necessary for this unembedded prose addition. Skill mirrors match
and `git diff --check` passed.

## Remaining work

1. If older installations contain pending requests generated for shipped seeds,
   define an exact migration that distinguishes those requests from genuine
   authored reviews and preserves historical audit bytes. New installations no
   longer create them; this change does not bulk-delete historical requests.
2. Consolidate superseded detailed qualification wording and preparation function
   names in older v3/v3.5 passages. The dated binding rule already resolves the
   timing conflict; terminology cleanup must not restore installed qualification.
3. Continue the separately owned general library, successor-reader and deployment
   cutovers. This acceptance covers the canonical installation path and selected
   runtime regressions, not complete B1/B2/B3 or whole-platform acceptance.

The component author performed no Git staging, commit or push during qualification.

## Root integration review

Root reviewed the connected producer, transfer, readback and test changes for
local integration on `main`, based on `73f85e79d387b201e627aa1edf67fabcfd7e9f99`.
All forty selected source/document files match the r3 source manifest exactly.
Mixed files were reconstructed in the index from their tested bytes; the shared
working files and the subsequent owner's frozen build inputs were preserved.
All other tracked compiler and fixture inputs match r3. The excluded Finder
`.DS_Store` metadata does not participate in compilation or the fixtures.

The existing strict Clippy and nine-test receipts therefore apply without another
build, startup or Modal job. This report is unembedded development documentation.
Later seed-review retirement, V216 and further terminology cleanup remain separate
uncommitted work and receive no acceptance from this receipt. Root's integration
is a local commit only; it does not publish a release or update the old Modal
product artifact. No push was requested for this integration.
