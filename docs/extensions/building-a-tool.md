---
title: How to implement a Reborn tool extension
description: "A Reborn-only implementation guide for BrassClaw extension tools"
---

# How to implement a Reborn tool extension

This guide is for coding agents and engineers adding a BrassClaw Reborn
extension tool. It is intentionally Reborn-only. Do not use V1 extension,
native-extension, pending-OAuth-map, WIT/Wasm, or script-runtime patterns
when following this document. The Wasm and Script runtime lanes were
removed in Phase 4 of the v2 consolidation (`docs/reborn/contracts/extensions.md`).
Every new tool today is a First-party or MCP capability.

The guide is grounded in the current Notion, GitHub hosted-MCP, and
internal first-party tool implementations bundled in
`crates/brassclaw_first_party_extensions/assets/`.

## Success criteria

A Reborn tool extension is complete only when all of the following are true:

1. The extension package has a `schema_version = "reborn.extension_manifest.v2"`
   manifest and every model-visible capability has schema, output schema, and
   prompt assets.
2. The manifest declares the correct runtime lane: `mcp`, `first_party`, or
   `system`. Wasm and script lanes are no longer wired.
3. The manifest exposes tools through `brassclaw.capability_provider/v1` via the
   registry extension manifest path. Do not add or copy top-level
   `[[capabilities]]` declarations.
4. The runtime code does not read raw secrets, create its own HTTP client for
   external provider calls, bypass approvals, or dispatch directly into the
   agent loop.
5. Network, credentials, approvals, and resource bounds are enforced by the
   Reborn host APIs and runtime services.
6. Tests cover manifest validation, runtime dispatch behavior, credential/auth
   gates, and caller-facing behavior through the runtime or lifecycle call site.

## Reborn extension flow

Use this mental model before touching files:

```text
Extension package
  -> lifecycle/discovery materializes it into the extension registry
  -> brassclaw_extensions parses manifest v2 host APIs and projects descriptors
  -> brassclaw_host_runtime publishes hot model-facing schemas/prompts
  -> model selects a visible capability
  -> brassclaw_capabilities performs authorization, approvals, obligations, run state
  -> host runtime selects the runtime adapter by RuntimeKind
  -> first-party or MCP adapter executes through host-provided services
  -> host HTTP egress injects staged credentials and enforces network policy
  -> sanitized JSON output returns to the loop
```

Important ownership rule:

```text
brassclaw_extensions knows what can run.
runtime crates know how to run it.
authorization/approvals decide whether it may run.
host runtime/composition wires the concrete services.
```

Do not collapse those layers into a shortcut.

## Choose the runtime lane

Pick one lane first. Do not blend lanes to make a tool work.

| Lane | Use when | Current examples | Main files |
| --- | --- | --- | --- |
| Hosted HTTP MCP | The provider already exposes an MCP server and the host should lock egress to that endpoint. This is the default for provider tools. | Notion, GitHub hosted MCP | `crates/brassclaw_first_party_extensions/assets/<provider>-mcp/manifest.toml`, `schemas/`, `prompts/`, optionally `crates/brassclaw_reborn_composition/src/mcp.rs` only for new host-bundled MCP policy shape |
| First-party capability | Logic is host-owned — a Rust crate shipped with the binary or a workspace-level capability. | Internal HTTP wrappers, scheduled cron, declarative HTTP descriptors, system accessors | `crates/brassclaw_first_party_extensions/src/**` plus a `manifest.toml` declaring `kind = "first_party"` |
| Product adapter | The extension receives external inbound events or product webhooks. This is not just a model-callable tool lane. | Telegram v2, future Slack/Discord v2 | `crates/brassclaw_product_adapters`, `crates/brassclaw_product_adapter_registry` |
| System | Host-owned system fixtures and reference loops. Not user-installable. | Regression test fixtures, ship-with-the-binary scheduled missions | Manifest declared by composition code only; users cannot install `System` lane packages |

For a new provider API like Linear, Jira, or a small internal SaaS API, start
with hosted MCP — point the manifest at the provider's MCP server and let
Reborn enforce egress, credentials, and approvals. Reach for First-party
when the logic is genuinely host-owned (cron, declarative HTTP wrappers,
telemetry, internal service access).

> **Removed lanes.** `wasm`, `script`, and Docker-script backends are no
> longer wired. The legacy `brassclaw_wasm`,
> `brassclaw_wasm_sandbox_core`, `brassclaw_wasm_limiter`,
> `brassclaw_wasm_product_adapters`, and `brassclaw_scripts` crates were
> deleted. If you need to spawn a process, use the `brassclaw_process_sandbox`
> shell tool inside a capability grant; if you need a remote adapter, host its
> MCP server and wire it through `brassclaw_mcp`.

## Crates to touch

Touch only the smallest set for your lane.

### Common extension package work

Usually touch:

- `crates/brassclaw_first_party_extensions/assets/<extension>/manifest.toml`
- `crates/brassclaw_first_party_extensions/assets/<extension>/schemas/<extension>/*.json`
- `crates/brassclaw_first_party_extensions/assets/<extension>/prompts/<extension>/*.md`
- `crates/brassclaw_reborn_composition/src/available_extensions.rs` only when adding
  a host-bundled available extension to the built-in install catalog.

Do not touch for ordinary tools:

- `crates/brassclaw_extensions/src/v2.rs`, unless changing the manifest contract
  itself.
- `crates/brassclaw_host_api/src/*`, unless adding a new shared host API type.
- `crates/brassclaw_capabilities`, unless changing authorization/approval
  orchestration for all capabilities.
- `crates/brassclaw_approvals`, unless changing approval lease semantics.
- `crates/brassclaw_secrets`, unless changing low-level secret storage/lease
  semantics.
- `crates/brassclaw_network`, unless changing global network policy/HTTP egress
  semantics.
- agent loop crates for tool-specific routing. Tool selection must come from the
  published capability surface, not hardcoded model-routing logic.

### Hosted MCP lane

Usually touch:

- `crates/brassclaw_first_party_extensions/assets/<provider>-mcp/manifest.toml`
- `schemas/<provider>/...`
- `prompts/<provider>/...`
- `crates/brassclaw_reborn_composition/src/mcp.rs` only if adding a new
  host-bundled MCP policy shape.
- `crates/brassclaw_reborn_composition/src/<provider>_oauth.rs` for product
  auth / OAuth DCR wiring if the provider uses hosted MCP.

Use as references:

- `crates/brassclaw_first_party_extensions/assets/notion-mcp/manifest.toml`
- `crates/brassclaw_reborn_composition/src/mcp.rs`
- `crates/brassclaw_reborn_composition/src/notion_oauth.rs`

Only touch `crates/brassclaw_reborn_composition/src/mcp.rs` if the hosted MCP
runtime policy needs a new generic rule. Notion already demonstrates the common
shape: HTTPS-only endpoint, exact host/path match, no URL credentials, no query,
no fragment, host-mediated egress, staged product-auth token.

### First-party capability lane

Usually touch only when adding a host-owned Rust implementation:

- `crates/brassclaw_first_party_extensions/src/<extension>.rs` for the Rust
  implementation.
- `crates/brassclaw_first_party_extensions/assets/<extension>/manifest.toml`
  declaring `kind = "first_party"`.
- `schemas/`, `prompts/` for model-facing surfaces.
- `crates/brassclaw_reborn_composition/src/available_extensions.rs` to wire
  the extension into the built-in install catalog.

First-party tools exercise the host HTTP egress boundary by calling
`brassclaw_host_runtime` helpers directly (not via WIT, not via a script
runtime). Reach for shell-side work through `brassclaw_process_sandbox` —
never reintroduce a script runtime variant.

### Auth/OAuth lane

Usually touch only when adding a new product-auth provider:

- `crates/brassclaw_auth` for provider/scopes/account-domain vocabulary when it
  must be shared and durable.
- `crates/brassclaw_reborn_composition/src/<provider>_oauth.rs` for provider
  specs like Notion.
- `crates/brassclaw_reborn_composition/src/oauth_provider_client.rs` only if the
  provider needs a new generic exchange behavior.
- `crates/brassclaw_reborn_composition/src/product_auth_serve/` only for product
  auth HTTP setup/callback surfaces.

Do not create extension-local OAuth maps or store OAuth tokens in runtime code.
Credential accounts and secrets belong to `brassclaw_auth` /
`brassclaw_secrets` through Reborn composition.

## Files not to touch

For a normal extension, do not touch these:

- `src/agent/*` or Reborn loop strategy code to special-case your tool.
- `crates/brassclaw_llm/*` to teach the model your tool name.
- `crates/brassclaw_engine/*` V1 runtime paths.
- `src/tools/*` V1 tools.
- `crates/brassclaw_host_api` for one provider's fields.
- `crates/brassclaw_extensions/src/v2.rs` to allow a one-off manifest shortcut.
- `crates/brassclaw_network` to allow one provider host.
- `crates/brassclaw_secrets` to fetch one provider token.
- `crates/brassclaw_approvals` to make one write operation easier.

If your implementation appears to require one of these, stop and identify the
missing Reborn contract or composition seam first.

## Manifest v2 structure

All Reborn packages use:

```toml
schema_version = "reborn.extension_manifest.v2"
id = "example"
name = "Example"
version = "0.1.0"
description = "Example tools for Reborn."
trust = "third_party"

[runtime]
kind = "mcp"            # or "first_party" / "system"
transport = "stdio"      # MCP-specific
command = "example-mcp-server"
args = ["--stdio"]
```

Extension IDs and capability IDs are authority-bearing:

- `id` must be lowercase ASCII letters/digits plus `_`, `-`, or `.`.
- Capability IDs are `<extension_id>.<capability_name>`.
- Do not use slashes, uppercase, raw host paths, or `..`.
- Registry extensions cannot claim effective first-party/system authority.
  Host composition decides effective trust.

### All tool extensions: use `host_api`

Publish model-visible tools via the capability-provider host API:

```toml
[[host_api]]
id = "brassclaw.capability_provider/v1"
section = "capability_provider.tools"

[capability_provider.tools]

[[capability_provider.tools.capabilities]]
id = "example.search"
description = "Search Example records."
effects = ["dispatch_capability", "network", "use_secret"]
default_permission = "ask"
visibility = "model"
input_schema_ref = "schemas/example/search.input.v1.json"
output_schema_ref = "schemas/example/search.output.v1.json"
prompt_doc_ref = "prompts/example/search.md"
required_host_ports = ["host.runtime.http_egress"]
runtime_credentials = [
  { handle = "example_runtime_token", source = { type = "product_auth_account", provider = "example", setup = { kind = "oauth", scopes = ["records.read"] } }, provider_scopes = ["records.read"], audience = { scheme = "https", host_pattern = "api.example.com" }, target = { type = "header", name = "authorization", prefix = "Bearer " } },
]
```

Do not use top-level `[[capabilities]]` for Reborn tool work. If a current
bundled manifest still has that shape, do not treat that file shape as a
reference. Treat it as migration debt and move it to `[[host_api]]` plus
`[capability_provider.tools]` when touching that extension.

### Capability fields

Required per model-visible capability:

- `id`: stable `<extension>.<name>` capability ID.
- `description`: short, model-facing description.
- `effects`: accurate effects. Include `external_write` for provider writes,
  mutations, sends, deletes, comments, or workflow dispatches.
- `default_permission`: use `ask` for writes and high-risk reads; use `allow`
  only for low-risk read capabilities that policy deliberately permits.
- `visibility`: usually `model`.
- `input_schema_ref`: relative path to JSON schema.
- `output_schema_ref`: relative path to JSON schema.
- `prompt_doc_ref`: relative path to concise operation guidance.
- `required_host_ports`: include `host.runtime.http_egress` when the runtime
  must make host-mediated HTTP calls.
- `runtime_credentials`: declare every credential the runtime may receive.

Validation catches common mistakes:

- `runtime_credentials` without `use_secret` is rejected. This includes
  product-auth account credentials: product auth selects/refreshes the account,
  but runtime dispatch still uses a host-staged access-secret handle.
- Duplicate effects and duplicate credential handles are rejected.
- Unknown host ports are rejected.
- Credential audiences must be declared as HTTPS.
- Schema and prompt refs must be relative package paths, not absolute paths,
  URLs, backslash paths, or paths with `..`.

### Effects and approvals

Use effects as authorization inputs, not as documentation.

Common mapping:

- Read-only API call with credentials: `["dispatch_capability", "network",
  "use_secret"]`.
- Provider write: add `"external_write"`.
- Local filesystem read/write: use `read_filesystem`, `write_filesystem`,
  `delete_filesystem` as appropriate.
- Process/CLI work: use `execute_code` or `spawn_process` as appropriate.
- Money or irreversible financial actions: include `financial`.

`default_permission = "ask"` is the normal default for anything with
`external_write`, `financial`, local write/delete, process execution, approval
mutation, extension mutation, or budget mutation.

Approvals are resolved by `brassclaw_capabilities`, `brassclaw_approvals`, and run
state. Runtime code must return a normal runtime error when blocked; it must not
prompt the user, mint approval leases, or resume turns directly.

## Schemas and prompts

Schemas are part of the hot model-facing surface. They should make the desired
input shape obvious and reject ambiguous or unsafe input before side effects.

Follow these rules:

- Use JSON Schema object inputs with `additionalProperties: false` unless the
  upstream provider truly requires arbitrary JSON.
- Require the fields needed to construct one provider operation.
- Prefer provider-neutral names only when they are already established locally.
- Put path/ID/URL validation in runtime code too; schemas are not a security
  boundary.
- Output schemas may be provider raw JSON for compatibility, but typed output
  is better when the runtime owns the shape.

Prompt docs are lazy help metadata. Keep them operation-specific:

- What the tool does.
- Required identifiers.
- How to avoid common destructive mistakes.
- Any provider constraints the model should know.

Do not put secrets, host paths, environment assumptions, or V1 setup commands in
prompt docs.

## HTTP and network integration

Runtime code must use host-mediated HTTP:

- Hosted MCP goes through `McpHostHttpClient` with `McpRuntimeHttpAdapter` and
  a host-owned egress planner.
- First-party capability implementations call `brassclaw_host_runtime`
  HTTP helpers directly.

Do not:

- instantiate direct `reqwest` clients in runtime code for provider API calls;
- follow redirects yourself to bypass host policy;
- accept model-provided `Authorization`, cookie, API-key, or token headers;
- put credentials in URLs;
- widen global network policy for one extension.

Network policy belongs in host/runtime planning:

- Hosted MCP policy is planned in
  `crates/brassclaw_reborn_composition/src/mcp.rs`.
- First-party credential injection is derived from manifest descriptors in
  `crates/brassclaw_host_runtime/src/credentials.rs` (the
  `wasm_credentials.rs` module was generalised in Phase 4).
- Shared HTTP enforcement and redaction live in
  `crates/brassclaw_host_runtime/src/egress/` and `crates/brassclaw_network`.

Provider requests should set ordinary provider headers like `Accept`,
`Content-Type`, API version, and User-Agent in runtime code. Credential headers
must come from `runtime_credentials` and host egress injection.

## Secrets and runtime credentials

Secrets are opaque handles in manifests and host API types. Runtime code should
never see raw token material except as already-injected HTTP request data inside
the host egress boundary.

Use `runtime_credentials` for every credential. Product auth is the preferred
source for provider accounts, but it is still represented as a runtime
credential because host egress injects the selected account's access-secret
handle at dispatch time:

```toml
runtime_credentials = [
  { handle = "github_runtime_token", source = { type = "product_auth_account", provider = "github" }, audience = { scheme = "https", host_pattern = "api.github.com" }, target = { type = "header", name = "authorization", prefix = "Bearer " } },
]
```

Important fields:

- `handle`: extension/runtime-local credential handle. Keep it stable.
- `source`: omit or use `{ type = "secret_handle" }` only for manual direct
  secret-handle credentials. Prefer `{ type = "product_auth_account", provider = "..." }`
  for OAuth-backed provider accounts.
- `audience`: must be HTTPS. Use `host_pattern` to scope the audience broadly
  to a provider backend.
- `target`: header/query_param name and prefix. The host egress writes the
  injected secret into the request using this target.
- `required`: declare each credential as `required = true` when the runtime
  cannot function correctly without it; otherwise the host may grant optional
  credentials.

The runtime never inlines, logs, returns, or returns with the secret value.
Egress responses are scanned for credentials; raw token material is redacted
in logs and audit.

## Validation and provenance

Every extension goes through:

1. Manifest schema validation (`brassclaw_extensions::v2::validate`).
2. Host-API contract parser.
3. Effect/credential consistency check.
4. Trust ceiling check (host composition authority; manifest `trust` is a
   ceiling only).
5. Schema and prompt reference resolution.
6. Runtime adapter health probe for hosted MCP lanes (`McpHostHttpClient::
   health`).
7. Catalog publication into the prompt-envelope tool surface.

A failure at any step rejects the extension at install time. There is no
partial-load path; every host API contract must validate atomically.

## Operational checklist

Before opening a PR for a new tool:

- [ ] Manifest declares `runtime.kind` in `{mcp, first_party, system}`.
- [ ] Every model-visible capability has schema, output schema, prompt, and
      `default_permission` set.
- [ ] `runtime_credentials` lists every credential the runtime needs, and
      `audience` is HTTPS-bound to the provider host.
- [ ] Network egress is host-mediated; no inline `reqwest::Client` in
      runtime code.
- [ ] No raw secrets, host paths, environment assumptions, or V1 setup
      commands appear in prompt docs.
- [ ] Tests cover manifest validation, runtime dispatch behavior, and
      credential/auth gates via the runtime or lifecycle call site (not
      only via internal helpers).
- [ ] Optional: agent-skill coverage — if the capability supports automatic
      learning, the capability descriptor is annotated so Phase-7 recipe
      extraction can pick it up from successful threads.
