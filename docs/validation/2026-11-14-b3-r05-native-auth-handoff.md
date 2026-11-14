# B3-R05 — native Login/Refresh boundary audit

Worktree: `/Users/ollama/.codex/worktrees/38ed/brassclaw`; branch
`codex/b3-r05-native-auth-boundaries`. Initial HEAD/Main is
`3ca800ed6295c152c4f06dd398c1c49712782bb6`. Root permits using reviewed candidate
`61b456b41711ccbdc55a1e1d019bc938a2ec64f5` after the initial owner audit; its R03
fix is a private dependency, not Main/deployed/accepted support. No Cargo, test,
DB, SSH, issuer/model call, cloud job, shared-cache operation or delegation runs
in this package. Root owns actual native acceptance and integration.

## Existing actual interfaces and callers

| Boundary | Current signature / caller |
| --- | --- |
| Factory attachment | `RebornProductAuthServices::with_native_codex_owners(self, Arc<PgAuthProductServices>, ProductAuthProviderRuntimePorts) -> Result<Self, AuthProductError>`; rejects second attachment. `native_codex_session_owner() -> Result<NativeCodexSessionOwner, AuthProductError>` carries existing Auth/ports/settings, no selected account/token. |
| Public product login | `RebornLlmConfigService::{start_native_codex_login(WebUiAuthenticatedCaller, SetActiveLlmRequest), read_native_codex_login_status(WebUiAuthenticatedCaller, Uuid)}` return `CodexLoginStart`/`CodexLoginStatus`. Login original is mutation UUID == flow UUID == scope invocation UUID. `CodexLoginContext` retains original request/provider revision/selection ref/model/expected head/client/definition. |
| Device preparation | `PgAuthProductServices::prepare_native_device(NativeDeviceLoginDefinition, SecretString) -> Result<NativeDeviceLoginAction, AuthProductError>` retains encrypted first setup; `run_native_device_once(&HostOAuthProviderClient, &NativeDeviceLoginOriginal, Uuid) -> Result<NativeDeviceLoginAction, AuthProductError>` claims one eligible protocol request before I/O. Driver loop is narrow device-protocol polling, not Recipe sequencing. |
| Device publication | `native_device_ready_in_transaction(&Transaction, &NativeDeviceLoginOriginal) -> Result<NativeDeviceReady, AuthProductError>` and `publish_codex_account_and_head_in_transaction(&Transaction, &NativeDeviceReady, namespace, provider_id, Option<&str>) -> Result<PublishedCodexAccount, LlmConfigServiceError>` join account/secrets/head/mutation/selection/flow finish in caller's transaction. Lost COMMIT reads original mutation, no new exchange. |
| Host request | `HostOAuthProviderClient::prepare_oauth_http_request(ResourceScope, endpoint, content_type, Vec<u8>) -> Result<RuntimeHttpEgressRequest, AuthProductError>` captures acknowledged OAuth timeout/body settings and stages existing obligation policy. `native_original_egress() -> Result<Arc<dyn HostHttpOriginalEgress>, AuthProductError>`. |
| Original custody | `HostHttpOriginalEgress::execute_with_original_custody<'a>(&'a self, RuntimeHttpEgressRequest, Box<dyn HostHttpOriginalCustody + 'a>) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError>`; custodian authenticates exact final request and commits request Original before permission to transport; response retained encrypted before sanitizer/public projection. |
| Refresh selection | `prepare_codex_access(self: &Arc<Self>, &ProductAuthProviderRuntimePorts, Option<Arc<dyn HostPrimitiveSettingsSource>>, CodexAccessSelection<'_>) -> Result<(), LlmError>`; selection is namespace/provider/head/API endpoint/margin. `admit_codex_refresh(&CodexAccessSelection) -> Result<Option<AdmittedCodexRefresh>, AuthProductError>` and `execute_codex_refresh(&HostOAuthProviderClient, &AdmittedCodexRefresh, &CodexAccessSelection) -> Result<(), AuthProductError>` use existing refresh journal, no new backend. |
| Refresh identity | UUIDv5 derives from exact binding/head/account/access reference/refresh reference (`native-codex-refresh-generation/1`), not caller random IDs or refresh token alone. `begin_host_instance_refresh_in_transaction` separates System-owned native account from ordinary extension refresh. Only acknowledged winning admission can dispatch. |
| Refresh publication | Existing `record_refresh_result_in_transaction`/`reconcile_refresh_action_in_transaction` retain original source/account/CAS; omitted refresh token preserves old credential under RFC6749 §6. Actual request/response are private SecretStore originals; absent response remains unresolved. |
| Access / final send | `take_codex_access_session(self: &Arc<Self>, namespace, provider_id, head, API endpoint, Arc<dyn ProviderDispatchGuard>) -> Result<CodexHostSession, LlmError>` conditionally leases/consumes exact encrypted generation. `NativeCodexSourceSupplier::session()` prepares eligible refresh then takes new immutable token/guard. `NativeCodexAccessGuard::{check,check_preparation}` composes original provider guard, exact head/account/access generation and expiry; it never changes an old request's selected source. Host `TokenRefreshingProvider` requires SingleSubmission and never executes file refresh/model replay. |

Auth/Host/Secrets remain native primitives, transport/authentication and effect
custody. Recipes/PythonCode on one global Monty own task sequencing and typed
immutable originals. No new Recipe, Skill, Tool, approval, registry, queue or
prompt pipeline is proposed. Metatada/claims/approval selection grant no Tool
permission; live policy, cancellation and original effects remain independent.

## Features and evidence applicability

Production PG attachment exists in normal factory and explicit `from_composed`
injection. Generic injected ports/non-PG recovery return typed unavailability;
they gain no file/ENV/SDK fallback. `root-llm-provider` owns native Codex modules;
`skills-db` additionally owns actual provider-bootstrap/product-runtime tests.
Composition defaults are postgres+root-llm-provider; CLI propagates skills-db.
Non-Codex providers and unconfigured optional Codex remain independent.

Existing `pg_auth_device_tests.rs` exercises actual PG/encrypted originals with
real isolated HTTP through `LocalIssuerHttp`; its endpoint rewrite bypasses
production DNS/TLS/private-IP checks. Existing `ObservedKernelEgress` denial case
uses genuine Host + PolicyNetwork + Reqwest and proves private-target refusal,
not positive TLS. Existing refresh tests cover two owners, two OS processes,
omitted refresh token, subsequent generation, invalid grant/disconnect, failed
publication/readback, confirmed no-send and restart. R03's own handoff identifies
two runtime/factory cases; those are not duplicated here.

## Concrete shared-owner boundary for Root

1. Positive local production HTTPS acceptance currently cannot be obtained by
   simply replacing `LocalIssuerHttp`: `oauth_network_policy()` always sets
   `deny_private_ip_ranges=true`, and `ReqwestNetworkTransport` exposes no scoped
   trust-root configuration. `oauth_endpoint_host` requires HTTPS at actual
   request preparation (the metadata client's syntax validator allows HTTP too).
   A loopback issuer is therefore refused before actual TLS. No private-IP,
   hostname, certificate or global safety bypass is authorized. This is an
   acceptance-environment/owner-interface seam, not proof that public issuer
   transport is broken. Any scoped trust/endpoint settings extension would need
   existing Network/Host/settings-owner agreement before edits. No such extension
   or dependency is added speculatively.
2. Direct full `MontyExecutionLimits` serialization is absent from these audited
   Auth/Login/Refresh original/hash domains. `auth_native_codex::native_codex_limits`
   forwards `acknowledged_oauth_limits`; request construction extracts only
   `oauth_token_timeout_millis` and `oauth_token_response_body_bytes`. Device
   originals serialize actual `RuntimeHttpEgressRequest`; refresh generation hashes
   binding/head/account/access/refresh refs. `CodexLoginContext` serializes its
   explicit definition/request fields, not the complete limits struct. B2-R04 owns
   frozen-limit serialization/V216; this package will not copy it or change those
   original bytes.

3. Request cancellation before refresh has a distinct signature seam:
   `CodexHostSessionSource::session(&self)` carries no request/run/attempt or
   cancellation observer. `TokenRefreshingProvider` prepares request bytes before
   `session()`, but its original request observer/final-send guard is used later.
   `NativeCodexSourceSupplier` knows the captured provider guard, not that
   request's cancellation observer. Auth still authenticates current original
   account/head/generation at refresh preflight and final model-send remains
   fenced. The existing task-local `provider_observation::current()` resolves a
   `ProviderWireObserver`; its `before_submission()` delegates in production to
   `ProviderWireCustodian` / `LoopModelResponseCustodian::before_provider_submission`,
   and independently to `ReviewWire` for review work. Neither is currently passed
   into credential preparation. Reusing that checked observer is preferable to
   inventing a second cancellation authority, but authenticating it immediately
   before refresh transport still needs an agreed LLM/Auth handoff. No invented
   IDs, cached request guard or production signature change is made here pending
   Root's owner decision.

## Connected bounded correction and regression sources

Actual defect: the shared native PG Auth `get_record`/`list_records` formatted
`serde_json::Error` into debug logs. A rejected enum value from a corrupt account
row is quoted by Serde and could therefore disclose token/code/header-shaped DB
payload text. `put_record` had the same unfiltered diagnostic path. Preserve
table/filter identity plus Serde category/line/column, never its payload-bearing
message. Public errors remain the existing typed `BackendUnavailable`; no wire,
schema, CAS, selector, runtime/factory or frozen-limit contract changes.

New real-PG account regression corrupts an actual created account's status with
a canary, then invokes actual `CredentialAccountService::get_account` and
`list_accounts` from a recreated owner. Both must fail closed, retain two Data
category diagnostics without the canary, leave the row unchanged, and recover
their exact positive account projection after a deliberate native CAS repair.
It is a security regression through the actual reader, not a helper mirror.

Two further native PG/actual-local-HTTP regressions hold a real refresh response
after issuer receipt. A second native Auth owner must (a) revoke the actual
account or (b) atomically publish a separately completed newer login/head while
that response remains held. The old return must be retained privately without
updating either revoked/new account; original-only repeated execution and a
recreated owner cannot issue another refresh. Both races prove DB writers can
complete across an awaited issuer response; the 20-second fixture watchdog is
test orchestration only, with production timeouts unchanged. The local endpoint
adapter remains explicit: no production DNS/TLS, whole product login, actual
OpenAI compatibility or complete cancellation acceptance follows.

No runtime acceptance or full B3-04/05 completion is claimed.

## Self-review and executable supervisor gate

Final implementation basis is the exact reviewed Root candidate `61b456b41`.
Only the four Auth production/test files and this unembedded handoff change
relative to that dependency; inherited B2/R03 files are not package edits.
No public/interface/schema/SQL/ABI/feature/dependency/limit contract changes.
No general runtime/factory/LLM configuration, Skills/Engine/Capture/N/R/Q/T edit.

Executed locally: actual definition/caller/feature/ownership and connected diff
review; direct `rustfmt --edition 2024 --config skip_children=true` on all four
Rust files; same direct `--check` and `git diff --check` pass. No local Cargo,
compiler/test/DB/HTTP/issuer execution. Existing cases stay enabled unchanged;
the isolated worker's preexisting ignore is not a skipped new test.

Root should compile once with the existing canonical target/lock, retain exact
source/image/worker evidence and execute these three nonignored selectors:

```text
pg_auth_product_services::refresh_tests::native_account_lookup_and_list_redact_rejected_database_values
pg_auth_product_services::device::tests::codex_refresh_tests::native_codex_refresh_late_return_retains_custody_after_account_revocation
pg_auth_product_services::device::tests::codex_refresh_tests::native_codex_refresh_late_return_never_installs_into_a_new_login
```

Actual manifest/feature scope:

```text
cargo clippy -p brassclaw_reborn_composition --all-targets --features skills-db -- -D warnings
cargo test -p brassclaw_reborn_composition --features skills-db --lib native_account_lookup_and_list_redact_rejected_database_values -- --test-threads=1
cargo test -p brassclaw_reborn_composition --features skills-db --lib native_codex_refresh_late_return_ -- --test-threads=1
```

Expected selected count is one plus two, none ignored. Required lint/real native
security/concurrency acceptance is **unexecuted** here. Reuse unchanged original
refresh/device/factory/cross-process/SingleSubmission results only after Root
checks their exact applicability. General PG helper changes also merit the
existing native account-CAS positive/negative cases in `refresh_tests`; they
need not rerun unrelated library/bootstrap qualification.

Operational open boundaries: real production HTTPS/TLS/private-IP success and
real issuer compatibility; original request cancellation through native refresh;
complete authenticated API/UI login/publication/restart/ACK and broader product/
platform gates. This package supplies neither a deployed readiness claim nor a
complete B3 checkbox. Source checkpoint does not release Root review corrections.

## Cancellation seam: audited proposal, no signature implementation

Root's subsequent priority is B1, then B2, then B3 after this correction package.
The following is a concrete cross-owner proposal for Root's boundary decision,
not permission to start another B3 package. No LLM/Auth signatures, Network/TLS
sources, scheduler or second guard authority were changed.

### Existing observer implementations and effects

The production `ProviderWireObserver` implementations are:

- `reborn/model_gateway.rs::ProviderWireCustodian`: `before_submission` calls
  `LoopModelResponseCustodian::before_provider_submission`; rejection sets its
  preflight-denied marker. `submission_started` is a separate model marker.
- `llm/openai_codex_provider.rs::CodexCredentialObserver`: checks the immutable
  access/provider guard, then calls its captured original observer. It is built
  after session acquisition and cannot protect earlier refresh/secret work.
- `composition/completed_turn_review/wire.rs::ReviewWire`: checks closed state,
  original global ownership and the actual attempt/owner row in
  `model_dispatching`, count one, with retained model binding. This check is
  read-only; started/rejected are separate markers.

All other implementations found are test observers in `codex_host_session_tests`,
`final_request_sdk_tests` and `single_submission_tests`; their event-recording,
unconditional success or explicit rejection is fixture behavior, not production
authority. There is no reusable production default-success request fence.

The production loop custodian is
`reborn/monty_task_host.rs::SelectedModelResponseCustodian`, delegating to its
own `RecipeModelResponseRecord`. The production record implementation is
`composition/pg_monty_model_response.rs::Handle`. Its
`before_provider_submission` is **not a repeatable read-only check**: it checks
the original admission, locks the invocation, checks no observations, reserves
related review submission where applicable, verifies the claim with its snapshot
lock, atomically sets `submission_reserved`, and commits. An uncertain COMMIT
does not release the reservation. Calling this during auth refresh would consume
the model submission and break the later real send. The loop trait's existing
legacy default is fail-closed only when provider observation is required; it is
not a suitable new native-auth fence.

### Proposed existing-port changes and actual callers

Propose a required async, repeatable `check_request_fence` method on the existing
`ProviderWireObserver`, `LoopModelResponseCustodian` and
`RecipeModelResponseRecord` ports. Root chooses the final name/signature. No new
guard trait or parallel authority is needed. Every production implementation
must explicitly implement/delegate it; no permissive default for required
BrassClaw calls. The final consuming `before_submission` path stays separate.

`ProviderWireCustodian -> SelectedModelResponseCustodian -> Handle` would check
the authentic retained admission/invocation through existing claim readers in a
short transaction, plus unchanged invocation phase and unsubmitted observations.
Related review records must also check their own exact work/request/prefix and
prepared/count-zero state without calling `reserve_related_submission`. This
method must not update submission counts, reserve anything, set model-started,
or retain an auth reply as a model response. `ReviewWire` can share its existing
read-only ownership checks with this method; its already-owned dispatch count
must not increment again. `CodexCredentialObserver` must delegate the pure check
separately from the consuming final preflight.

Propose changing `CodexHostSessionSource::session` to require the actual request's
observer/fence, supplied by `TokenRefreshingProvider` in both text and tools Host
branches. The only production source implementation is
`composition/pg_auth_codex_access.rs::NativeCodexSourceSupplier`; the other two
are `codex_host_session_tests::{Source, ExpiredHostSource}` fixtures. Capture the
current explicit observer once for this request; missing native context must
fail before admission/secret consumption. Do not cache the last observer on a
supplier/provider, fabricate a run/attempt, or substitute a passing fixture.

Carry this request-local context through `prepare_codex_access`, winning refresh
execution and `take_codex_access_session`. Check before new refresh admission or
secret consumption and in the actual `RefreshCustodian` transport preflight,
alongside the existing exact account/head checks. The implementation must cover
the final pre-transport gap after request-original persistence as well. The
actual model send then independently performs its current access/provider check
and consuming model submission preflight. Auth dispatch never marks model send.

### Caller/ownership matrix

| Path | Actual observer and authentic ownership | Required integration boundary |
| --- | --- | --- |
| Ordinary completion and tools | `model_gateway::observed_provider_call` creates `ProviderWireCustodian`; exact model invocation/admission in its own record | Both completion branches must supply the same request-local pure fence before Host session acquisition. |
| Task/explicit child model call | Same record pipeline; child uses its own admission/invocation with inherited immutable model selection | Never substitute parent's claim for child's claim; private deterministic model calls currently remain rejected by the existing host. |
| Pre-call Sempai | `model_interceptor::accounted_review` prepares a separate related response owner via `prepare_related_model_work`, then calls `stream_model_observed` | Related record owns original work/request/checksum/reviewer prefix and its separate reservation. It must never borrow the outer Kohai observer. |
| Completed-turn review | Backend owns actual review attempt/owner; explicit `observe_chat_completion(ReviewWire, operation)` wraps gateway `stream_model` when `prefix_at_final_boundary` is true | Keep this own observer through the gateway's no-loop-custodian path. Its actual ownership/closed state must fence session acquisition. |
| Legacy completed-turn/no-custody path | `prefix_at_final_boundary` false constructs no `ReviewWire`; legacy loop custodians can also omit observation | This is a real missing native fence, not evidence of safety. Do not silently accept native Host Codex or inherit an unrelated ambient observer. |

`model_interceptor::prepare` runs the precall reviewer before installing the
primary gateway's observer; own related custody is nevertheless required.
`observed_provider_call` currently preserves ambient task-local observation when
there is no loop custodian, which is intentional for completed-turn `ReviewWire`.
Blanket clearing that scope would break that actual caller. Root must decide
explicit own-scope propagation/masking for legacy and reviewer calls together;
simply reading whatever outer observer exists is not sufficient authentication.
File-backed Codex and non-Codex compatibility stay a separate caller decision;
their existing behavior must not gain a fake native observer or unrelated gate.

### Request cancellation versus shared refresh custody

The refresh journal belongs to its original credential generation and exact
binding/head/account/access/refresh references. A request fence is request-local,
not part of the refresh ID and not a replacement for the original sealed action.
Two waiters must keep independent fences. A cancelled waiter cannot cancel,
retarget or lend its authorization to the other waiter; no new refresh ID or
replacement dispatch is created to bypass the original winner.

Before admission, rejection can prevent new work. If the winning action is
admitted but transport is provably unsubmitted, use the existing original-only
no-send outcome. Once request transport may have begun, late response custody
must retain the exact private request/response/returned originals despite request
cancellation. Reconciliation may publish only to that original account/head
under existing identity/CAS checks; it must not require a cancelled caller's
permission merely to retain received bytes. Unknown outcomes remain unknown,
never deleted/reset/replayed. A cancelled caller must not consume/return a token
for a subsequent model send. Another valid waiter may observe the same confirmed
generation outcome through its own fence; it cannot replay the original issuer
request or take over its sealed dispatch.

No detached refresh scheduler is proposed. The existing pending-future/original
custody owner must be checked at implementation time for dropped callers as well
as cancellation observed while polling; this source audit does not establish
crash recovery or durable continuation. The new revocation/new-login races prove
neither request cancellation nor all coalesced-waiter behavior until actually
compiled/executed, and even then cover their stated account/head races only.

## Final source self-review checkpoint

Source commit: `ea3690a467575f9465ed628f95bedf56aba8665e` on
`codex/b3-r05-native-auth-boundaries`, based on Root-authorized private candidate
`61b456b41711ccbdc55a1e1d019bc938a2ec64f5`. Rechecked actual account revision
initialization/CAS repair, held issuer notifications, separate native owner
transactions, original action/custody assertions, issuer call counts and
unchanged production timeout/transport boundaries. The Serde diagnostic fix
retains only structural category/position fields; no payload-bearing error is
formatted. No additional source correction was identified in this review.

Root owns native compilation, lint and the three real regressions above. They
remain pending, with no source-only acceptance or completed B3 claim. No further
B3 implementation starts from this handoff ahead of required B1/B2 work unless
Root establishes a concrete shared prerequisite.
