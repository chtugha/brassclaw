---
title: Model Context Protocol (MCP)
description: BrassClaw exposes its capabilities as an MCP server and connects to external MCP providers
---

BrassClaw operates as an **MCP server**, exposing its approved Recipe-backed
capabilities to external MCP clients (AI clients, orchestration systems, IDE
integrations). It also connects to external MCP servers through its extension
system, making remote provider tools available to the agent.

---

## BrassClaw as an MCP server

The BrassClaw MCP server is always running while the instance is up. It
exposes a `tools/list` derived from the set of available activated, approved
**mcp-call-skill-recipes** — Recipe components that have been explicitly
qualified for MCP publication with a complete command contract.

### How it works

```
MCP client sends tools/list request
    ↓
BrassClaw MCP server returns list of approved mcp-call-skill-recipes
    (each listed as one command sentence with variable positions and types)
    ↓
MCP client selects a tool and sends a completed command
    ↓
BrassClaw MCP server opens a new ordinary chat, submits the command
    ↓
Global Monty orchestrator matches the command to the Recipe,
assembles the BuildInstruction, executes the matched steps
    ↓
BrassClaw MCP server forwards the terminal chat reply to the client
    ↓
Chat closed; MCP server connection remains active for further calls
```

The MCP server has no direct Monty/IBS/Rust Tool execution connection of its
own. Every command goes through ordinary chat ingress, intent matching, and
Recipe execution. The server accepts only completed advertised Recipe commands;
it never accepts raw Python payloads or arbitrary code.

### Discovery source

`tools/list` is populated from active, approved `mcp-call-skill-recipe`
components in the component catalogue. A Skill or Recipe that has not been
explicitly published with a full MCP command contract does not appear in
`tools/list`. Raw `reborn_skills` DB rows alone do not create MCP listings.

The list is refreshed at startup, restart, and on qualified Skill/Recipe
catalogue activation changes. In-flight calls retain their advertised contract.

### Authentication

The inbound MCP transport requires host-owned authentication. Browser Origins
are rejected until explicit browser-origin policy is wired. Production transport
authenticates per-request provider exchanges; routes without a chat facade are
unavailable for calls.

### Configuration

MCP server settings are managed via **Settings → MCP Server** in the WebUI or
the corresponding Settings API endpoints:

```
GET  /api/settings/mcp-server       # current state
POST /api/settings/mcp-server/start # start the server
POST /api/settings/mcp-server/stop  # stop the server
```

The MCP protocol version used is `2025-06-18`. Maximum request size is
configurable (default 1 MiB).

---

## Connecting to external MCP servers (extensions)

BrassClaw's extension system supports MCP as a runtime lane for external
provider integrations. When an extension declares `kind = "mcp"` in its
manifest, BrassClaw acts as an MCP client connecting to that provider's MCP
server on behalf of the agent.

This is different from BrassClaw's own MCP server surface. The extension
system's MCP client lane connects to external tools (Notion, GitHub, Stripe,
etc.) and exposes their capabilities through the standard extension capability
surface — the same authorization, credential injection, and network policy
boundaries that apply to all extensions.

```toml
# Example extension manifest for a hosted MCP provider
schema_version = "reborn.extension_manifest.v2"
id = "notion-mcp"
name = "Notion"
version = "1.0.0"
trust = "third_party"

[runtime]
kind = "mcp"
transport = "http"
url = "https://mcp.notion.com/mcp"
```

### Available hosted MCP extensions

| Extension | Transport | What it does |
|-----------|-----------|--------------|
| Notion | HTTP | Pages, databases, and comments |
| GitHub | HTTP | Repositories, issues, pull requests |
| Stripe | HTTP | Payments, subscriptions, invoices |
| Linear | HTTP | Issue tracking and project management |
| Asana | HTTP | Task management and coordination |
| Cloudflare | HTTP | DNS, Workers, KV, infrastructure |
| Sentry | HTTP | Error tracking and performance |
| Intercom | HTTP | Customer messaging and support |

Extensions are installed and managed through the extension lifecycle, not via
`brassclaw mcp add` CLI commands. See the
[Extension overview](/extensions/overview) for installation instructions.

### Authentication for external MCP providers

OAuth-backed providers use product auth accounts managed by BrassClaw's auth
layer. Credentials are never passed raw to the extension runtime — the host
egress layer injects the access token into each outbound request.

```toml
runtime_credentials = [
  { handle = "notion_token",
    source = { type = "product_auth_account", provider = "notion" },
    audience = { scheme = "https", host_pattern = "mcp.notion.com" },
    target = { type = "header", name = "authorization", prefix = "Bearer " } },
]
```

---

## Summary: two distinct MCP directions

| Direction | What it is | How it works |
|-----------|-----------|--------------|
| **BrassClaw as MCP server** | Exposes BrassClaw capabilities to external AI clients | Commands matched to approved mcp-call-skill-recipes; executed via ordinary chat → Recipe execution |
| **Extension MCP client** | Connects to external provider MCP servers | Extension manifest declares `kind = "mcp"`; host mediates egress and credentials |

These two directions are independent. Enabling an external MCP extension does
not affect the BrassClaw MCP server's `tools/list`. The BrassClaw MCP server
does not proxy arbitrary external MCP servers to clients.

---

## Troubleshooting

<AccordionGroup>
  <Accordion title="tools/list is empty" icon="list">
    `tools/list` is populated from activated, approved mcp-call-skill-recipes.
    If no such Recipes are published, the list will be empty. Check
    **Settings → Component Catalog → Recipes** to confirm mcp-call-skill-recipe
    entries are active and approved.
  </Accordion>

  <Accordion title="Command not matched" icon="x-circle">
    MCP accepts only completed advertised commands as listed in `tools/list`.
    The command is submitted as a user message to ordinary chat ingress and must
    match the Recipe's intent examples. Verify the command matches the exact
    listed syntax including variable positions.
  </Accordion>

  <Accordion title="External MCP extension not connecting" icon="network">
    Check the extension's installation status and the provider's MCP server
    availability. The runtime adapter health probe runs during extension
    activation. Re-activating the extension triggers a fresh health probe.
  </Accordion>
</AccordionGroup>
