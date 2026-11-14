# B3-R03 — native Factory/consumer ownership

Base: `3ca800ed6295c152c4f06dd398c1c49712782bb6`; own branch
`codex/b3-r03-native-factory`, managed worktree `b3-r03-native-factory`.
This is a package handoff, not B3 completion, deployment or issuer acceptance.

## Initial actual interface/gap inventory

- `factory::build_local_dev` already constructs one `Arc<PgAuthProductServices>`
  using its existing PG secret owner. `with_native_codex_owners` attaches that
  exact Arc and `ProductAuthProviderRuntimePorts` once; a second attachment fails.
  A-01's default-tenant migration/recovery/fences are retained unchanged.
- Injected `RebornProductAuthServicePorts::from_composed` preserves those native
  owners. Generic injected ports and filesystem recovery remain explicitly
  unavailable for native Codex; they do not acquire SDK file/env credentials.
- M-01 already routes Use/Sempai through
  `llm_catalog::resolve_captured_provider_selection`. Startup, reload and
  `TaskProviderConfiguration` carry `NativeCodexSessionOwner`; only Codex creates
  `NativeCodexSourceSupplier` for the exact OAuth head. API-key presence is separate.
- O-01 already supplies actual Responses construction and credential-free
  preflight. `TokenRefreshingProvider` Host requires SingleSubmission, obtains
  one immutable token/guard and does not retry model sends on 401 or transport
  failure. Final request transport composes the provider policy with
  `NativeCodexAccessGuard`; P-01 supplies safe native Auth/status projections.
- **Source-proven remaining ownership gap:** `runtime` installs
  `PgSempaiPreparation` before replacing `services.product_auth` with the cloned
  facade carrying the final continuation dispatcher. The callback's configuration
  keeps only `Weak<RebornProductAuthServices>`; the old facade can die. Unchanged
  publication short-circuits preparation and masks this. A changed later Codex
  role needs `prepare_retained_provider`, where upgrading the dead Weak loses
  native capability. Keep the already existing narrow native session capability
  in configuration, independently of that facade's lifetime. Do not retain the
  whole auth facade/coordinator strongly in a provider callback.

## Architecture and ownership boundaries

The existing Recipe/global Monty path still owns sequencing and typed task/child
handoffs. This package adds no component, Tool, prompt pipeline, VM, workflow loop,
credential store, scheduler, migration or approval schema. Stable component UUIDs,
immutable approvals/artifacts, one coherent task selection and current kernel
policy remain distinct. Errors never become No-Match or authorize replay.

The four component guides govern preload/export association, one component per
step, typed data and live dispatch; this infrastructure repair changes none of
those contracts. Auth keeps original device/refresh journal and encrypted custody;
provider configuration remains PG-owned. Same-head access rotation constructs a
new immutable request-local guard; it cannot mutate or retarget an earlier request.
No new tenant/user/feature Tool grants or operational constants are introduced.

## Coordination and validation boundary

Only this package's composition/provider ownership and regression sources are
edited. B1/B2/Skills/Engine/Q/N and global status/plan/CHANGELOG files stay untouched.
No local Cargo, native tests, Modal, real personal login, external issuer/model
call, remote SSH or provider/server configuration mutation is authorized here.
Root owns serial native validation and integration. Tool metadata requires direct
human authorization for cross-chat messages; this delegation alone is insufficient,
so questions/results are recorded here and in commentary for Root to read.

## Connected implementation and self-review

`RebornLlmConfigService` now retains `Option<NativeCodexSessionOwner>` when
`with_native_product_auth` receives the already-composed facade, and preserves it
through its existing Clone implementation. `prepare_retained_provider` consumes
that capability rather than upgrading the facade Weak. Login/status operations
still use the Weak pointing to the current product facade; their public behavior
and authenticated mutation routes are unchanged. No public signature changes.

Before: startup `runtime.rs` creates configuration/attaches Sempai preparation
(lines 2745–2792), then replaces `services.product_auth` for final continuation
routing (line 3643). The callback can reuse an unchanged publication, but cannot
build another Codex client after its startup facade dies.

After: the callback owns only the existing native Auth/Host/settings capability
and can resolve the later deliberately selected role's current authenticated
head. It never selects an account while attaching that capability. The existing
`resolve_captured_provider_selection` still binds each constructed source to its
original provider/model/OAuth head/endpoint. A role switch cannot retarget an
already-built provider, token or native access guard.

Lifetime review: `NativeCodexSessionOwner` holds an `Arc<PgAuthProductServices>`,
the existing `ProductAuthProviderRuntimePorts`, and an optional shared primitive-
settings source. The PG owner contains pool, encrypted secret store and account
serialization locks. Host ports contain the same egress/original-egress,
obligation and secret-staging services. Neither contains the product facade,
turn coordinator, Sempai wrapper or a selected token/account/head. Settings remain
the original live Arc that transitions from startup to acknowledged effective
settings; no copied settings revision or new store is added. The preparation
callback retains only a Weak to its provider wrapper. Shutdown adds no producer,
timer or background owner. Existing lifecycle/resource acceptance remains required.

`native_codex_session_owner()` has only two source branches: composed capability
or BackendUnavailable. Mapping unavailable to None at attachment keeps native
capability optional. The existing protocol-specific catalogue resolver explicitly
rejects missing native capability/head when Codex is actually requested; other
protocols never construct a supplier or require native credentials.

## Producer/consumer trace retained unchanged

| Product path | Actual owner and handoff |
| --- | --- |
| CLI run/REPL and serve | CLI `runtime/mod.rs` and `commands/serve.rs` establish PostgreSQL then call the same `build_reborn_runtime`; no CLI-specific Codex connector. Config CRUD uses `ConfigWriteContext::Operator` in the existing PG owner. |
| Factory and injected ports | `build_reborn_services` constructs/attaches PG Auth and host ports once; explicit `from_composed` carries the same capability. No generic trait downcast or reconstruction. |
| Startup Kohai | `resolve_boot_provider_selection_db` captures a coherent role/credential selection and passes its native source into `ResolvedRebornLlm`; optional configuration failure remains a visible usage failure. |
| Reload/current Kohai | `RebornLlmReloadAdapter` carries the same capability, captures the new role once, and publishes the exact rebuilt client. Its weak primary preparation avoids a provider cycle. |
| Use, Sempai restore and later preparation | `RebornLlmConfigService::prepare_retained_provider` uses the same catalogue resolver. This package repairs only its capability lifetime. Mutation/head CAS and original receipt readback stay with existing provider/Auth owners. |
| API/WebUI mutation/bootstrap/readback | Runtime's lazy `webui_llm_configuration` attaches current Auth. Existing config/provider-login facade methods own Use/upsert/login/status, bootstrap linkage and receipt readback; P-01 status distinguishes unobserved/failed/ready facts. This patch grants no bootstrap or serving readiness. |
| Tasks/children | `TaskProviderConfiguration` receives the same native capability at runtime construction. `TaskRoleSource::prepare` resolves the retained task capture, reuses only an identical published client or builds the private original client. Explicit children retain their parent `TaskSelection`/owner identity without rematching. No edits to selection/codecs/Engine. |
| Final send | Guarded provider chain checks captured provider/technical policy. Host Codex requires SingleSubmission and validates body/options before auth. Supplier uses original native refresh/secret custody, then returns a request-local token with immutable `NativeCodexAccessGuard`. Responses creates one request; final checked transport invokes its credential observer, composes original provider guard with exact account/access/head/expiry verification and original submission/cancellation observer before send. This code is unchanged. |

M-01's `eaa2a292e` already supplied preparation/identity fixes; its current
`auth_native_codex.rs` and `llm_catalog.rs` match the integrated APIs. O-01's
fourteen-file transport source and P-01's status source are integrated per the
current package board and scoped native receipts. Their private branch tips also
contain unrelated later fixtures/features: no copy-over, squash or old branch
merge is used. Current Main contracts were read directly. Historical plan text
that says refresh/session propagation is missing does not describe this baseline.

## Regression sources and actual local checks

1. Expanded `native_codex_product_startup_restores_sempai_without_webui_or_dispatch`:
   actual factory/PG login followed by actual `build_reborn_runtime`; the startup
   facade is replaced inside that function before it returns. First prove the
   unchanged client is reused, then change the model through the existing PG
   configuration owner. Invoke the already-attached Sempai preparation callback,
   require a different client with exact new publication/original head, and prove
   the old immutable model remains usable through its existing guard. Remove the
   actual OAuth head and require ConfigurationChanged from the already-built
   new client's dispatch guard. No WebUI/config service is reattached in the test.
2. New `native_codex_product_preparation_survives_non_codex_startup_and_later_login`:
   actual runtime starts on API-key OpenAI without any Codex registration. Later
   Codex selection fails before login and preserves OpenAI's effective client;
   the retained OpenAI guard remains valid. Publish one actual local-HTTP/native-
   PG login after full startup, then the same callback prepares the desired Codex
   model from that newly authenticated head, without reattachment or stale auth
   snapshot. Both tests check that preparation does not increase issuer calls.

Both use the existing real PostgreSQL and isolated local HTTP issuer fixtures;
the existing host test adapter does not qualify production HTTPS/private-IP policy.
No fake session source or successful no-op is introduced. Actual complete-request
measurement/serving, model inference and issuer compatibility are not claimed.

Executed in this worktree: source/definition/caller/feature/lifetime and complete
diff review; direct `rustfmt --edition 2024` for the two changed Rust files;
direct rustfmt `--check` and `git diff --check` pass. First format-check correctly
reported new layout differences; formatting corrected them. Sandbox initially
denied rustfmt writes, then the exact command was explicitly approved and ran.
No Cargo, native test, DB, issuer or model execution was performed by this chat.

## Exact pending supervisor acceptance

Use Root's canonical target, existing native lock and retained worker packaging;
verify actual selected test count and captured source/artifacts. Build once with
the real `skills-db` feature, then execute the focused native cases serially.
Do not count source or rustfmt as runtime success.

```text
cargo clippy -p brassclaw_reborn_composition --all-targets --features skills-db -- -D warnings
cargo test -p brassclaw_reborn_composition --features skills-db --lib pg_auth_product_services::device::tests::codex_refresh_tests::product_tests:: -- --test-threads=1
```

The product filter now contains **seven** tests: four preparation/product cases,
one request-preflight and two native status cases. Exact two changed/new selectors:

```text
pg_auth_product_services::device::tests::codex_refresh_tests::product_tests::native_codex_product_startup_restores_sempai_without_webui_or_dispatch
pg_auth_product_services::device::tests::codex_refresh_tests::product_tests::native_codex_product_preparation_survives_non_codex_startup_and_later_login
```

Existing product Use/head-CAS and missing-head/owner/other-provider cases are
recommended in that same native batch because they consume the changed config
capability. Existing LLM/native refresh/final-send/no-replay code is untouched:
Root may reuse exact applicable prior evidence rather than rebuilding identical
LLM/Auth/Host crates solely for this patch. Full B3 acceptance still separately
needs all named native/cross-process/device/refresh/feature/platform/browser and
actual-issuer gates. Composition defaults are postgres+root-llm-provider; CLI
defaults additionally enable skills-db. Root may reuse relevant feature-off
evidence: this new private field is gated identically to the existing native
Auth methods and no new dependency/feature/API is introduced. No unsupported
backend is promoted to production support.

No B1/B2/Skills/Engine source, migration, status/plan doc or CHANGELOG is changed.
No Main merge/push is performed. Final source commit/hash follows in the chat's
completion; this document cannot certify its own subsequent native acceptance.
