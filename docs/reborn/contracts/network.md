# BrassClaw Reborn network service contract

**Date:** 2026-04-26
**Status:** V1 policy + HTTP egress slice
**Crate:** `crates/brassclaw_network`
**Depends on:** `docs/reborn/contracts/host-api.md`

---

## 1. Purpose

`brassclaw_network` is the scoped network policy and outbound HTTP egress service for Reborn.

It turns a host API `NetworkPolicy` plus a scoped target request into a metadata-only permit:

```text
NetworkRequest { ResourceScope, NetworkTarget, NetworkMethod, estimated_bytes }
  -> NetworkPolicyEnforcer::authorize(...)
  -> NetworkPermit or NetworkPolicyError
```

For host-mediated runtime HTTP, the crate also turns a `NetworkHttpRequest` into a bounded outbound transport call:

```text
NetworkHttpRequest { scope, method, url, headers, body, policy, response_body_limit }
  -> URL parse + target normalization
  -> NetworkPolicyEnforcer with a conservative method + URL + header + body byte estimate
  -> DNS resolution + private/reserved IP denial
  -> pinned outbound HTTP transport with redirects disabled and a bounded client cache
  -> NetworkHttpResponse { body, NetworkUsage { request_bytes, response_bytes, resolved_ip } }
```

This crate does not inject credentials, reserve resources, emit audit/events, make authorization/approval decisions, or execute product workflow. Runtime crates and host-runtime composition call this boundary instead of constructing their own HTTP clients, DNS logic, redirect handling, SSRF checks, or response-limit code.

---

## 2. Boundary

The public contract is intentionally small:

```rust
NetworkRequest
NetworkPermit
NetworkPolicyError
NetworkPolicyEnforcer
StaticNetworkPolicyEnforcer
NetworkHttpEgress
PolicyNetworkHttpEgress
NetworkHttpTransport
ReqwestNetworkTransport
NetworkResolver
NetworkHttpRequest
NetworkHttpResponse
NetworkUsage
network_policy_allows(...)
target_matches_pattern(...)
host_matches_pattern(...)
is_private_or_loopback_ip(...)
```

Ownership remains:

```text
host_api       -> NetworkPolicy, NetworkTarget, NetworkMethod shapes
network        -> scoped policy evaluation, DNS/private-IP checks, bounded HTTP transport
authorization  -> whether a caller has a grant with network authority
capabilities   -> caller-facing workflow; fails closed on ApplyNetworkPolicy unless an obligation handler is configured
host_runtime   -> built-in obligation handler validates/stages scoped network policy and shared runtime HTTP egress enforces policy
runtimes        -> translate native HTTP calls into host-mediated egress requests
```

---

## 3. Policy semantics

V1 semantics intentionally mirror the current WASM network import policy checks so they can later be centralized:

- empty `allowed_targets` fails closed
- `NetworkTargetPattern.scheme` must match when present
- `NetworkTargetPattern.port` must match when present; URL-derived targets use the known default port for `http` and `https` when no explicit port is present
- `host_pattern` is exact host or one leading wildcard label such as `*.github.com`
- wildcard patterns do not match the apex host itself or deeper multi-label subdomains
- `deny_private_ip_ranges` blocks literal private, loopback, link-local, documentation, broadcast, multicast, unspecified, carrier-grade NAT, IPv4-mapped IPv6 private ranges, and unique-local IP targets
- `max_egress_bytes` requires a request-byte estimate and denies requests whose estimated bytes exceed the configured limit. Host-mediated HTTP estimates include the method, URL, headers, HTTP framing overhead, and body so large URLs or headers cannot bypass the limit.

HTTP egress rejects URL userinfo before policy, DNS, or transport so credentials cannot be smuggled through an allowlisted host URL. It resolves hostnames before dispatch and denies the request before transport when `deny_private_ip_ranges` is true and any resolved address is private, loopback, link-local, documentation, multicast, broadcast, unspecified, carrier-grade NAT, unique-local, or an IPv4-mapped IPv6 address that maps to a non-public IPv4 address. The default transport disables ambient proxies and redirects and pins the request to the vetted resolved address set so a later DNS answer cannot silently change the destination for that request, while still allowing connector-level fallback across alternate A/AAAA answers. Caller-provided `Host` headers are rejected before transport so virtual-host routing cannot diverge from the URL host that policy validated. Runtime-visible response bodies are always bounded. An omitted `response_body_limit` uses the acknowledged instance default; an explicit positive allocation-representable limit is retained exactly, including values above that default. Zero and nonrepresentable values fail before dispatch. Explicit positive request timeouts are retained without clamping to the instance default; zero or host-clock overflow fails before dispatch. Response buffer allocation failures return a sanitized transport failure. Current Tool policy and schema constraints remain independent; transport does not grant permission or silently weaken those checks.

---

### Host HTTP client retention (v3)

`max_http_client_cache_entries` is an instance execution setting (default 128).
Zero disables retention. The same hardened transport supplies the cache control
and actual host egress; replacing an egress port invalidates that attachment.
Settings publish through the shared acknowledged runtime revision before the
full effective view is exposed. This control grants no network permission and
is separate from completed model-response caching.

Cache lookup and least-recent eviction use randomized keys containing host,
port, vetted addresses and request timeout. Full caches evict individual entries.
Reductions remove cache ownership while active requests keep their captured
clients. A client build admitted before a new settings revision cannot insert
into its successor. Limits are ceilings, not up-front cache allocation.

### Host HTTP request defaults (v3)

`default_http_request_timeout_millis` (30,000 ms) and
`default_http_response_body_bytes` (10 MiB) are live instance execution settings
under Settings → Monty VM, with English and German explanations. They apply only
to host HTTP requests that omit explicit bounds. Model-provider settings are
separate. Defaults are positive and must fit the host clock/allocation types;
invalid changes fail before persistence or runtime effects.

The transport captures both defaults together before asynchronous work. An
accepted request retains its timeout and response limit across later settings
publication. New requests use the acknowledged successor. Cache capacity and
both defaults publish atomically on the same transport under the shared
settings revision. Oversized responses remain explicit failures, not successful
truncated responses. Tool schemas and live network policy remain independent.

### First-party HTTP Tool timeout

`builtin.http` and `builtin.http.save` forward an omitted timeout to the actual
host transport. Explicit positive `timeout_ms` values are retained exactly;
the field is an unsigned 32-bit millisecond value because that is the current
request representation. Values outside that representation, zero, malformed
numbers, null and null strings are rejected before dispatch. No 30-second
clamp or static HTTP wall-time ceiling is applied by the handler, its native
input schema or resource manifest. The configured transport default remains
finite. Task compute, live network policy and other resource constraints still
apply independently.

Packaged Tool/ToolSkill metadata describes this contract. Insert-only seeding
preserves existing stored rows and operator overrides; this source change does
not establish immutable successor activation on an upgraded instance. The first-party HTTP path now forwards omitted response bounds too, uses strict
response-limit errors and returns complete sanitized output. Its former fixed
body, request/header and output formatting ceilings are removed. Shared limits
for other Tools and immutable seed upgrades remain separate work.

## 4. Current API flow

```rust
let enforcer = StaticNetworkPolicyEnforcer::new(policy);
let permit = enforcer
    .authorize(NetworkRequest {
        scope,
        target,
        method: NetworkMethod::Post,
        estimated_bytes: Some(512),
    })
    .await?;
```

`NetworkPermit` carries only metadata needed by a runtime adapter to proceed. It does not hold sockets, HTTP clients, response bodies, secrets, raw host paths, or resource reservations.

The host-mediated HTTP path is:

```rust
let egress = PolicyNetworkHttpEgress::new(ReqwestNetworkTransport::default());
let response = egress.execute(NetworkHttpRequest {
    scope,
    method: NetworkMethod::Post,
    url: "https://api.example.test/v1/run".to_string(),
    headers,
    body,
    policy,
    response_body_limit: Some(64 * 1024),
})?;
```

`NetworkUsage.request_bytes` is outbound request body bytes only. Response bytes are recorded separately as `NetworkUsage.response_bytes` and must not be folded into `ResourceUsage.network_egress_bytes`. Runtime-visible error mapping must use stable sanitized categories (for example `invalid_url`, `policy_denied`, `dns_failed`, `transport_failed`, and `response_body_limit_exceeded`) rather than raw DNS/transport backend strings.

---

## 5. Non-goals

This slice does not implement:

- proxy execution or process-level network sandboxing
- resource reservation for network egress
- credential or secret injection
- durable audit/event emission
- per-method policy matrices
- per-tenant persisted policy stores
- OAuth/token refresh flows

Those should be added as separate service/composition slices without moving runtime execution or product workflow semantics into this crate. Runtime adapters that wrap external protocol clients must fail closed unless the host-selected client explicitly uses this host-mediated egress boundary rather than ambient direct HTTP. Reborn MCP HTTP/SSE uses `brassclaw_mcp::McpHostHttpClient` with `McpRuntimeHttpAdapter<RuntimeHttpEgress>` plus a host-owned egress planner; only that fully host-mediated client may report `uses_host_mediated_http_egress() == true`. Reborn script execution remains ambient-network-disabled by default; any future script HTTP surface must translate into `ScriptRuntimeHttpAdapter<RuntimeHttpEgress>` requests instead of adding direct HTTP/DNS/private-IP logic to `brassclaw_scripts`.

---

## 6. Contract tests

The crate tests cover:

- exact scheme/host/port allow path
- one-label wildcard host matching
- wildcard apex and nested-subdomain denial
- scheme/host/port mismatch denial
- estimated egress limit denial
- missing egress estimate denial when a limit is configured
- literal non-public IP denial
- IPv4-mapped IPv6 private address denial
- hostname resolution to non-public IP denial before transport
- method + URL + header byte counting for `max_egress_bytes`
- URL userinfo denial before policy/DNS/transport
- full resolved-address set preservation for transport fallback
- caller-provided `Host` header denial before transport
- default-port target matching for URL-derived requests
- redirects are not followed by the default transport
- ambient proxy environment variables cannot divert vetted-address transport
- real send/body-transfer errors retain sanitized categories without request URLs, injected credentials or backend body data
- streaming response body limits are enforced separately from request-byte accounting
- omitted response body limits use a bounded default; actual responses above that default succeed with a larger explicit limit
- explicit timeouts above the constructor default remain effective through actual delayed responses
- zero/nonrepresentable limits fail before dispatch without copying sensitive URLs
- fail-closed empty policy behavior
- crate boundary remains low-level and does not depend on workflow/runtime/secret/observability crates
