# Bob 3 Codex/Auth custody — isolated worktree

Baseline: `0f1236a6c8a0f9d5ba4d7145bd040bf8c4576149`. Branch:
`codex/bob3-auth-custody`, worktree `/Users/ollama/.codex/worktrees/e8ba/brassclaw`.
The user's explicit second-chat/worktree/Modal exception applies. No subagents,
local Cargo, push, merge, release, provider configuration edit or human issuer
login was performed. B1/B2/driver/Q1/approval sources are outside this patch.

## Source scope

- Default PostgreSQL Auth now uses the existing native owner after a transactional
  filesystem-custody import. V204 reserves the installation cutover marker and
  old-writer fence. Original files/versions remain unchanged. Typed scope and
  canonical mount resolution authenticate each source location. Accounts, flows,
  interactions, original refresh actions and requirement selections retain IDs,
  handles and timestamps; native conflicts abort the whole import. A restart
  never overwrites newer native state from archived bytes.
- In-flight filesystem callback/refresh claims, DCR work and incomplete CAS-only
  publication decisions block cutover visibly. Their original bytes remain
  available to the old owner for reconciliation. This deliberately supplies no
  fabricated outcome or unsafe live-writer handoff. Those interruption cases
  remain installation acceptance gates.
- `RebornProductAuthServicePorts::from_composed` explicitly carries an existing
  facade's sealed native owner/host pair through the injected branch. Generic
  trait bundles retain typed native unavailability and never adopt another pool.
- Startup, reload, primary/Sempai preparation and task/child construction carry
  the same internal native session-preparation capability. Missing native owner
  or credential head errors only for the selected Codex protocol. Other provider
  connectors remain independent. There is no SDK file/environment fallback.
- Native refresh uses `CredentialRefreshAction`, its existing host-instance
  admission and result/publication transitions, and the actual PG secret owner.
  One generation-derived original includes login/head/client, account, access and
  refresh generations; separate callers/processes discover the same admitted
  action. A new access generation remains refreshable even without token rotation.
- Admission commits before HTTP. The existing host original-egress sink commits
  the exact final encrypted request before permitting transport and retains the
  complete original response before parsing. Tokens, account and journal publish
  atomically under account → action → head/secret locks. The new secret integrity
  primitive locks/compares a selected head without rewriting its generation.
  No DB lock spans HTTP. Old responses cannot overwrite a different head/account.
- A recovered action only reads/reconciles its existing outcome. Missing or
  uncertain responses stay unresolved; no issuer/model replay, new random refresh
  identity or guessed completion is introduced. Concurrent pending callers receive
  a pre-submission recovery error and can independently read back later; no new
  polling timer/quota/scheduler was added. Only the winning execution, after its
  preflight has actually returned and the journal is locked, may classify absent
  request custody as `NotSubmitted(HostPreparationFailed)`. That original stop
  preserves the account and grants no redispatch. An installed request without a
  retained return remains unconfirmed; recovered callers cannot classify another
  live driver's absence as no-send.
- Host protocol parsing requires a positive actual `expires_in`, rejects explicit
  null/empty refresh material and preserves an omitted refresh token under the
  existing OAuth refresh contract. Standalone SDK file-session behavior is separate.
  `SingleSubmission` model dispatch code remains unchanged.

## Actual evidence

Real PostgreSQL 17.11 SQL acceptance passed with the filesystem owner's exact
V26/V27/V28 schema, exact V204 SQL and the importer's exact trigger-installation
DDL. Existing UPDATE/DELETE and new INSERT attempts into transferred Auth custody
were rejected; original bytes stayed present; another tenant remained writable.
Receipt: `/private/tmp/brassclaw-bob3-auth-sql-check.json`; runner:
`/private/tmp/brassclaw-bob3-auth-sql-check.py`. V204 SHA-256:
`b5f40e5df73a06ef4cdbb2ecf0de083405805972f6040bf96c2ced4c3fe48226`.
This accepts the actual SQL fence, **not** Rust importer/factory/runtime behavior.
The sandbox initdb attempt failed; the authorized isolated native PG rerun passed.

Modal SDK/skill 1.6.3 and activated Apple image
`im-S1cpT9G1fsMPpwjISvBSlK` were reused with the qualified 8 CPU/32 GiB/8 jobs,
900-second/nonpreemptible settings. No image provisioning or sizing job ran.
The controller owns `/private/tmp/brassclaw-modal-build.lock` throughout sync,
submission and receipt collection. Native target was never assigned or changed.

First actual Modal job `affd31af-6c35-45e7-a4ff-d2702e67c55c` failed compilation
(returncode 101, 252.95 seconds) on the response-custody argument order and a
non-Send form serializer across await. Both causes were repaired together.
Manifest SHA-256: `f534320ee82ffc41eb895314afd37b554f7295f598b6e9105654ae1db874b50d`.
The earlier wrong controller subcommand and wrong Python interpreter failed
locally before submission and are not build evidence.

Second job `3f6ee067-6c3d-4cda-887b-f808f7a0b966` passed the connected importer,
refresh and parser compilation/link (390.94 seconds). Manifest SHA-256:
`e2580f863b0fc6969b3c2be79368288d285a7166d0b6ce65c772cf41ea507e9f`;
intermediate arm64 binary SHA-256:
`d551228a1fb59e03a0d9cfbc643a80af4f6855e4c65b64fb55c253c3e258f21b`.
It exposed one previously suppressed unused challenge-expiry field; its production
consumer now checks that expiry against the current nonterminal native flow before
showing a code. This intermediate artifact does not cover the later injected-port,
no-send and additional migration integrity refinements. The next bundled build `5a57353c-8738-4146-a730-f90d0414ffe4` passed without
warnings (worker duration 467.62 seconds; actual Cargo 6m33s). Manifest SHA-256:
`b668c855e8e65b4b9c4994fa095a52afd9626d6547d4a692437cefbeb1a4234d`.
Arm64 executable SHA-256:
`44636f83ee386c5b5693bda7aa83714df29865a218f0543c5a6561f2e5fb74bc`.
Native `file`, `otool -L`, actual `--version` and `--help` all passed through
`accept-native`; `native-accepted.json` retains the exact commands and scope.
This is packaging/CLI startup acceptance, not a product login/refresh test.
Receipts live under `/Users/ollama/brassclaw-target/modal-build/jobs/`.

After this snapshot, the native form construction was consolidated into the
existing OAuth owner's `native_refresh_body` (reusing its existing `refresh_body`
encoder), and the cross-process/migration test sources were completed. The two
runtime files affected by that final small consolidation are
`oauth_provider_client.rs` and `pg_auth_codex_refresh.rs`; that delta is reviewed
and formatted but **not yet compiled**. The leading chat explicitly requested
this branch for its next combined native validation; no redundant additional
paid build was started. All tests remain uncompiled/unexecuted here, including
those captured by a non-test build manifest. See the exact [source hash map](2026-11-03-b3-auth-custody-sources.json)
for baseline/final/captured bytes. No old/intermediate executable is claimed to
cover a later source interface.

## Native test handoff — written, not compiled or executed here

Use composition's real `postgres,skills-db,root-llm-provider` feature combination
(the standalone CLI defaults propagate these). Required focused filters:

- `pg_factory_explicit_tenant_reuses_native_auth_owner` — now also the explicitly
  injected composed facade; generic injection rejection remains in the existing
  `local_dev_runtime_auth_interactions_are_unavailable_without_flow_record_source`.
- `pg_factory_default_tenant_attaches_native_auth_after_custody_migration`.
- `native_default_auth_migration_retains_accounts_flows_interactions_and_fences_old_writers`.
- `native_auth_migration_malformed_original_rolls_back_without_cutover_or_source_loss`.
- `native_codex_refresh_two_owners_share_one_generation_and_omitted_refresh_is_retained`.
- `native_codex_refresh_invalid_grant_and_lost_return_never_redispatch_after_restart`.
- `native_codex_refresh_retained_response_reconciles_failed_publication_without_issuer_replay`.
- `native_codex_refresh_confirmed_no_send_keeps_account_and_original_stop_after_restart`.
- `native_codex_two_processes_refresh_one_original_generation_once`. This launches
  two real test-binary child processes, passes private fixture connection input
  through stdin and uses a test-only barrier after their acknowledged admissions.
  The ignored `native_codex_refresh_crossprocess_worker` is a helper, not a
  standalone passing acceptance case; the parent requires its exact test to run.
  Both use the actual local HTTP endpoint, encrypted store and native journal.

Auth pure-state contract: `host_confirmed_no_send_preserves_account_and_never_grants_redispatch`.
This checks metadata transitions only, not host proof or issuer authority.
- Existing `pg_auth_refresh_tests`, `pg_auth_device_tests`, ordinary Auth callback,
  secret custody and factory consumers affected by the default-backend switch.

LLM test: `host_tokens_require_actual_issuer_lifetime_and_distinguish_refresh_omission`.
Reuse/run the actual SDK tests `host_source_expiry_stops_both_model_entry_points_without_file_repair`,
`host_auth_rejection_never_replays_or_starts_file_or_device_login`,
`host_failures_never_repeat_model_or_source_and_preserve_response_custody` and
`host_transport_failure_is_one_effect_with_no_source_or_model_replay` as applicable.

New PG/HTTP tests use the existing actual local HTTP issuer endpoint adapter,
real stores, admission, native publication and response custody. They do not
qualify production DNS/TLS/private-IP policy or real OpenAI issuer compatibility.
No old executable is represented as testing a newly added interface. Real human
OpenAI device login/refresh, complete ordinary factory→HTTP→UI acceptance,
cross-process execution/interruption, package lint/feature matrix and the final
combined native tests remain open. B3-02–05/09/11 checkboxes are not closed here.
