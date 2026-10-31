# BrassClaw Reborn secrets service contract

**Date:** 2026-04-26
**Status:** V1 service-boundary slice
**Crate:** `crates/brassclaw_secrets`
**Depends on:** `docs/reborn/contracts/host-api.md`

---

## 1. Purpose

`brassclaw_secrets` is the scoped secret metadata and lease service for Reborn.

It turns opaque host API handles into explicit, short-lived access leases:

```text
ResourceScope + SecretHandle
  -> SecretStore::lease_once(...)
  -> SecretLease
  -> SecretStore::consume(...)
  -> SecretMaterial exactly once
```

The crate owns storage mechanics and one-shot lease state. It does not decide authorization, run approval flows, contact networks, emit audit events, or execute product workflows. It only provides the metadata and lease/consume primitive; host-runtime composition owns any concrete injection into runtime requests.

---

## 2. Boundary

The public contract is intentionally small:

```rust
SecretMaterial
SecretMetadata
SecretLeaseId
SecretLeaseStatus
SecretLease
SecretStoreError
SecretStore
InMemorySecretStore
FilesystemSecretStore      // durable when backed by Postgres RootFilesystem
CredentialAccountStore
CredentialSessionStore
InMemoryCredentialBroker
FilesystemCredentialBroker // durable when backed by Postgres RootFilesystem
```

`SecretMaterial` is backed by `secrecy::SecretString`, so access to raw values is explicit through `ExposeSecret`. Metadata, lease records, and errors never contain raw values.

Ownership remains:

```text
host_api       -> opaque SecretHandle and Action::UseSecret shapes
secrets        -> scoped storage, metadata, and one-shot leases
authorization  -> whether a caller may use a SecretHandle
capabilities   -> caller-facing workflow; fails closed on InjectSecretOnce unless an obligation handler is configured
host_runtime   -> built-in obligation handler leases/stages one-shot secret material and shared runtime HTTP egress injects/redacts secrets for host-mediated requests
runtimes        -> consume injected values only after host-side authorization and lease handling
```

---

## 3. Scope and isolation

All operations receive a `ResourceScope`. The in-memory and filesystem-backed V1 implementations key secrets by tenant/user/agent/project plus `SecretHandle`; leases are scoped by the full invocation context plus `SecretLeaseId`.

Rules:

- no global handle lookup
- the same `SecretHandle` in another tenant/user/agent/project is a distinct secret
- cross-scope lease consumption returns `UnknownLease`
- missing secrets return `UnknownSecret` and do not create leases
- consumed leases cannot be consumed again
- revoked leases cannot be consumed

This is the minimum shape needed for host-runtime composition to wire secret injection and credential brokerage into obligation handling without exposing raw values to runtime crates.

---

## 4. Current API flow

```rust
let metadata = secrets
    .put(scope.clone(), handle.clone(), SecretMaterial::from("token"))
    .await?;

let lease = secrets.lease_once(&scope, &handle).await?;
let material = secrets.consume(&scope, lease.id).await?;
```

`metadata` and `lease` are safe to log only as metadata; they do not include secret values. `material` is the only raw-value carrier and should stay inside the narrow injection path that requested it.

`SecretStore::put(...)` is for trusted setup, composition, migration, or storage-code paths that are already allowed to manage secret material. It is not a runtime/plugin API, and it intentionally does not perform authorization itself.

Trusted setup also exposes three required owner operations: `record_reference`
returns an opaque authenticated encrypted generation; `put_if_record_reference`
atomically replaces exactly that generation (`None` means absent-only insertion);
`matches` compares authenticated material inside the owner without returning it.
A stale comparison returns no write. Storage failure can still mean a committed
write with a lost acknowledgement; callers must read back the original rather
than assume no effect. Reads neither lock/migrate PostgreSQL legacy records nor
consume a lease. These operations are not runtime eligibility or authority and
can compare an expired stored value without permitting its use. References are
backend-bound and must not be interpreted as plaintext fingerprints.

An ordinary mutable handle still does not retain the original input or pin a
lease to a generation. Trusted setup additionally uses three required custody
operations on every selected store/forwarder:

- `put_original` stores the first encrypted raw input with its full original scope
  and typed nonsecret context. Exact input/context/scope repeats return the actual
  stored metadata without rewriting. Different inputs, an ordinary occupant or
  a retired identity return no write. The context must contain setup identities
  and selections, never raw tokens or plaintext token fingerprints.
- `custody` authenticates and returns the original, retained or retired metadata.
  It does not expose material, issue a lease or establish runtime authority.
- `put_retained_from_original` creates an immutable output using the actual live
  original's scope/handle/encrypted-generation reference inside the storage owner;
  callers supply no substitute material. Exact target/source repeats read back
  unchanged; incompatible, retired or missing selections cannot create outputs.

Original inputs are unavailable to ordinary runtime metadata/lease/consume. A
retained output follows the existing scoped one-shot runtime boundary. Ordinary
put and conditional put cannot overwrite custody records. Delete retires such a
record: it erases raw material while retaining occupied identity, original context
and exact source references. No writer resurrects that identity. Original
retirement is input-data disposition and does not revoke an independently issued
output; live account/kernel/attempt/expiry enforcement remains required. A raw
submit after original retirement conflicts; a payload-free confirmed-original
lookup is a separate operation.

All bodies use the existing AES-GCM/HKDF owner with a distinct authenticated
custody purpose. Ordinary raw JSON remains ordinary secret material. FS omits
its optional format marker for old ordinary records; native PG uses the
`custody-v1:` purpose prefix before its existing encoded encrypted envelope.
Unknown, altered or wrong-owner formats fail before effects. The current/legacy
ordinary reference algorithms and public reserved-system scope rejection remain
unchanged. Only internal scoped/authenticated DTOs rehydrate trusted System data.

Memory serializes writes under its actual owner lock. Native PG offers the same
custody operations within the caller's transaction, including exact source and
target locking. CAS-only FS captures authenticated source data at the read; it
does not offer an atomic multi-record publication or grant. All FS normal writes
use absence or the version plus encrypted-body checksum from one backend read;
ordinary deletion uses native conditional single-entry removal. An unsupported
backend never downgrades to unconditional mutation. False deletion means no
selected mutation, not proof another current value is absent. An error may mean
a committed effect whose acknowledgement was lost.

These storage primitives alone do not publish an AuthProduct workflow atomically,
reconcile account/flow/result/browser state or fence leases for ordinary mutable
values. Those consumers retain their explicit K3 step2/step4 ownership and
acceptance requirements. Never compensate unknown publication with blind delete
or recreate an original identity.

Durable PostgreSQL storage is provided by `FilesystemSecretStore` and
`FilesystemCredentialBroker` over the database-backed `RootFilesystem`
implementations. Backend selection is now a property of the filesystem layer;
the current filesystem crate exports PostgreSQL, memory, local and composed/HSM
backends, with no libSQL runtime backend. Earlier libSQL references describe the
retired implementation; the separate legacy SQLite/libSQL importer is not a
current persistence fallback. Qualify native PgSecretStore and actual persistent
PG-backed FS behavior independently when either path changes.
`brassclaw_secrets` stores encrypted payloads and per-record salts under scoped
filesystem paths, with tenant id projected as a defense-in-depth index. Store
readiness must fail closed when the configured master key is missing or
malformed. The earlier filesystem-stored key-check sentinel was removed with the
tenant-aware `ScopedFilesystem` rework; master-key mismatch is verified on the
first per-tenant decrypt operation.

The shared Reborn runtime HTTP egress service uses this surface to:

- check metadata for required or optional credential handles
- create one-shot leases scoped to the request
- consume the lease exactly once inside the host process
- reject runtime-supplied sensitive headers, auth-like headers, credential query parameters, credential-shaped request content, and credential-shaped raw or percent-decoded URL content before network dispatch
- inject material into the outgoing request shape
- scrub leased values from runtime-visible network errors and response headers/bodies
- strip sensitive response headers and block credential-shaped response bodies before they reach runtime callers
- support header, query parameter, and path-placeholder credential targets. Request-body credential injection remains out of scope.

Path-placeholder injection has the weakest ambient-redaction story: upstream
access logs, CDN/proxy logs, crash dumps, and `Referer` values commonly retain
URL paths. It must be used only for a capability with a documented upstream
requirement that cannot use headers or query parameters. Host-runtime egress
keeps this target HTTPS-only, rejects empty, `.`/`..`, control-character, and
reserved-character values, and requires exactly one full-segment placeholder so
secret material cannot rewrite the destination path structure.

Runtime HTTP credential injection is authority-bearing and must be host-derived.
`RuntimeCredentialInjection` is not a permission request supplied by guest code,
runtime code, or an extension process. The upstream capability/obligation owner
must derive it only after proving:

- the extension or capability declared the secret handle
- the caller is authorized or approved to use the secret
- the destination URL matches the capability or secret destination policy
- the injection target and prefix are host-approved
- the final request still passes the network policy boundary

The shared egress service intentionally does not perform that authorization
decision; it consumes the already-approved injection plan, injects it, redacts
it, and fails closed when a required credential is unavailable. Injection plans
also declare a material source. Production runtime tool egress uses
`StagedObligation { capability_id }`, which consumes material that
`BuiltinObligationHandler` already leased, consumed, and staged in
`RuntimeSecretInjectionStore`. `SecretStoreLease` remains only for explicitly
named legacy/test compatibility paths that lease and consume directly from
`SecretStore`; production egress rejects it before outbound transport. Runtime
adapters that use the staged source must not lease the same handle
independently; `HostHttpEgressService` removes staged material with
`take(scope, capability_id, handle)` before outbound transport so the value
cannot be reused after success, failure, or runtime-visible errors. Staged
entries also expire after the store TTL (five minutes by default) and expired
material is pruned during insertion, `take(...)`, and explicit
`prune_expired(...)` calls. If one approved request plan injects the same
source+handle into multiple targets, the egress service consumes or leases the
material once and reuses it only within that request. Runtime callers must not
supply their own `Authorization`, cookie, or API-key-style headers; those values
must come from the host-approved injection plan. WASM host-mediated HTTP
composition should derive production staged plans from manifest v2
`runtime_credentials`: the declaration identifies the runtime credential slot,
source (`secret_handle` or product-auth account provider), HTTPS audience,
required/optional behavior, and injection target, while authorization still
decides whether material may be staged for that invocation. Product-auth account
sources resolve to the selected account's access secret before the same
one-shot handoff store is populated. Explicit
`WasmStagedRuntimeCredentials` construction is retained for named legacy/test
composition; exact-url rules should be preferred there when a credential is only
valid for specific destinations.

---

## 5. Non-goals

This slice does not implement:

- platform keychain integration
- secret rotation/versioning
- secret audit emission
- authorization policy for secret use
- approval prompts for secret use
- direct runtime environment/request injection from this crate
- OAuth/token refresh flows
- network policy enforcement

Those should be added as separate service/composition slices without moving runtime or product workflow semantics into this crate.

---

## 6. Contract tests

The crate tests cover:

- metadata returns no raw secret material
- one-shot leases consume exactly once
- same-handle secrets are tenant/user/agent/project isolated
- consumed and revoked lease records drop retained secret material
- revoked leases cannot be consumed
- missing secrets fail without creating leases
- durable filesystem-backed stores keep raw secret, credential-account, and credential-session payloads encrypted at rest
- filesystem-backed broker records preserve tenant/user/agent/project scope isolation and session use limits
- malformed or missing master keys fail before production composition reports ready
- crate boundary remains low-level and does not depend on workflow/runtime/observability crates

---

## 7. Reborn issue #3088 closeout notes

This contract is the current status source for the secrets side of
[#3088](https://github.com/chtugha/brassclaw/issues/3088), alongside:

- [#3068](https://github.com/chtugha/brassclaw/issues/3068) for credential-injection parity
- [#3085](https://github.com/chtugha/brassclaw/issues/3085) for shared runtime HTTP egress
- [#3026](https://github.com/chtugha/brassclaw/issues/3026) for production composition
- [#3032](https://github.com/chtugha/brassclaw/issues/3032) for no-exposure safeguards

Closed in the current Reborn slice:

- durable encrypted secret storage over PostgreSQL-backed RootFilesystem
- durable credential account/session storage through `FilesystemCredentialBroker`
- production wiring guardrails for credential account/session stores
- staged-obligation production egress as the canonical direct-secret-injection boundary
- V1 HTTP credential target coverage for headers, query params, and path placeholders

Deferred outside this issue's V1 slice:

- request-body credential injection
- non-HTTP credentials
- arbitrary script or external MCP process ambient-network credential injection
- external proxy/sidecar credential enforcement
- provider-specific OAuth refresh UX
- redirect-following credential reinjection. Current built-in host HTTP returns redirect responses without following them, so credentials are not forwarded across redirect hops.

### Filesystem generation CAS

`FilesystemSecretStore::put_if_record_reference` captures the encrypted entry body
checksum and version from the same read. Native memory and PostgreSQL owners
check both atomically with the write. A recreated path may reuse a version, so a
version alone cannot prove the selected generation. A checksum mismatch returns
no replacement; unsupported preconditions remain storage errors without weakening
them to `Any`. This compares the exact serialized encrypted body, not plaintext
material, metadata or a physical lifetime restored with identical bytes. Original
manual-input custody and lease-generation capture remain their separate K3 steps.
